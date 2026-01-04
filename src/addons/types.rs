//! Add-on type definitions.
//!
//! Core data structures for the add-ons system.
//! Focused on install/uninstall functionality only.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of installed addons.
pub const MAX_INSTALLED_ADDONS: usize = 50;

/// Metadata from addon's config.yaml.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonMetadata {
    /// Display name (overrides derived name).
    #[serde(default)]
    pub name: Option<String>,
    /// Description (overrides README-based description).
    #[serde(default)]
    pub description: Option<String>,
    /// Commands to detect if already installed (e.g., ["vim", "nvim"]).
    #[serde(default)]
    pub detect_commands: Vec<String>,
}

/// Represents a technology available in the GitHub repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addon {
    /// Directory name in the repo (e.g., "nodejs", "python").
    pub id: String,
    /// Human-readable name (derived from id or config.yaml).
    pub name: String,
    /// Description (from config.yaml or README.md).
    pub description: String,
    /// Whether platform-specific install script exists.
    pub has_install: bool,
    /// Metadata from config.yaml.
    pub metadata: Option<AddonMetadata>,
}

impl Addon {
    /// Creates a new addon with the given ID.
    #[must_use]
    pub fn new(id: String) -> Self {
        assert!(!id.is_empty(), "Addon ID must not be empty");
        let name = id
            .replace(['-', '_'], " ")
            .split_whitespace()
            .map(capitalize_first)
            .collect::<Vec<_>>()
            .join(" ");

        Self {
            id,
            name,
            description: String::new(),
            has_install: false,
            metadata: None,
        }
    }

    /// Sets the metadata from config.yaml.
    #[must_use]
    pub fn with_metadata(mut self, metadata: AddonMetadata) -> Self {
        // Override name and description if provided in metadata
        if let Some(ref name) = metadata.name {
            self.name = name.clone();
        }
        if let Some(ref desc) = metadata.description {
            self.description = desc.clone();
        }
        self.metadata = Some(metadata);
        self
    }

    /// Returns the detection commands for this addon.
    #[must_use]
    pub fn detect_commands(&self) -> &[String] {
        self.metadata
            .as_ref()
            .map(|m| m.detect_commands.as_slice())
            .unwrap_or(&[])
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: String) -> Self {
        assert!(description.len() <= 500, "Description too long");
        self.description = description;
        self
    }

    /// Sets whether install script exists.
    #[must_use]
    pub fn with_install(mut self, has_install: bool) -> Self {
        self.has_install = has_install;
        self
    }

    /// Returns true if this addon can be installed on the current platform.
    #[must_use]
    pub fn is_installable(&self) -> bool {
        self.has_install
    }
}

/// An installed addon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledAddon {
    /// Addon ID (directory name).
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Installation timestamp (Unix epoch seconds).
    pub installed_at: u64,
}

impl InstalledAddon {
    /// Creates a new installed addon.
    #[must_use]
    pub fn new(id: String, display_name: String) -> Self {
        assert!(!id.is_empty(), "Addon ID must not be empty");
        assert!(!display_name.is_empty(), "Display name must not be empty");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id,
            display_name,
            installed_at: now,
        }
    }
}

/// Configuration for the addons system.
#[derive(Debug, Clone, Default)]
pub struct AddonConfig {
    /// GitHub repository for fetching addons (owner/repo format).
    pub repository: String,
    /// Branch to use (default: "main").
    pub branch: String,
    /// List of installed addons.
    pub installed: Vec<InstalledAddon>,
}

impl AddonConfig {
    /// Creates a new addon config with default repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            repository: String::from("hastur-dev/ratterm-installer"),
            branch: String::from("main"),
            installed: Vec::new(),
        }
    }

    /// Sets the repository.
    pub fn set_repository(&mut self, repo: String) {
        assert!(!repo.is_empty(), "Repository must not be empty");
        assert!(repo.contains('/'), "Repository must be in owner/repo format");
        self.repository = repo;
    }

    /// Sets the branch.
    pub fn set_branch(&mut self, branch: String) {
        assert!(!branch.is_empty(), "Branch must not be empty");
        self.branch = branch;
    }

    /// Adds an installed addon.
    pub fn add_installed(&mut self, addon: InstalledAddon) {
        assert!(
            self.installed.len() < MAX_INSTALLED_ADDONS,
            "Maximum addon limit reached"
        );

        // Remove existing addon with same ID if present
        self.installed.retain(|a| a.id != addon.id);
        self.installed.push(addon);
    }

    /// Removes an installed addon by ID.
    pub fn remove_installed(&mut self, addon_id: &str) {
        self.installed.retain(|a| a.id != addon_id);
    }

    /// Gets an installed addon by ID.
    #[must_use]
    pub fn get_installed(&self, addon_id: &str) -> Option<&InstalledAddon> {
        self.installed.iter().find(|a| a.id == addon_id)
    }

    /// Returns true if an addon with the given ID is installed.
    #[must_use]
    pub fn is_installed(&self, addon_id: &str) -> bool {
        self.installed.iter().any(|a| a.id == addon_id)
    }
}

/// Script type for addons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    /// Installation script.
    Install,
}

impl ScriptType {
    /// Returns the script filename for the current platform.
    #[must_use]
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Install => {
                #[cfg(windows)]
                {
                    "install-windows.ps1"
                }
                #[cfg(target_os = "macos")]
                {
                    "install-macos.sh"
                }
                #[cfg(all(not(windows), not(target_os = "macos")))]
                {
                    "install-linux.sh"
                }
            }
        }
    }

    /// Returns the script prefix.
    #[must_use]
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Install => "install-",
        }
    }
}

/// Add-on error types.
#[derive(Debug, Clone)]
pub enum AddonError {
    /// Network error during GitHub API call.
    NetworkError(String),
    /// GitHub API rate limit exceeded.
    RateLimitExceeded,
    /// Repository not found.
    RepositoryNotFound,
    /// Addon not found in repository.
    AddonNotFound(String),
    /// Script not found for current platform.
    ScriptNotFound(String, ScriptType),
    /// Script execution failed.
    ExecutionFailed(String),
    /// Configuration error.
    ConfigError(String),
    /// Maximum addons limit reached.
    LimitReached,
}

impl std::fmt::Display for AddonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::RateLimitExceeded => write!(f, "GitHub API rate limit exceeded"),
            Self::RepositoryNotFound => write!(f, "Add-on repository not found"),
            Self::AddonNotFound(id) => write!(f, "Add-on '{}' not found", id),
            Self::ScriptNotFound(id, script_type) => {
                write!(f, "{:?} script not found for '{}'", script_type, id)
            }
            Self::ExecutionFailed(msg) => write!(f, "Script execution failed: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Self::LimitReached => write!(f, "Maximum number of add-ons reached"),
        }
    }
}

impl std::error::Error for AddonError {}

/// Capitalizes the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addon_new() {
        let addon = Addon::new("node-js".to_string());
        assert_eq!(addon.id, "node-js");
        assert_eq!(addon.name, "Node Js");
        assert!(!addon.has_install);
    }

    #[test]
    fn test_addon_builder() {
        let addon = Addon::new("python".to_string())
            .with_description("Python runtime".to_string())
            .with_install(true);

        assert_eq!(addon.description, "Python runtime");
        assert!(addon.is_installable());
    }

    #[test]
    fn test_installed_addon() {
        let addon = InstalledAddon::new("nodejs".to_string(), "Node.js".to_string());
        assert!(addon.installed_at > 0);
    }

    #[test]
    fn test_addon_config() {
        let mut config = AddonConfig::new();
        assert_eq!(config.repository, "hastur-dev/ratterm-installer");
        assert_eq!(config.branch, "main");

        let addon = InstalledAddon::new("test".to_string(), "Test".to_string());
        config.add_installed(addon);
        assert!(config.is_installed("test"));
        assert!(!config.is_installed("other"));

        config.remove_installed("test");
        assert!(!config.is_installed("test"));
    }

    #[test]
    fn test_script_type_filename() {
        let install = ScriptType::Install.filename();

        #[cfg(windows)]
        {
            assert_eq!(install, "install-windows.ps1");
        }
        #[cfg(target_os = "macos")]
        {
            assert_eq!(install, "install-macos.sh");
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            assert_eq!(install, "install-linux.sh");
        }
    }
}
