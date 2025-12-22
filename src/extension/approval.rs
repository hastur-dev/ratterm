//! Extension approval management.
//!
//! Handles user consent for running API extensions. Extensions must be approved
//! before they can execute. Approvals are persisted to disk and checked on startup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// An approved extension entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalEntry {
    /// Extension name (unique identifier).
    pub name: String,
    /// Version that was approved - must match for approval to be valid.
    pub version: String,
    /// Unix timestamp when approved.
    pub approved_at: u64,
    /// Whether the extension is enabled.
    pub enabled: bool,
}

impl ApprovalEntry {
    /// Creates a new approval entry for the current time.
    #[must_use]
    pub fn new(name: String, version: String) -> Self {
        let approved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            name,
            version,
            approved_at,
            enabled: true,
        }
    }
}

/// Persistent storage for approved extensions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalStore {
    /// Map of extension name to approval entry.
    #[serde(default)]
    pub extensions: HashMap<String, ApprovalEntry>,
}

/// Manages extension approvals with persistent storage.
pub struct ApprovalManager {
    /// The approval store.
    store: ApprovalStore,
    /// Path to the approval file.
    path: PathBuf,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalManager {
    /// Creates a new approval manager, loading from disk if available.
    #[must_use]
    pub fn new() -> Self {
        let path = Self::default_path();
        let store = Self::load_from_path(&path).unwrap_or_default();

        Self { store, path }
    }

    /// Creates an approval manager with a custom path (for testing).
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        let store = Self::load_from_path(&path).unwrap_or_default();
        Self { store, path }
    }

    /// Returns the default approval file path (~/.ratterm/approved_extensions.toml).
    fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("approved_extensions.toml")
    }

    /// Loads the approval store from a file path.
    fn load_from_path(path: &PathBuf) -> io::Result<ApprovalStore> {
        if !path.exists() {
            return Ok(ApprovalStore::default());
        }

        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Saves the approval store to disk.
    fn save(&self) -> io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            toml::to_string_pretty(&self.store).map_err(io::Error::other)?;

        std::fs::write(&self.path, content)
    }

    /// Checks if an extension is approved for a specific version.
    ///
    /// Returns true only if:
    /// - The extension has been approved
    /// - The approved version matches the requested version
    /// - The extension is enabled
    #[must_use]
    pub fn is_approved(&self, name: &str, version: &str) -> bool {
        self.store
            .extensions
            .get(name)
            .is_some_and(|entry| entry.enabled && entry.version == version)
    }

    /// Checks if an extension has any approval (regardless of version).
    #[must_use]
    pub fn has_any_approval(&self, name: &str) -> bool {
        self.store.extensions.contains_key(name)
    }

    /// Gets the approved version for an extension, if any.
    #[must_use]
    pub fn approved_version(&self, name: &str) -> Option<&str> {
        self.store
            .extensions
            .get(name)
            .map(|entry| entry.version.as_str())
    }

    /// Approves an extension for a specific version.
    ///
    /// This creates or updates the approval entry and persists to disk.
    pub fn approve(&mut self, name: &str, version: &str) -> io::Result<()> {
        let entry = ApprovalEntry::new(name.to_string(), version.to_string());
        self.store.extensions.insert(name.to_string(), entry);
        self.save()
    }

    /// Revokes approval for an extension.
    ///
    /// This removes the extension from the approved list.
    pub fn revoke(&mut self, name: &str) -> io::Result<()> {
        self.store.extensions.remove(name);
        self.save()
    }

    /// Disables an approved extension without removing the approval.
    pub fn disable(&mut self, name: &str) -> io::Result<()> {
        if let Some(entry) = self.store.extensions.get_mut(name) {
            entry.enabled = false;
            self.save()?;
        }
        Ok(())
    }

    /// Enables a previously disabled extension.
    pub fn enable(&mut self, name: &str) -> io::Result<()> {
        if let Some(entry) = self.store.extensions.get_mut(name) {
            entry.enabled = true;
            self.save()?;
        }
        Ok(())
    }

    /// Lists all approved extensions.
    #[must_use]
    pub fn list(&self) -> Vec<&ApprovalEntry> {
        self.store.extensions.values().collect()
    }

    /// Gets a specific approval entry by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ApprovalEntry> {
        self.store.extensions.get(name)
    }

    /// Returns the number of approved extensions.
    #[must_use]
    pub fn count(&self) -> usize {
        self.store.extensions.len()
    }

    /// Checks if there are no approved extensions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.store.extensions.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_manager() -> (ApprovalManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("approvals.toml");
        let manager = ApprovalManager::with_path(path);
        (manager, dir)
    }

    #[test]
    fn test_new_manager_is_empty() {
        let (manager, _dir) = temp_manager();
        assert!(manager.is_empty());
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_approve_and_check() {
        let (mut manager, _dir) = temp_manager();

        // Not approved initially
        assert!(!manager.is_approved("test-ext", "1.0.0"));

        // Approve
        manager.approve("test-ext", "1.0.0").unwrap();

        // Now approved for that version
        assert!(manager.is_approved("test-ext", "1.0.0"));

        // Not approved for different version
        assert!(!manager.is_approved("test-ext", "2.0.0"));
    }

    #[test]
    fn test_version_change_requires_reapproval() {
        let (mut manager, _dir) = temp_manager();

        manager.approve("test-ext", "1.0.0").unwrap();
        assert!(manager.is_approved("test-ext", "1.0.0"));

        // Version 1.0.1 is not approved
        assert!(!manager.is_approved("test-ext", "1.0.1"));

        // Approve new version
        manager.approve("test-ext", "1.0.1").unwrap();
        assert!(manager.is_approved("test-ext", "1.0.1"));

        // Old version is now overwritten
        assert!(!manager.is_approved("test-ext", "1.0.0"));
    }

    #[test]
    fn test_revoke() {
        let (mut manager, _dir) = temp_manager();

        manager.approve("test-ext", "1.0.0").unwrap();
        assert!(manager.is_approved("test-ext", "1.0.0"));

        manager.revoke("test-ext").unwrap();
        assert!(!manager.is_approved("test-ext", "1.0.0"));
        assert!(!manager.has_any_approval("test-ext"));
    }

    #[test]
    fn test_disable_enable() {
        let (mut manager, _dir) = temp_manager();

        manager.approve("test-ext", "1.0.0").unwrap();
        assert!(manager.is_approved("test-ext", "1.0.0"));

        // Disable keeps the approval but marks as disabled
        manager.disable("test-ext").unwrap();
        assert!(!manager.is_approved("test-ext", "1.0.0"));
        assert!(manager.has_any_approval("test-ext"));

        // Re-enable
        manager.enable("test-ext").unwrap();
        assert!(manager.is_approved("test-ext", "1.0.0"));
    }

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("approvals.toml");

        // Create and approve
        {
            let mut manager = ApprovalManager::with_path(path.clone());
            manager.approve("test-ext", "1.0.0").unwrap();
        }

        // Load fresh and verify
        {
            let manager = ApprovalManager::with_path(path);
            assert!(manager.is_approved("test-ext", "1.0.0"));
        }
    }

    #[test]
    fn test_list() {
        let (mut manager, _dir) = temp_manager();

        manager.approve("ext-a", "1.0.0").unwrap();
        manager.approve("ext-b", "2.0.0").unwrap();

        let list = manager.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_approved_version() {
        let (mut manager, _dir) = temp_manager();

        assert!(manager.approved_version("test-ext").is_none());

        manager.approve("test-ext", "1.0.0").unwrap();
        assert_eq!(manager.approved_version("test-ext"), Some("1.0.0"));
    }
}
