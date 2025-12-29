//! Remote file browser for SFTP-based directory navigation.
//!
//! Provides a file browser interface for remote systems over SSH/SFTP.

use super::{RemoteError, RemoteFileManager};
use crate::terminal::SSHContext;

/// Maximum entries to display.
const MAX_ENTRIES: usize = 500;

/// Remote file browser entry.
#[derive(Debug, Clone)]
pub struct RemoteFileEntry {
    /// Entry name.
    pub name: String,
    /// Full path on remote system.
    pub path: String,
    /// True if this is a directory.
    pub is_directory: bool,
    /// File size in bytes.
    pub size: u64,
    /// True if this is the parent directory entry.
    pub is_parent: bool,
}

impl RemoteFileEntry {
    /// Creates a new file entry.
    #[must_use]
    pub fn new(name: String, path: String, is_directory: bool, size: u64) -> Self {
        Self {
            name,
            path,
            is_directory,
            size,
            is_parent: false,
        }
    }

    /// Creates a parent directory entry.
    #[must_use]
    pub fn parent(parent_path: String) -> Self {
        Self {
            name: "..".to_string(),
            path: parent_path,
            is_directory: true,
            size: 0,
            is_parent: true,
        }
    }
}

/// Remote file browser state.
pub struct RemoteFileBrowser {
    /// SSH context for the connection.
    ssh_context: SSHContext,
    /// Current remote directory.
    current_dir: String,
    /// Entries in the current directory.
    entries: Vec<RemoteFileEntry>,
    /// Selected entry index.
    selected: usize,
    /// Scroll offset for viewing.
    scroll_offset: usize,
    /// Visible height (entries).
    visible_height: usize,
    /// Filter pattern.
    filter: String,
    /// Filtered entry indices.
    filtered_indices: Vec<usize>,
    /// Is browser visible/active.
    visible: bool,
    /// Last error message.
    last_error: Option<String>,
}

impl RemoteFileBrowser {
    /// Creates a new remote file browser.
    ///
    /// # Arguments
    /// * `ssh_context` - The SSH connection context
    /// * `initial_dir` - The initial directory to browse
    #[must_use]
    pub fn new(ssh_context: SSHContext, initial_dir: String) -> Self {
        assert!(!initial_dir.is_empty(), "initial_dir must not be empty");

        Self {
            ssh_context,
            current_dir: initial_dir,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_height: 20,
            filter: String::new(),
            filtered_indices: Vec::new(),
            visible: true,
            last_error: None,
        }
    }

    /// Returns the SSH context.
    #[must_use]
    pub fn ssh_context(&self) -> &SSHContext {
        &self.ssh_context
    }

    /// Returns the current remote directory.
    #[must_use]
    pub fn current_dir(&self) -> &str {
        &self.current_dir
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[RemoteFileEntry] {
        &self.entries
    }

    /// Returns filtered entries based on current filter.
    #[must_use]
    pub fn filtered_entries(&self) -> Vec<&RemoteFileEntry> {
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
    pub fn selected_entry(&self) -> Option<&RemoteFileEntry> {
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

    /// Sets the visible height.
    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height.max(1);
        self.ensure_visible();
    }

    /// Returns the last error message, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Clears the last error.
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    /// Refreshes the directory listing using the remote manager.
    ///
    /// # Errors
    /// Returns error if directory cannot be read.
    pub fn refresh(&mut self, manager: &mut RemoteFileManager) -> Result<(), RemoteError> {
        self.entries.clear();
        self.last_error = None;

        // Add parent directory entry if not at root
        if self.current_dir != "/" {
            let parent = self.get_parent_path();
            self.entries.push(RemoteFileEntry::parent(parent));
        }

        // List remote directory
        match manager.list_dir(&self.ssh_context, &self.current_dir) {
            Ok(dir_entries) => {
                let mut count = 0;
                for entry in dir_entries {
                    if count >= MAX_ENTRIES {
                        break;
                    }

                    // Skip . and ..
                    if entry.name == "." || entry.name == ".." {
                        continue;
                    }

                    let path = if self.current_dir == "/" {
                        format!("/{}", entry.name)
                    } else {
                        format!("{}/{}", self.current_dir, entry.name)
                    };

                    self.entries.push(RemoteFileEntry::new(
                        entry.name,
                        path,
                        entry.is_directory,
                        entry.size,
                    ));
                    count += 1;
                }
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                return Err(e);
            }
        }

        // Sort: parent first, then directories, then files alphabetically
        self.entries.sort_by(|a, b| {
            if a.is_parent {
                return std::cmp::Ordering::Less;
            }
            if b.is_parent {
                return std::cmp::Ordering::Greater;
            }
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        // Reset selection
        self.selected = 0;
        self.scroll_offset = 0;
        self.update_filter();

        Ok(())
    }

    /// Changes to a new directory.
    ///
    /// # Errors
    /// Returns error if directory cannot be accessed.
    pub fn change_dir(
        &mut self,
        path: &str,
        manager: &mut RemoteFileManager,
    ) -> Result<(), RemoteError> {
        assert!(!path.is_empty(), "path must not be empty");

        let old_dir = self.current_dir.clone();
        self.current_dir = path.to_string();
        self.filter.clear();

        if let Err(e) = self.refresh(manager) {
            // Revert on failure
            self.current_dir = old_dir;
            return Err(e);
        }

        Ok(())
    }

    /// Goes up one directory level.
    ///
    /// # Errors
    /// Returns error if parent directory cannot be accessed.
    pub fn go_up(&mut self, manager: &mut RemoteFileManager) -> Result<(), RemoteError> {
        if self.current_dir == "/" {
            return Ok(());
        }

        let parent = self.get_parent_path();
        self.change_dir(&parent, manager)
    }

    /// Gets the parent directory path.
    fn get_parent_path(&self) -> String {
        if self.current_dir == "/" {
            return "/".to_string();
        }

        let trimmed = self.current_dir.trim_end_matches('/');
        match trimmed.rfind('/') {
            Some(0) => "/".to_string(),
            Some(pos) => trimmed[..pos].to_string(),
            None => "/".to_string(),
        }
    }

    /// Enters the selected directory or returns the selected file path.
    ///
    /// Returns `Some(path)` if a file was selected, `None` if a directory was entered.
    ///
    /// # Errors
    /// Returns error if the entry cannot be accessed.
    pub fn enter_selected(
        &mut self,
        manager: &mut RemoteFileManager,
    ) -> Result<Option<String>, RemoteError> {
        let entry = match self.selected_entry() {
            Some(e) => e.clone(),
            None => return Ok(None),
        };

        if entry.is_directory {
            self.change_dir(&entry.path, manager)?;
            Ok(None)
        } else {
            Ok(Some(entry.path))
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
            .filter(|(_, e)| e.name.to_lowercase().contains(&filter_lower))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_file_entry() {
        let entry = RemoteFileEntry::new(
            "test.txt".to_string(),
            "/home/user/test.txt".to_string(),
            false,
            1024,
        );
        assert_eq!(entry.name, "test.txt");
        assert!(!entry.is_directory);
        assert!(!entry.is_parent);
    }

    #[test]
    fn test_parent_entry() {
        let entry = RemoteFileEntry::parent("/home".to_string());
        assert_eq!(entry.name, "..");
        assert!(entry.is_directory);
        assert!(entry.is_parent);
    }

    #[test]
    fn test_get_parent_path() {
        let ctx = SSHContext::new("user".to_string(), "host".to_string(), 22);

        let browser = RemoteFileBrowser::new(ctx.clone(), "/home/user/docs".to_string());
        assert_eq!(browser.get_parent_path(), "/home/user");

        let browser = RemoteFileBrowser::new(ctx.clone(), "/home".to_string());
        assert_eq!(browser.get_parent_path(), "/");

        let browser = RemoteFileBrowser::new(ctx, "/".to_string());
        assert_eq!(browser.get_parent_path(), "/");
    }
}
