//! Docker settings and quick-connect storage.
//!
//! Persists Docker quick-connect assignments and settings to
//! `~/.ratterm/docker_items.toml`.

use super::container::DockerItemList;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use thiserror::Error;

/// Maximum file size for Docker settings file (512KB).
const MAX_FILE_SIZE: u64 = 512 * 1024;

/// Storage errors.
#[derive(Debug, Error)]
pub enum DockerStorageError {
    /// File I/O error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// TOML parsing error.
    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),

    /// TOML serialization error.
    #[error("Serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),

    /// File too large.
    #[error("File too large (max {MAX_FILE_SIZE} bytes)")]
    FileTooLarge,
}

/// Docker storage manager.
///
/// Handles loading and saving Docker settings and quick-connect assignments.
#[derive(Debug)]
pub struct DockerStorage {
    /// Path to the storage file.
    path: PathBuf,
    /// Whether the storage has been initialized.
    initialized: bool,
}

impl DockerStorage {
    /// Creates a new storage manager with the default path.
    ///
    /// Default path: `~/.ratterm/docker_items.toml`
    #[must_use]
    pub fn new() -> Self {
        let path = Self::default_path();
        Self {
            path,
            initialized: false,
        }
    }

    /// Creates a storage manager with a custom path.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        assert!(!path.as_os_str().is_empty(), "path must not be empty");

        Self {
            path,
            initialized: false,
        }
    }

    /// Returns the default storage path.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("docker_items.toml")
    }

    /// Returns true if the storage file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Returns true if storage has been initialized.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns the storage file path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Loads the Docker item list from storage.
    pub fn load(&mut self) -> Result<DockerItemList, DockerStorageError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // If file doesn't exist, return default list
        if !self.path.exists() {
            self.initialized = true;
            return Ok(DockerItemList::new());
        }

        // Check file size
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(DockerStorageError::FileTooLarge);
        }

        // Read and parse
        let content = fs::read_to_string(&self.path)?;
        let items: DockerItemList = toml::from_str(&content)?;

        self.initialized = true;
        Ok(items)
    }

    /// Saves the Docker item list to storage.
    pub fn save(&self, items: &DockerItemList) -> Result<(), DockerStorageError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to TOML
        let content = toml::to_string_pretty(items)?;

        // Write atomically (write to temp, then rename)
        let temp_path = self.path.with_extension("tmp");

        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
        }

        fs::rename(&temp_path, &self.path)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&self.path, perms);
        }

        Ok(())
    }

    /// Saves a quick-connect assignment.
    pub fn save_quick_connect(
        &mut self,
        items: &mut DockerItemList,
        index: usize,
        item: super::container::DockerQuickConnectItem,
    ) -> Result<(), DockerStorageError> {
        assert!(index < 9, "index must be 0-8");

        items.set_quick_connect(index, item);
        self.save(items)
    }

    /// Removes a quick-connect assignment.
    pub fn remove_quick_connect(
        &mut self,
        items: &mut DockerItemList,
        index: usize,
    ) -> Result<(), DockerStorageError> {
        assert!(index < 9, "index must be 0-8");

        items.remove_quick_connect(index);
        self.save(items)
    }

    /// Updates the default shell setting.
    pub fn set_default_shell(
        &mut self,
        items: &mut DockerItemList,
        shell: String,
    ) -> Result<(), DockerStorageError> {
        assert!(!shell.is_empty(), "shell must not be empty");

        items.default_shell = shell;
        self.save(items)
    }
}

impl Default for DockerStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::docker::container::{DockerItemType, DockerQuickConnectItem};
    use tempfile::NamedTempFile;

    #[test]
    fn test_storage_default_path() {
        let path = DockerStorage::default_path();
        assert!(path.to_string_lossy().contains("docker_items.toml"));
    }

    #[test]
    fn test_storage_save_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Create and save
        let storage = DockerStorage::with_path(path.clone());
        let mut items = DockerItemList::new();

        let qc_item = DockerQuickConnectItem {
            item_type: DockerItemType::RunningContainer,
            id: "abc123".to_string(),
            name: "my-nginx".to_string(),
        };
        items.set_quick_connect(0, qc_item);

        storage.save(&items).unwrap();

        // Load and verify
        let mut storage2 = DockerStorage::with_path(path);
        let loaded = storage2.load().unwrap();

        let retrieved = loaded.get_quick_connect(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "abc123");
        assert_eq!(retrieved.unwrap().name, "my-nginx");
    }

    #[test]
    fn test_storage_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent.toml");

        let mut storage = DockerStorage::with_path(path);
        let items = storage.load().unwrap();

        assert_eq!(items.quick_connect_count(), 0);
        assert_eq!(items.default_shell, "/bin/sh");
    }

    #[test]
    fn test_storage_initialized() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("docker.toml");

        let mut storage = DockerStorage::with_path(path);
        assert!(!storage.is_initialized());

        let _ = storage.load();
        assert!(storage.is_initialized());
    }
}
