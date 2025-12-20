//! Extension system for Ratterm.
//!
//! Provides a plugin architecture supporting:
//! - Theme extensions (TOML-based color schemes)
//! - WASM plugins (sandboxed, portable)
//! - Native plugins (.dll/.so/.dylib for power users)
//! - Lua plugins (scripted, full system access)
//!
//! Extensions can be installed from GitHub repositories.

pub mod api;
pub mod installer;
pub mod lua;
pub mod lua_api;
pub mod manifest;
pub mod native;
pub mod registry;
pub mod theme_ext;
pub mod wasm;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use api::{PluginCapability, PluginHost, PluginInfo, PluginType, RattermPlugin};
pub use lua::{LuaPlugin, LuaPluginManager};
pub use lua_api::events::EventType as LuaEventType;
pub use lua_api::{EditorOp, LuaContext, LuaState, TerminalOp};
pub use manifest::{ExtensionManifest, ExtensionType, LuaConfig};

/// Errors that can occur in the extension system.
#[derive(Debug)]
pub enum ExtensionError {
    /// IO error.
    Io(io::Error),
    /// Manifest parsing error.
    Manifest(String),
    /// Extension not found.
    NotFound(String),
    /// Extension already installed.
    AlreadyInstalled(String),
    /// Plugin load error.
    PluginLoad(String),
    /// Registry/download error.
    Registry(String),
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::Io(e) => write!(f, "IO error: {}", e),
            ExtensionError::Manifest(e) => write!(f, "Manifest error: {}", e),
            ExtensionError::NotFound(n) => write!(f, "Extension not found: {}", n),
            ExtensionError::AlreadyInstalled(n) => write!(f, "Already installed: {}", n),
            ExtensionError::PluginLoad(e) => write!(f, "Plugin load error: {}", e),
            ExtensionError::Registry(e) => write!(f, "Registry error: {}", e),
        }
    }
}

impl std::error::Error for ExtensionError {}

impl From<io::Error> for ExtensionError {
    fn from(e: io::Error) -> Self {
        ExtensionError::Io(e)
    }
}

/// Returns the path to the ratterm data directory.
#[must_use]
pub fn ratterm_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ratterm"))
}

/// Returns the path to the extensions directory.
#[must_use]
pub fn extensions_dir() -> Option<PathBuf> {
    ratterm_dir().map(|d| d.join("extensions"))
}

/// Returns the path to the download cache directory.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    ratterm_dir().map(|d| d.join("cache").join("downloads"))
}

/// Ensures all extension directories exist.
pub fn ensure_directories() -> io::Result<()> {
    if let Some(ext_dir) = extensions_dir() {
        fs::create_dir_all(&ext_dir)?;
    }
    if let Some(cache) = cache_dir() {
        fs::create_dir_all(&cache)?;
    }
    Ok(())
}

/// Information about an installed extension.
#[derive(Debug, Clone)]
pub struct InstalledExtension {
    /// Extension name.
    pub name: String,
    /// Extension version.
    pub version: String,
    /// Extension type.
    pub ext_type: ExtensionType,
    /// Path to extension directory.
    pub path: PathBuf,
    /// Manifest data.
    pub manifest: ExtensionManifest,
}

/// Extension manager that handles loading and managing extensions.
pub struct ExtensionManager {
    /// Installed extensions by name.
    installed: HashMap<String, InstalledExtension>,
    /// Loaded plugins (native and WASM).
    /// Currently unused but reserved for future plugin lifecycle management.
    #[allow(dead_code)]
    plugins: Vec<Box<dyn RattermPlugin>>,
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionManager {
    /// Creates a new extension manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            installed: HashMap::new(),
            plugins: Vec::new(),
        }
    }

    /// Initializes the extension system and loads installed extensions.
    pub fn init(&mut self) -> Result<(), ExtensionError> {
        ensure_directories()?;
        self.discover_extensions()?;
        Ok(())
    }

    /// Discovers all installed extensions.
    pub fn discover_extensions(&mut self) -> Result<(), ExtensionError> {
        let Some(ext_dir) = extensions_dir() else {
            return Ok(());
        };

        if !ext_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&ext_dir)?;

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                if let Err(e) = self.load_extension(&path) {
                    tracing::warn!("Failed to load extension at {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Loads an extension from a directory.
    fn load_extension(&mut self, path: &Path) -> Result<(), ExtensionError> {
        let manifest_path = path.join("extension.toml");
        if !manifest_path.exists() {
            return Err(ExtensionError::Manifest(format!(
                "No extension.toml found in {:?}",
                path
            )));
        }

        let manifest = manifest::load_manifest(&manifest_path)?;

        let ext = InstalledExtension {
            name: manifest.extension.name.clone(),
            version: manifest.extension.version.clone(),
            ext_type: manifest.extension.ext_type,
            path: path.to_path_buf(),
            manifest: manifest.clone(),
        };

        self.installed.insert(ext.name.clone(), ext);
        Ok(())
    }

    /// Returns all installed extensions.
    #[must_use]
    pub fn installed(&self) -> &HashMap<String, InstalledExtension> {
        &self.installed
    }

    /// Returns a list of installed extension names.
    #[must_use]
    pub fn list_installed(&self) -> Vec<&str> {
        self.installed.keys().map(String::as_str).collect()
    }

    /// Checks if an extension is installed.
    #[must_use]
    pub fn is_installed(&self, name: &str) -> bool {
        self.installed.contains_key(name)
    }

    /// Gets an installed extension by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&InstalledExtension> {
        self.installed.get(name)
    }

    /// Removes an installed extension.
    pub fn remove(&mut self, name: &str) -> Result<(), ExtensionError> {
        let ext = self
            .installed
            .remove(name)
            .ok_or_else(|| ExtensionError::NotFound(name.to_string()))?;

        // Remove the directory
        fs::remove_dir_all(&ext.path)?;

        Ok(())
    }

    /// Returns the count of installed extensions.
    #[must_use]
    pub fn count(&self) -> usize {
        self.installed.len()
    }

    /// Returns installed theme extensions.
    #[must_use]
    pub fn theme_extensions(&self) -> Vec<&InstalledExtension> {
        self.installed
            .values()
            .filter(|e| e.ext_type == ExtensionType::Theme)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_manager_new() {
        let manager = ExtensionManager::new();
        assert_eq!(manager.count(), 0);
        assert!(manager.list_installed().is_empty());
    }

    #[test]
    fn test_ratterm_dir() {
        let dir = ratterm_dir();
        assert!(dir.is_some());
    }

    #[test]
    fn test_extensions_dir() {
        let dir = extensions_dir();
        assert!(dir.is_some());
        if let Some(d) = dir {
            assert!(d.ends_with("extensions"));
        }
    }
}
