//! Saved search management for Docker log filtering.
//!
//! Persists named search patterns to `~/.ratterm/docker_logs/saved_searches.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A saved search pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    /// User-given name for the search.
    pub name: String,
    /// The search/filter pattern.
    pub pattern: String,
    /// Timestamp when the search was created (ISO 8601).
    pub created: String,
}

/// Manages a collection of saved searches.
#[derive(Debug, Clone)]
pub struct SearchManager {
    /// The saved searches.
    searches: Vec<SavedSearch>,
    /// Path to the storage file.
    storage_path: PathBuf,
}

impl SearchManager {
    /// Creates a new search manager with the default storage path.
    #[must_use]
    pub fn new() -> Self {
        let storage_path = Self::default_storage_path();
        Self {
            searches: Vec::new(),
            storage_path,
        }
    }

    /// Creates a search manager with a custom storage path (for testing).
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            searches: Vec::new(),
            storage_path: path,
        }
    }

    /// Returns the default storage path.
    fn default_storage_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("docker_logs")
            .join("saved_searches.json")
    }

    /// Adds a new saved search.
    pub fn add(&mut self, name: String, pattern: String) {
        assert!(!name.is_empty(), "search name must not be empty");
        assert!(!pattern.is_empty(), "search pattern must not be empty");

        let created = chrono::Utc::now().to_rfc3339();
        self.searches.push(SavedSearch {
            name,
            pattern,
            created,
        });
    }

    /// Removes a saved search by index.
    ///
    /// Returns `true` if the index was valid and the search was removed.
    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.searches.len() {
            self.searches.remove(index);
            true
        } else {
            false
        }
    }

    /// Returns a slice of all saved searches.
    #[must_use]
    pub fn list(&self) -> &[SavedSearch] {
        &self.searches
    }

    /// Returns the number of saved searches.
    #[must_use]
    pub fn len(&self) -> usize {
        self.searches.len()
    }

    /// Returns true if there are no saved searches.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.searches.is_empty()
    }

    /// Gets a search by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&SavedSearch> {
        self.searches.get(index)
    }

    /// Loads saved searches from disk.
    ///
    /// # Errors
    /// Returns error if the file exists but cannot be read or parsed.
    pub fn load(&mut self) -> Result<(), String> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.storage_path)
            .map_err(|e| format!("Failed to read saved searches: {}", e))?;

        self.searches = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse saved searches: {}", e))?;

        Ok(())
    }

    /// Saves all searches to disk.
    ///
    /// # Errors
    /// Returns error if the file cannot be written.
    pub fn save(&self) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(&self.searches)
            .map_err(|e| format!("Failed to serialize searches: {}", e))?;

        std::fs::write(&self.storage_path, json)
            .map_err(|e| format!("Failed to write saved searches: {}", e))?;

        Ok(())
    }
}

impl Default for SearchManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager(dir: &std::path::Path) -> SearchManager {
        SearchManager::with_path(dir.join("saved_searches.json"))
    }

    #[test]
    fn test_add_and_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);

        mgr.add("errors".to_string(), "ERROR".to_string());
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());

        let searches = mgr.list();
        assert_eq!(searches[0].name, "errors");
        assert_eq!(searches[0].pattern, "ERROR");
    }

    #[test]
    fn test_remove() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        mgr.add("first".to_string(), "pattern1".to_string());
        mgr.add("second".to_string(), "pattern2".to_string());
        assert_eq!(mgr.len(), 2);

        assert!(mgr.remove(0));
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.list()[0].name, "second");
    }

    #[test]
    fn test_remove_invalid_index() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        assert!(!mgr.remove(0));
        assert!(!mgr.remove(10));
    }

    #[test]
    fn test_get() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        mgr.add("test".to_string(), "pattern".to_string());
        assert!(mgr.get(0).is_some());
        assert_eq!(mgr.get(0).expect("exists").name, "test");
        assert!(mgr.get(1).is_none());
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("saved_searches.json");

        // Save
        {
            let mut mgr = SearchManager::with_path(path.clone());
            mgr.add("errors".to_string(), "ERROR".to_string());
            mgr.add("warnings".to_string(), "WARN".to_string());
            mgr.save().expect("save should succeed");
        }

        // Load
        {
            let mut mgr = SearchManager::with_path(path);
            mgr.load().expect("load should succeed");
            assert_eq!(mgr.len(), 2);
            assert_eq!(mgr.list()[0].name, "errors");
            assert_eq!(mgr.list()[1].name, "warnings");
        }
    }

    #[test]
    fn test_load_nonexistent_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        // Should succeed with empty list
        mgr.load().expect("load of nonexistent file should succeed");
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_created_timestamp_populated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        mgr.add("test".to_string(), "pattern".to_string());
        let search = mgr.get(0).expect("exists");
        assert!(!search.created.is_empty());
    }

    #[test]
    fn test_multiple_adds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mgr = test_manager(dir.path());

        for i in 0..5 {
            mgr.add(format!("search_{}", i), format!("pattern_{}", i));
        }
        assert_eq!(mgr.len(), 5);
    }
}
