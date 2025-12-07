//! Session persistence module for ratterm.
//!
//! Saves and restores application state for updates and restarts.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Session file name.
const SESSION_FILE: &str = "session.rat";

/// Maximum open files to persist (prevent huge session files).
const MAX_PERSISTED_FILES: usize = 50;

/// Represents a persisted open file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedFile {
    /// File path.
    pub path: PathBuf,
    /// Cursor line (0-indexed).
    pub cursor_line: usize,
    /// Cursor column (0-indexed).
    pub cursor_col: usize,
    /// Whether the file has unsaved changes.
    pub modified: bool,
    /// Scroll offset from top.
    pub scroll_offset: usize,
}

/// Represents the persisted session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Version of the session format.
    pub version: u32,
    /// Open files with their state.
    pub open_files: Vec<PersistedFile>,
    /// Currently active file index.
    pub active_file_idx: usize,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Which pane was focused (terminal=0, editor=1).
    pub focused_pane: u8,
    /// Editor keybinding mode.
    pub keybinding_mode: String,
    /// Number of terminal tabs.
    pub terminal_tab_count: usize,
    /// Active terminal tab index.
    pub active_terminal_idx: usize,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            version: 1,
            open_files: Vec::new(),
            active_file_idx: 0,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            focused_pane: 0,
            keybinding_mode: "vim".to_string(),
            terminal_tab_count: 1,
            active_terminal_idx: 0,
        }
    }
}

impl Session {
    /// Returns the session file path.
    #[must_use]
    pub fn session_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ratterm")
            .join(SESSION_FILE)
    }

    /// Saves the session to disk.
    ///
    /// # Errors
    /// Returns error if save fails.
    pub fn save(&self) -> io::Result<()> {
        let path = Self::session_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Limit number of persisted files
        let mut session = self.clone();
        if session.open_files.len() > MAX_PERSISTED_FILES {
            session.open_files.truncate(MAX_PERSISTED_FILES);
        }

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&session)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        fs::write(&path, json)?;

        tracing::info!("Session saved to {:?}", path);

        Ok(())
    }

    /// Loads the session from disk.
    ///
    /// # Errors
    /// Returns error if load fails.
    pub fn load() -> io::Result<Self> {
        let path = Self::session_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;

        let session: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Delete session file after successful load
        let _ = fs::remove_file(&path);

        tracing::info!("Session restored from {:?}", path);

        Ok(session)
    }

    /// Checks if a session file exists.
    #[must_use]
    pub fn exists() -> bool {
        Self::session_path().exists()
    }

    /// Deletes the session file.
    pub fn delete() {
        let path = Self::session_path();
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_default() {
        let session = Session::default();
        assert_eq!(session.version, 1);
        assert!(session.open_files.is_empty());
    }

    #[test]
    fn test_session_serialization() {
        let session = Session {
            version: 1,
            open_files: vec![PersistedFile {
                path: PathBuf::from("/test/file.rs"),
                cursor_line: 10,
                cursor_col: 5,
                modified: false,
                scroll_offset: 0,
            }],
            active_file_idx: 0,
            cwd: PathBuf::from("/test"),
            focused_pane: 1,
            keybinding_mode: "vim".to_string(),
            terminal_tab_count: 2,
            active_terminal_idx: 1,
        };

        let json = serde_json::to_string(&session).expect("Serialize failed");
        let restored: Session = serde_json::from_str(&json).expect("Deserialize failed");

        assert_eq!(restored.version, 1);
        assert_eq!(restored.open_files.len(), 1);
        assert_eq!(restored.open_files[0].cursor_line, 10);
    }
}
