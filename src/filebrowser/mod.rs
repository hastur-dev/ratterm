//! File browser module.
//!
//! Provides file system navigation and file picking functionality.

pub mod entry;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use entry::{EntryKind, FileEntry, SortOrder};

/// Maximum entries to load per directory.
const MAX_ENTRIES: usize = 1000;

/// File browser state.
pub struct FileBrowser {
    /// Current directory.
    current_dir: PathBuf,
    /// Entries in current directory.
    entries: Vec<FileEntry>,
    /// Selected entry index.
    selected: usize,
    /// Scroll offset for viewing.
    scroll_offset: usize,
    /// Visible height (entries).
    visible_height: usize,
    /// Sort order.
    sort_order: SortOrder,
    /// Filter pattern.
    filter: String,
    /// Filtered entries indices.
    filtered_indices: Vec<usize>,
    /// Extension counts for autocomplete.
    extension_counts: HashMap<String, usize>,
    /// Most common extension in current dir.
    common_extension: Option<String>,
    /// Is browser visible/active.
    visible: bool,
}

impl FileBrowser {
    /// Creates a new file browser at the given path.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let current_dir = path.as_ref().canonicalize()?;

        let mut browser = Self {
            current_dir,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_height: 20,
            sort_order: SortOrder::default(),
            filter: String::new(),
            filtered_indices: Vec::new(),
            extension_counts: HashMap::new(),
            common_extension: None,
            visible: false,
        };

        browser.refresh()?;
        Ok(browser)
    }

    /// Creates a file browser in the current working directory.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    pub fn current_dir() -> io::Result<Self> {
        Self::new(std::env::current_dir()?)
    }

    /// Returns the current directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.current_dir
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Returns filtered entries based on current filter.
    #[must_use]
    pub fn filtered_entries(&self) -> Vec<&FileEntry> {
        if self.filter.is_empty() {
            self.entries.iter().collect()
        } else {
            self.filtered_indices
                .iter()
                .filter_map(|&i| self.entries.get(i))
                .collect()
        }
    }

    /// Returns the selected index.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the selected entry.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        let entries = self.filtered_entries();
        entries.get(self.selected).copied()
    }

    /// Returns the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns true if the browser is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shows the file browser.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the file browser.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggles visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Sets the visible height.
    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height.max(1);
        self.ensure_visible();
    }

    /// Returns the most common file extension in current directory.
    #[must_use]
    pub fn common_extension(&self) -> Option<&str> {
        self.common_extension.as_deref()
    }

    /// Refreshes the directory listing.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    pub fn refresh(&mut self) -> io::Result<()> {
        self.entries.clear();
        self.extension_counts.clear();

        // Add parent directory if not at root
        if let Some(parent) = self.current_dir.parent() {
            self.entries
                .push(FileEntry::parent_dir(parent.to_path_buf()));
        }

        // Read directory entries
        let read_dir = fs::read_dir(&self.current_dir)?;
        let mut count = 0;

        for entry_result in read_dir {
            if count >= MAX_ENTRIES {
                break;
            }

            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let kind = if metadata.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            };

            let mut file_entry = FileEntry::new(path, kind);

            if kind == EntryKind::File {
                file_entry = file_entry.with_size(metadata.len());

                // Count extensions for autocomplete
                if let Some(ext) = file_entry.extension() {
                    *self.extension_counts.entry(ext.to_string()).or_insert(0) += 1;
                }
            }

            self.entries.push(file_entry);
            count += 1;
        }

        // Sort entries
        self.sort_order.sort(&mut self.entries);

        // Find most common extension
        self.common_extension = self
            .extension_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(ext, _)| ext.clone());

        // Reset selection
        self.selected = 0;
        self.scroll_offset = 0;
        self.update_filter();

        Ok(())
    }

    /// Changes to a new directory.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    pub fn change_dir(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let new_path = path.as_ref().canonicalize()?;

        if !new_path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Path is not a directory",
            ));
        }

        self.current_dir = new_path;
        self.filter.clear();
        self.refresh()
    }

    /// Goes up one directory level.
    ///
    /// # Errors
    /// Returns error if parent directory cannot be accessed.
    pub fn go_up(&mut self) -> io::Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            self.change_dir(parent.to_path_buf())
        } else {
            Ok(())
        }
    }

    /// Enters the selected directory or opens the selected file.
    ///
    /// Returns `Some(path)` if a file was selected, `None` if a directory was entered.
    ///
    /// # Errors
    /// Returns error if the entry cannot be accessed.
    pub fn enter_selected(&mut self) -> io::Result<Option<PathBuf>> {
        let entry = match self.selected_entry() {
            Some(e) => e.clone(),
            None => return Ok(None),
        };

        match entry.kind() {
            EntryKind::File => Ok(Some(entry.path().clone())),
            EntryKind::Directory | EntryKind::ParentDir => {
                self.change_dir(entry.path())?;
                Ok(None)
            }
        }
    }

    /// Moves selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    /// Moves selection down.
    pub fn move_down(&mut self) {
        let max = self.filtered_entries().len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    /// Moves selection up by a page.
    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.visible_height);
        self.ensure_visible();
    }

    /// Moves selection down by a page.
    pub fn page_down(&mut self) {
        let max = self.filtered_entries().len().saturating_sub(1);
        self.selected = (self.selected + self.visible_height).min(max);
        self.ensure_visible();
    }

    /// Moves to the first entry.
    pub fn move_to_start(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Moves to the last entry.
    pub fn move_to_end(&mut self) {
        let len = self.filtered_entries().len();
        self.selected = len.saturating_sub(1);
        self.ensure_visible();
    }

    /// Sets the filter pattern.
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.update_filter();
    }

    /// Clears the filter.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filtered_indices.clear();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Returns the current filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Updates filtered indices based on current filter.
    fn update_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices.clear();
            return;
        }

        let filter_lower = self.filter.to_lowercase();
        self.filtered_indices = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name().to_lowercase().contains(&filter_lower))
            .map(|(i, _)| i)
            .collect();

        // Reset selection if out of bounds
        let max = self.filtered_entries().len().saturating_sub(1);
        self.selected = self.selected.min(max);
        self.ensure_visible();
    }

    /// Ensures the selected entry is visible.
    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.visible_height {
            self.scroll_offset = self.selected - self.visible_height + 1;
        }
    }

    /// Searches for files matching a pattern in the current directory.
    #[must_use]
    pub fn search_files(&self, pattern: &str) -> Vec<&FileEntry> {
        let pattern_lower = pattern.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.is_file() && e.name().to_lowercase().contains(&pattern_lower))
            .collect()
    }

    /// Searches for directories matching a pattern.
    #[must_use]
    pub fn search_directories(&self, pattern: &str) -> Vec<&FileEntry> {
        let pattern_lower = pattern.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.is_directory() && e.name().to_lowercase().contains(&pattern_lower))
            .collect()
    }
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::current_dir().unwrap_or_else(|_| Self {
            current_dir: PathBuf::from("."),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_height: 20,
            sort_order: SortOrder::default(),
            filter: String::new(),
            filtered_indices: Vec::new(),
            extension_counts: HashMap::new(),
            common_extension: None,
            visible: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_browser_creation() {
        let browser = FileBrowser::current_dir();
        assert!(browser.is_ok());
    }

    #[test]
    fn test_file_entry_creation() {
        let entry = FileEntry::new(PathBuf::from("test.rs"), EntryKind::File);
        assert_eq!(entry.name(), "test.rs");
        assert_eq!(entry.extension(), Some("rs"));
        assert!(entry.is_file());
    }

    #[test]
    fn test_directory_entry() {
        let entry = FileEntry::new(PathBuf::from("src"), EntryKind::Directory);
        assert!(entry.is_directory());
        assert!(!entry.is_file());
    }
}
