//! Extension system for Ratterm.
//!
//! Provides a plugin architecture based on REST API communication.
//! Extensions run as external processes and communicate with Ratterm
//! via HTTP REST API, allowing any programming language to be used.
//!
//! Extensions can be installed from GitHub repositories.

pub mod api;
pub mod approval;
pub mod installer;
pub mod manifest;
pub mod process;
pub mod registry;
pub mod rest;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use api::{PluginCapability, PluginHost, PluginInfo, PluginType, RattermPlugin};
pub use approval::{ApprovalEntry, ApprovalManager};
pub use manifest::{ExtensionManifest, ProcessConfig};
pub use process::{ApiExtensionManager, ProcessStatus};
pub use rest::{ApiEvent, ApiState, AppRequest, RestApiServer};

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
    /// Approval error.
    Approval(String),
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
            ExtensionError::Approval(e) => write!(f, "Approval error: {}", e),
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
    /// Path to extension directory.
    pub path: PathBuf,
    /// Manifest data.
    pub manifest: ExtensionManifest,
}

impl InstalledExtension {
    /// Returns the command that this extension runs.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        self.manifest.command()
    }

    /// Returns the extension author.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        self.manifest.author()
    }

    /// Returns the extension description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.manifest.description()
    }
}

/// Extension manager that handles loading and managing extensions.
pub struct ExtensionManager {
    /// Installed extensions by name.
    installed: HashMap<String, InstalledExtension>,
    /// Approval manager for user consent.
    approval_manager: ApprovalManager,
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
            approval_manager: ApprovalManager::new(),
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

        // Register default hotkey if specified and not already in .ratrc
        if let Some(hotkey) = manifest.default_hotkey() {
            self.register_default_hotkey(&manifest.extension.name, hotkey, path);
        }

        let ext = InstalledExtension {
            name: manifest.extension.name.clone(),
            version: manifest.extension.version.clone(),
            path: path.to_path_buf(),
            manifest: manifest.clone(),
        };

        self.installed.insert(ext.name.clone(), ext);
        Ok(())
    }

    /// Registers an extension's default hotkey in .ratrc if not already present.
    fn register_default_hotkey(&self, name: &str, hotkey: &str, ext_path: &Path) {
        let Some(config_path) = dirs::home_dir().map(|h| h.join(".ratrc")) else {
            return;
        };

        // Read current config
        let content = fs::read_to_string(&config_path).unwrap_or_default();

        // Check if addon entry already exists
        let addon_key = format!("addon.{}", name);
        if content.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && line.starts_with(&addon_key)
        }) {
            // Already configured, don't override user settings
            return;
        }

        // Build the command path
        let command = if cfg!(windows) {
            ext_path.join(format!("{}.exe", name))
        } else {
            ext_path.join(name)
        };

        // Append the addon entry to .ratrc
        let entry = format!(
            "\n# {} extension (auto-registered)\n{} = {}|{}\n",
            name,
            addon_key,
            hotkey,
            command.display()
        );

        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&config_path) {
            use std::io::Write;
            let _ = file.write_all(entry.as_bytes());
            tracing::info!("Registered default hotkey {} for extension {}", hotkey, name);
        }
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

        // Also revoke approval when removing
        let _ = self.approval_manager.revoke(name);

        // Remove the directory
        fs::remove_dir_all(&ext.path)?;

        Ok(())
    }

    /// Returns the count of installed extensions.
    #[must_use]
    pub fn count(&self) -> usize {
        self.installed.len()
    }

    /// Returns a reference to the approval manager.
    #[must_use]
    pub fn approval_manager(&self) -> &ApprovalManager {
        &self.approval_manager
    }

    /// Returns a mutable reference to the approval manager.
    pub fn approval_manager_mut(&mut self) -> &mut ApprovalManager {
        &mut self.approval_manager
    }

    /// Checks if an extension is approved for its current version.
    #[must_use]
    pub fn is_approved(&self, name: &str) -> bool {
        if let Some(ext) = self.installed.get(name) {
            self.approval_manager.is_approved(name, &ext.version)
        } else {
            false
        }
    }

    /// Approves an extension for its current installed version.
    pub fn approve(&mut self, name: &str) -> Result<(), ExtensionError> {
        let ext = self
            .installed
            .get(name)
            .ok_or_else(|| ExtensionError::NotFound(name.to_string()))?;

        self.approval_manager
            .approve(name, &ext.version)
            .map_err(|e| ExtensionError::Approval(e.to_string()))
    }

    /// Returns extensions that need approval (not approved for current version).
    #[must_use]
    pub fn pending_approval(&self) -> Vec<&InstalledExtension> {
        self.installed
            .values()
            .filter(|ext| !self.approval_manager.is_approved(&ext.name, &ext.version))
            .collect()
    }

    /// Returns extensions that are approved and ready to run.
    #[must_use]
    pub fn approved_extensions(&self) -> Vec<&InstalledExtension> {
        self.installed
            .values()
            .filter(|ext| self.approval_manager.is_approved(&ext.name, &ext.version))
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

    #[test]
    fn test_pending_approval_empty() {
        let manager = ExtensionManager::new();
        assert!(manager.pending_approval().is_empty());
        assert!(manager.approved_extensions().is_empty());
    }
}
