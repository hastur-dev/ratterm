//! Add-on storage for .ratrc persistence.
//!
//! Handles reading and writing addon configuration to the .ratrc file.

use super::types::{AddonConfig, InstalledAddon, MAX_INSTALLED_ADDONS};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

/// Storage manager for addon configuration.
#[derive(Debug)]
pub struct AddonStorage {
    /// Path to the .ratrc file.
    config_path: PathBuf,
}

impl AddonStorage {
    /// Creates a new addon storage with the given config path.
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    /// Loads addon configuration from the .ratrc file.
    ///
    /// # Returns
    ///
    /// The loaded addon configuration, or defaults if not found.
    pub fn load(&self) -> AddonConfig {
        let mut config = AddonConfig::new();

        let file = match fs::File::open(&self.config_path) {
            Ok(f) => f,
            Err(_) => return config,
        };

        let reader = BufReader::new(file);
        let mut line_count = 0;
        const MAX_LINES: usize = 1000;

        for line in reader.lines() {
            line_count += 1;
            if line_count > MAX_LINES {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key = value
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.split('#').next().unwrap_or("").trim();

                self.apply_setting(&mut config, key, value);
            }
        }

        config
    }

    /// Applies a single setting to the config.
    fn apply_setting(&self, config: &mut AddonConfig, key: &str, value: &str) {
        assert!(!key.is_empty(), "Key must not be empty");

        match key {
            "addon_repository" => {
                if !value.is_empty() && value.contains('/') {
                    config.repository = value.to_string();
                }
            }
            "addon_branch" => {
                if !value.is_empty() {
                    config.branch = value.to_string();
                }
            }
            _ if key.starts_with("addon.") => {
                if config.installed.len() >= MAX_INSTALLED_ADDONS {
                    return;
                }

                let addon_id = key.strip_prefix("addon.").unwrap_or("");
                if addon_id.is_empty() {
                    return;
                }

                // Value can be "installed" or empty - we just track that it exists
                let addon = InstalledAddon::new(
                    addon_id.to_string(),
                    capitalize_addon_name(addon_id),
                );

                config.add_installed(addon);
            }
            _ => {}
        }
    }

    /// Saves an installed addon to the .ratrc file.
    ///
    /// Updates the existing entry or adds a new one.
    pub fn save_addon(&self, addon: &InstalledAddon) -> io::Result<()> {
        assert!(!addon.id.is_empty(), "Addon ID must not be empty");

        let key = format!("addon.{}", addon.id);
        self.update_setting(&key, "installed")
    }

    /// Removes an addon from the .ratrc file.
    pub fn remove_addon(&self, addon_id: &str) -> io::Result<()> {
        assert!(!addon_id.is_empty(), "Addon ID must not be empty");

        let key = format!("addon.{}", addon_id);
        self.remove_setting(&key)
    }

    /// Saves the repository setting.
    pub fn save_repository(&self, repository: &str) -> io::Result<()> {
        assert!(!repository.is_empty(), "Repository must not be empty");
        assert!(
            repository.contains('/'),
            "Repository must be in owner/repo format"
        );

        self.update_setting("addon_repository", repository)
    }

    /// Saves the branch setting.
    pub fn save_branch(&self, branch: &str) -> io::Result<()> {
        assert!(!branch.is_empty(), "Branch must not be empty");

        self.update_setting("addon_branch", branch)
    }

    /// Updates a single setting in the config file.
    fn update_setting(&self, key: &str, value: &str) -> io::Result<()> {
        let content = fs::read_to_string(&self.config_path).unwrap_or_default();

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let target_key = format!("{} =", key);
        let target_key_nospace = format!("{}=", key);

        // Find and update existing line, or add new one
        let mut found = false;
        const MAX_LINE_SEARCH: usize = 1000;

        for (i, line) in lines.iter_mut().enumerate() {
            if i >= MAX_LINE_SEARCH {
                break;
            }

            let trimmed = line.trim();
            if trimmed.starts_with(&target_key) || trimmed.starts_with(&target_key_nospace) {
                *line = format!("{} = {}", key, value);
                found = true;
                break;
            }
        }

        if !found {
            // Add new line at the end
            lines.push(format!("{} = {}", key, value));
        }

        // Write back
        let mut file = fs::File::create(&self.config_path)?;
        for line in &lines {
            writeln!(file, "{}", line)?;
        }

        Ok(())
    }

    /// Removes a setting from the config file.
    fn remove_setting(&self, key: &str) -> io::Result<()> {
        let content = fs::read_to_string(&self.config_path).unwrap_or_default();

        let target_key = format!("{} =", key);
        let target_key_nospace = format!("{}=", key);

        let lines: Vec<&str> = content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with(&target_key) && !trimmed.starts_with(&target_key_nospace)
            })
            .collect();

        let mut file = fs::File::create(&self.config_path)?;
        for line in &lines {
            writeln!(file, "{}", line)?;
        }

        Ok(())
    }
}

/// Capitalizes an addon ID into a display name.
fn capitalize_addon_name(id: &str) -> String {
    id.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_capitalize_addon_name() {
        assert_eq!(capitalize_addon_name("node-js"), "Node Js");
        assert_eq!(capitalize_addon_name("python_runtime"), "Python Runtime");
        assert_eq!(capitalize_addon_name("rust"), "Rust");
    }

    #[test]
    fn test_storage_load() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "addon_repository = custom/repo").unwrap();
        writeln!(file, "addon_branch = dev").unwrap();
        writeln!(file, "addon.nodejs = installed").unwrap();
        writeln!(file, "addon.python = installed").unwrap();
        file.flush().unwrap();

        let storage = AddonStorage::new(file.path().to_path_buf());
        let config = storage.load();

        assert_eq!(config.repository, "custom/repo");
        assert_eq!(config.branch, "dev");
        assert_eq!(config.installed.len(), 2);
        assert!(config.is_installed("nodejs"));
        assert!(config.is_installed("python"));
    }

    #[test]
    fn test_storage_save_addon() {
        let file = NamedTempFile::new().unwrap();
        let storage = AddonStorage::new(file.path().to_path_buf());

        let addon = InstalledAddon::new("test".to_string(), "Test".to_string());
        storage.save_addon(&addon).unwrap();

        let content = fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("addon.test = installed"));
    }

    #[test]
    fn test_storage_remove_addon() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "addon.nodejs = installed").unwrap();
        writeln!(file, "addon.python = installed").unwrap();
        file.flush().unwrap();

        let storage = AddonStorage::new(file.path().to_path_buf());
        storage.remove_addon("nodejs").unwrap();

        let content = fs::read_to_string(file.path()).unwrap();
        assert!(!content.contains("addon.nodejs"));
        assert!(content.contains("addon.python"));
    }
}
