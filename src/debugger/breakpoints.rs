//! Breakpoint persistence and management.
//!
//! Stores breakpoints per file and persists them to `.ratterm/breakpoints.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Persistent breakpoint storage.
///
/// Maps file paths to their set breakpoint line numbers.
/// Saves to and loads from `.ratterm/breakpoints.json` in the project root.
#[derive(Debug, Clone, Default)]
pub struct BreakpointStore {
    /// Breakpoints keyed by absolute file path.
    breakpoints: HashMap<String, Vec<u32>>,
    /// Project root directory (for persistence).
    project_root: Option<PathBuf>,
}

/// On-disk format for breakpoint persistence.
#[derive(Serialize, Deserialize)]
struct BreakpointFile {
    breakpoints: HashMap<String, Vec<u32>>,
}

impl BreakpointStore {
    /// Creates a new empty breakpoint store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            project_root: None,
        }
    }

    /// Creates a store with a project root for persistence.
    #[must_use]
    pub fn with_project_root(root: PathBuf) -> Self {
        let mut store = Self::new();
        store.project_root = Some(root);
        store.load();
        store
    }

    /// Toggles a breakpoint on the given line. Returns true if added, false if removed.
    pub fn toggle(&mut self, file: &str, line: u32) -> bool {
        assert!(!file.is_empty(), "File path must not be empty");
        assert!(line > 0, "Line numbers are 1-based");

        let lines = self.breakpoints.entry(file.to_string()).or_default();
        if let Some(pos) = lines.iter().position(|&l| l == line) {
            lines.remove(pos);
            if lines.is_empty() {
                self.breakpoints.remove(file);
            }
            self.save();
            false
        } else {
            lines.push(line);
            lines.sort_unstable();
            self.save();
            true
        }
    }

    /// Adds a breakpoint. Returns true if it was newly added.
    pub fn add(&mut self, file: &str, line: u32) -> bool {
        assert!(!file.is_empty(), "File path must not be empty");
        assert!(line > 0, "Line numbers are 1-based");

        let lines = self.breakpoints.entry(file.to_string()).or_default();
        if lines.contains(&line) {
            return false;
        }
        lines.push(line);
        lines.sort_unstable();
        self.save();
        true
    }

    /// Removes a breakpoint. Returns true if it was present.
    pub fn remove(&mut self, file: &str, line: u32) -> bool {
        assert!(!file.is_empty(), "File path must not be empty");

        let Some(lines) = self.breakpoints.get_mut(file) else {
            return false;
        };

        let Some(pos) = lines.iter().position(|&l| l == line) else {
            return false;
        };

        lines.remove(pos);
        if lines.is_empty() {
            self.breakpoints.remove(file);
        }
        self.save();
        true
    }

    /// Returns breakpoint lines for a given file.
    #[must_use]
    pub fn get(&self, file: &str) -> &[u32] {
        self.breakpoints.get(file).map_or(&[], Vec::as_slice)
    }

    /// Returns true if there is a breakpoint at the given file and line.
    #[must_use]
    pub fn has_breakpoint(&self, file: &str, line: u32) -> bool {
        self.breakpoints
            .get(file)
            .map_or(false, |lines| lines.contains(&line))
    }

    /// Returns all breakpoints as (file, lines) pairs.
    #[must_use]
    pub fn all(&self) -> &HashMap<String, Vec<u32>> {
        &self.breakpoints
    }

    /// Returns the total number of breakpoints across all files.
    #[must_use]
    pub fn count(&self) -> usize {
        self.breakpoints.values().map(Vec::len).sum()
    }

    /// Clears all breakpoints.
    pub fn clear(&mut self) {
        self.breakpoints.clear();
        self.save();
    }

    /// Persistence path: `<project_root>/.ratterm/breakpoints.json`.
    fn persistence_path(&self) -> Option<PathBuf> {
        self.project_root
            .as_ref()
            .map(|root| root.join(".ratterm").join("breakpoints.json"))
    }

    /// Saves breakpoints to disk.
    fn save(&self) {
        let Some(path) = self.persistence_path() else {
            return;
        };

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = BreakpointFile {
            breakpoints: self.breakpoints.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&file) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Loads breakpoints from disk.
    fn load(&mut self) {
        let Some(path) = self.persistence_path() else {
            return;
        };

        if !path.exists() {
            return;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            return;
        };

        if let Ok(file) = serde_json::from_str::<BreakpointFile>(&content) {
            self.breakpoints = file.breakpoints;
        }
    }

    /// Reloads breakpoints from disk (public interface for testing / startup).
    pub fn reload(&mut self) {
        self.load();
    }

    /// Sets the project root and reloads.
    pub fn set_project_root(&mut self, root: &Path) {
        self.project_root = Some(root.to_path_buf());
        self.load();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_store_is_empty() {
        let store = BreakpointStore::new();
        assert_eq!(store.count(), 0);
        assert!(store.all().is_empty());
    }

    #[test]
    fn test_add_breakpoint() {
        let mut store = BreakpointStore::new();
        assert!(store.add("src/main.rs", 10));
        assert!(store.has_breakpoint("src/main.rs", 10));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_add_duplicate_returns_false() {
        let mut store = BreakpointStore::new();
        assert!(store.add("src/main.rs", 10));
        assert!(!store.add("src/main.rs", 10));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_remove_breakpoint() {
        let mut store = BreakpointStore::new();
        store.add("src/main.rs", 10);
        assert!(store.remove("src/main.rs", 10));
        assert!(!store.has_breakpoint("src/main.rs", 10));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_returns_false() {
        let mut store = BreakpointStore::new();
        assert!(!store.remove("src/main.rs", 10));
    }

    #[test]
    fn test_toggle_adds_then_removes() {
        let mut store = BreakpointStore::new();
        assert!(store.toggle("src/main.rs", 5)); // Added
        assert!(store.has_breakpoint("src/main.rs", 5));

        assert!(!store.toggle("src/main.rs", 5)); // Removed
        assert!(!store.has_breakpoint("src/main.rs", 5));
    }

    #[test]
    fn test_get_returns_sorted_lines() {
        let mut store = BreakpointStore::new();
        store.add("src/main.rs", 20);
        store.add("src/main.rs", 5);
        store.add("src/main.rs", 10);

        let lines = store.get("src/main.rs");
        assert_eq!(lines, &[5, 10, 20]);
    }

    #[test]
    fn test_get_empty_file() {
        let store = BreakpointStore::new();
        assert!(store.get("nonexistent.rs").is_empty());
    }

    #[test]
    fn test_multiple_files() {
        let mut store = BreakpointStore::new();
        store.add("a.rs", 1);
        store.add("b.rs", 2);
        store.add("a.rs", 3);

        assert_eq!(store.count(), 3);
        assert_eq!(store.get("a.rs"), &[1, 3]);
        assert_eq!(store.get("b.rs"), &[2]);
    }

    #[test]
    fn test_clear() {
        let mut store = BreakpointStore::new();
        store.add("a.rs", 1);
        store.add("b.rs", 2);
        store.clear();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_persist_and_reload() {
        let tmp = TempDir::new().expect("create temp dir");
        let root = tmp.path().to_path_buf();

        // Save breakpoints
        {
            let mut store = BreakpointStore::with_project_root(root.clone());
            store.add("src/main.rs", 10);
            store.add("src/main.rs", 20);
            store.add("src/lib.rs", 5);
        }

        // Reload from disk
        let store = BreakpointStore::with_project_root(root);
        assert_eq!(store.count(), 3);
        assert_eq!(store.get("src/main.rs"), &[10, 20]);
        assert_eq!(store.get("src/lib.rs"), &[5]);
    }

    #[test]
    fn test_reload_nonexistent_file() {
        let tmp = TempDir::new().expect("create temp dir");
        let store = BreakpointStore::with_project_root(tmp.path().to_path_buf());
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_remove_cleans_up_empty_file_entry() {
        let mut store = BreakpointStore::new();
        store.add("a.rs", 1);
        store.remove("a.rs", 1);
        assert!(!store.all().contains_key("a.rs"));
    }
}
