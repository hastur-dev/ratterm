//! Extension manifest parsing.
//!
//! Parses `extension.toml` files that define extension metadata and configuration.
//! All extensions are now API-based, running as external processes that communicate
//! via REST API.

use std::collections::HashMap;
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
    /// Process configuration (how to run the extension).
    #[serde(alias = "api")]
    pub process: Option<ProcessConfig>,
    /// Default hotkey configuration.
    #[serde(default)]
    pub hotkey: Option<HotkeyConfig>,
}

/// Default hotkey configuration for an extension.
#[derive(Debug, Clone, Deserialize)]
pub struct HotkeyConfig {
    /// Default hotkey (e.g., "f3", "ctrl+shift+e").
    pub default: String,
    /// Description of what the hotkey does.
    #[serde(default)]
    pub description: String,
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
}

/// Compatibility requirements.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompatibilityInfo {
    /// Minimum ratterm version (semver range).
    #[serde(default)]
    pub ratterm: String,
}

/// Process configuration for API extensions.
///
/// Defines how to run the extension as an external process that communicates
/// with ratterm via REST API.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessConfig {
    /// Command to execute (e.g., "python", "node", "{ext_dir}/myext").
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory (default: extension directory).
    /// Supports {ext_dir} placeholder.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Environment variables to pass to the process.
    /// Values support {ext_dir} placeholder.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Required capabilities (for documentation/filtering).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Whether to restart on crash.
    #[serde(default = "default_restart_on_crash")]
    pub restart_on_crash: bool,
    /// Maximum number of restarts before giving up.
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    /// Delay between restarts in milliseconds.
    #[serde(default = "default_restart_delay_ms")]
    pub restart_delay_ms: u64,
}

fn default_restart_on_crash() -> bool {
    true
}

fn default_max_restarts() -> u32 {
    3
}

fn default_restart_delay_ms() -> u64 {
    1000
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            capabilities: Vec::new(),
            restart_on_crash: default_restart_on_crash(),
            max_restarts: default_max_restarts(),
            restart_delay_ms: default_restart_delay_ms(),
        }
    }
}

impl ExtensionManifest {
    /// Returns the extension name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.extension.name
    }

    /// Returns the extension version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.extension.version
    }

    /// Returns the extension author.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        if self.extension.author.is_empty() {
            None
        } else {
            Some(&self.extension.author)
        }
    }

    /// Returns the extension description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        if self.extension.description.is_empty() {
            None
        } else {
            Some(&self.extension.description)
        }
    }

    /// Returns the process command to run.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        self.process.as_ref().map(|p| p.command.as_str())
    }

    /// Returns the default hotkey configuration if specified.
    #[must_use]
    pub fn default_hotkey(&self) -> Option<&str> {
        self.hotkey.as_ref().map(|h| h.default.as_str())
    }

    /// Returns the hotkey description if specified.
    #[must_use]
    pub fn hotkey_description(&self) -> Option<&str> {
        self.hotkey.as_ref().and_then(|h| {
            if h.description.is_empty() {
                None
            } else {
                Some(h.description.as_str())
            }
        })
    }
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

    // Process config is required and must have a command
    match &manifest.process {
        Some(process_config) => {
            if process_config.command.is_empty() {
                return Err(ExtensionError::Manifest(
                    "Extensions require 'command' field in [process] section".to_string(),
                ));
            }
        }
        None => {
            return Err(ExtensionError::Manifest(
                "Extensions require [process] section with command to run".to_string(),
            ));
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
    fn test_parse_process_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "my-extension"
version = "1.0.0"
description = "A nice extension"
author = "test"
license = "MIT"

[process]
command = "python"
args = ["{ext_dir}/main.py"]
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.extension.name, "my-extension");
        assert_eq!(manifest.extension.version, "1.0.0");
        assert!(manifest.process.is_some());
        let process = manifest.process.as_ref().expect("process config");
        assert_eq!(process.command, "python");
        assert_eq!(process.args.len(), 1);
    }

    #[test]
    fn test_parse_api_alias() {
        // Test that [api] section works as alias for [process]
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "my-extension"
version = "1.0.0"

[api]
command = "node"
args = ["index.js"]
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert!(manifest.process.is_some());
        let process = manifest.process.as_ref().expect("process config");
        assert_eq!(process.command, "node");
    }

    #[test]
    fn test_full_process_config() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "full-config"
version = "2.0.0"
description = "Extension with full config"
author = "developer"
license = "GPL-3.0"
homepage = "https://github.com/dev/ext"

[compatibility]
ratterm = ">=1.0.0"

[process]
command = "{ext_dir}/bin/myext"
args = ["--config", "{ext_dir}/config.yaml"]
cwd = "{ext_dir}"
restart_on_crash = true
max_restarts = 5
restart_delay_ms = 2000
capabilities = ["commands", "formatting"]

[process.env]
MY_VAR = "value"
EXT_DIR = "{ext_dir}"
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.extension.name, "full-config");
        assert_eq!(manifest.compatibility.ratterm, ">=1.0.0");

        let process = manifest.process.as_ref().expect("process config");
        assert_eq!(process.command, "{ext_dir}/bin/myext");
        assert_eq!(process.args.len(), 2);
        assert_eq!(process.cwd, Some("{ext_dir}".to_string()));
        assert!(process.restart_on_crash);
        assert_eq!(process.max_restarts, 5);
        assert_eq!(process.restart_delay_ms, 2000);
        assert_eq!(process.capabilities.len(), 2);
        assert_eq!(process.env.get("MY_VAR"), Some(&"value".to_string()));
    }

    #[test]
    fn test_invalid_manifest_missing_name() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = ""
version = "1.0.0"

[process]
command = "python"
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_manifest_missing_version() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "test"
version = ""

[process]
command = "python"
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_manifest_missing_process() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "no-process"
version = "1.0.0"
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_manifest_empty_command() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "empty-command"
version = "1.0.0"

[process]
command = ""
"##;

        let path = create_manifest(dir.path(), content);
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_process_config_values() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "minimal"
version = "1.0.0"

[process]
command = "myext"
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        let process = manifest.process.as_ref().expect("process config");
        assert!(process.restart_on_crash);
        assert_eq!(process.max_restarts, 3);
        assert_eq!(process.restart_delay_ms, 1000);
        assert!(process.args.is_empty());
        assert!(process.env.is_empty());
        assert!(process.cwd.is_none());
    }

    #[test]
    fn test_manifest_helper_methods() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "helpers-test"
version = "1.2.3"
description = "Test description"
author = "Test Author"

[process]
command = "test-cmd"
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.name(), "helpers-test");
        assert_eq!(manifest.version(), "1.2.3");
        assert_eq!(manifest.author(), Some("Test Author"));
        assert_eq!(manifest.description(), Some("Test description"));
        assert_eq!(manifest.command(), Some("test-cmd"));
    }

    #[test]
    fn test_manifest_empty_optional_fields() {
        let dir = TempDir::new().expect("temp dir");
        let content = r##"
[extension]
name = "minimal"
version = "1.0.0"

[process]
command = "cmd"
"##;

        let path = create_manifest(dir.path(), content);
        let manifest = load_manifest(&path).expect("parse manifest");

        assert_eq!(manifest.author(), None);
        assert_eq!(manifest.description(), None);
    }
}
