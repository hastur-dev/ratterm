//! Extension manifest parsing.
//!
//! Parses `extension.toml` files that define extension metadata and configuration.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::ExtensionError;

/// Extension manifest from extension.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionManifest {
    /// Extension metadata.
    pub extension: ExtensionMetadata,
    /// Compatibility requirements.
    #[serde(default)]
    pub compatibility: CompatibilityInfo,
    /// Theme configuration (for theme extensions).
    #[serde(default)]
    pub theme: Option<ThemeConfig>,
    /// WASM plugin configuration.
    #[serde(default)]
    pub wasm: Option<WasmConfig>,
    /// Native plugin configuration.
    #[serde(default)]
    pub native: Option<NativeConfig>,
    /// Lua plugin configuration.
    #[serde(default)]
    pub lua: Option<LuaConfig>,
}

/// Extension metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionMetadata {
    /// Extension name (unique identifier).
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Author name or handle.
    #[serde(default)]
    pub author: String,
    /// License identifier (e.g., "MIT", "GPL-3.0").
    #[serde(default)]
    pub license: String,
    /// Homepage URL.
    #[serde(default)]
    pub homepage: String,
    /// Extension type.
    #[serde(rename = "type")]
    pub ext_type: ExtensionType,
}

/// Extension type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {
    /// Theme extension (colors only).
    #[default]
    Theme,
    /// Widget extension (WASM or native).
    Widget,
    /// Command extension (adds commands).
    Command,
    /// Native plugin (full access).
    Native,
    /// Lua plugin (scripted, full access).
    Lua,
}

impl std::fmt::Display for ExtensionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionType::Theme => write!(f, "theme"),
            ExtensionType::Widget => write!(f, "widget"),
            ExtensionType::Command => write!(f, "command"),
            ExtensionType::Native => write!(f, "native"),
            ExtensionType::Lua => write!(f, "lua"),
        }
    }
}

/// Compatibility requirements.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompatibilityInfo {
    /// Minimum ratterm version (semver range).
    #[serde(default)]
    pub ratterm: String,
}

/// Theme extension configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfig {
    /// Path to theme definition file.
    pub file: String,
}

/// WASM plugin configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WasmConfig {
    /// Path to WASM file.
    pub file: String,
    /// Plugin capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Native plugin configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NativeConfig {
    /// Windows DLL path.
    #[serde(default)]
    pub windows: Option<String>,
    /// Linux SO path.
    #[serde(default)]
    pub linux: Option<String>,
    /// macOS dylib path.
    #[serde(default)]
    pub macos: Option<String>,
    /// Whether the plugin is trusted (requires user confirmation if false).
    #[serde(default)]
    pub trusted: bool,
}

impl NativeConfig {
    /// Returns the plugin path for the current platform.
    #[must_use]
    pub fn current_platform_path(&self) -> Option<&str> {
        #[cfg(target_os = "windows")]
        {
            self.windows.as_deref()
        }
        #[cfg(target_os = "linux")]
        {
            self.linux.as_deref()
        }
        #[cfg(target_os = "macos")]
        {
            self.macos.as_deref()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }
}

/// Lua plugin configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LuaConfig {
    /// Main Lua entry file (e.g., "init.lua").
    #[serde(default)]
    pub main: String,
    /// Optional list of Lua files to preload before main.
    #[serde(default)]
    pub preload: Vec<String>,
}

/// Loads an extension manifest from a file.
pub fn load_manifest(path: &Path) -> Result<ExtensionManifest, ExtensionError> {
    let content = fs::read_to_string(path)
        .map_err(|e| ExtensionError::Manifest(format!("Failed to read manifest: {}", e)))?;

    let manifest: ExtensionManifest = toml::from_str(&content)
        .map_err(|e| ExtensionError::Manifest(format!("Failed to parse manifest: {}", e)))?;

    validate_manifest(&manifest)?;

    Ok(manifest)
}

/// Validates a manifest for required fields and consistency.
fn validate_manifest(manifest: &ExtensionManifest) -> Result<(), ExtensionError> {
    // Name must not be empty
    if manifest.extension.name.is_empty() {
        return Err(ExtensionError::Manifest(
            "Extension name is required".to_string(),
        ));
    }

    // Version must not be empty
    if manifest.extension.version.is_empty() {
        return Err(ExtensionError::Manifest(
            "Extension version is required".to_string(),
        ));
    }

    // Type-specific validation
    match manifest.extension.ext_type {
        ExtensionType::Theme => {
            if manifest.theme.is_none() {
                return Err(ExtensionError::Manifest(
                    "Theme extensions require [theme] section".to_string(),
                ));
            }
        }
        ExtensionType::Widget | ExtensionType::Command => {
            if manifest.wasm.is_none() && manifest.native.is_none() {
                return Err(ExtensionError::Manifest(
                    "Widget/Command extensions require [wasm] or [native] section".to_string(),
                ));
            }
        }
        ExtensionType::Native => {
            if manifest.native.is_none() {
                return Err(ExtensionError::Manifest(
                    "Native extensions require [native] section".to_string(),
                ));
            }
        }
        ExtensionType::Lua => {
            if let Some(lua_config) = &manifest.lua {
                if lua_config.main.is_empty() {
                    return Err(ExtensionError::Manifest(
                        "Lua extensions require 'main' field in [lua] section".to_string(),
                    ));
                }
            } else {
                return Err(ExtensionError::Manifest(
                    "Lua extensions require [lua] section".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_manifest(dir: &Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("extension.toml");
        let mut file = fs::File::create(&path).expect("create file");
        file.write_all(content.as_bytes()).expect("write file");
        path
    }

    #[test]
    fn test_parse_theme_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "my-theme"
version = "1.0.0"
description = "A nice theme"
author = "test"
license = "MIT"
type = "theme"

[theme]
file = "theme.toml"
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.extension.name, "my-theme");
        assert_eq!(manifest.extension.version, "1.0.0");
        assert_eq!(manifest.extension.ext_type, ExtensionType::Theme);
        assert!(manifest.theme.is_some());
    }

    #[test]
    fn test_parse_wasm_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "git-widget"
version = "0.1.0"
type = "widget"

[wasm]
file = "plugin.wasm"
capabilities = ["status_widget"]
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.extension.ext_type, ExtensionType::Widget);
        assert!(manifest.wasm.is_some());
        let wasm = manifest.wasm.as_ref().expect("wasm config");
        assert_eq!(wasm.file, "plugin.wasm");
    }

    #[test]
    fn test_invalid_manifest_missing_name() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = ""
version = "1.0.0"
type = "theme"

[theme]
file = "theme.toml"
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_lua_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "my-lua-extension"
version = "1.0.0"
description = "A Lua extension"
author = "test"
license = "MIT"
type = "lua"

[lua]
main = "init.lua"
preload = ["lib/utils.lua", "lib/helpers.lua"]
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.extension.name, "my-lua-extension");
        assert_eq!(manifest.extension.version, "1.0.0");
        assert_eq!(manifest.extension.ext_type, ExtensionType::Lua);
        assert!(manifest.lua.is_some());
        let lua = manifest.lua.as_ref().expect("lua config");
        assert_eq!(lua.main, "init.lua");
        assert_eq!(lua.preload.len(), 2);
        assert_eq!(lua.preload[0], "lib/utils.lua");
    }

    #[test]
    fn test_invalid_lua_manifest_missing_main() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "bad-lua"
version = "1.0.0"
type = "lua"

[lua]
preload = ["lib/utils.lua"]
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_lua_manifest_missing_section() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "bad-lua"
version = "1.0.0"
type = "lua"
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }
}
