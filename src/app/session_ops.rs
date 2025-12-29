//! Session save/restore operations for the App.

use std::io;

use crate::config::KeybindingMode;
use crate::editor::edit::Position;
use crate::session::{PersistedFile, Session};
use crate::ui::layout::FocusedPane;

use super::App;

impl App {
    /// Saves the current session state to disk.
    ///
    /// # Errors
    /// Returns error if save fails.
    pub fn save_session(&self) -> io::Result<()> {
        let mut session = Session::default();

        // Save open files with cursor positions
        for (idx, file) in self.open_files.iter().enumerate() {
            let (cursor_line, cursor_col) = if idx == self.current_file_idx {
                let pos = self.editor.cursor_position();
                (pos.line, pos.col)
            } else {
                (0, 0)
            };

            let scroll_offset = if idx == self.current_file_idx {
                self.editor.view().scroll_top()
            } else {
                0
            };

            session.open_files.push(PersistedFile {
                path: file.path.clone(),
                cursor_line,
                cursor_col,
                modified: idx == self.current_file_idx && self.editor.is_modified(),
                scroll_offset,
            });
        }

        session.active_file_idx = self.current_file_idx;
        session.cwd = self.file_browser.path().to_path_buf();

        session.focused_pane = match self.layout.focused() {
            FocusedPane::Terminal => 0,
            FocusedPane::Editor => 1,
        };

        session.keybinding_mode = match self.config.mode {
            KeybindingMode::Vim => "vim".to_string(),
            KeybindingMode::Emacs => "emacs".to_string(),
            KeybindingMode::VsCode => "vscode".to_string(),
            KeybindingMode::Default => "default".to_string(),
        };

        if let Some(ref terminals) = self.terminals {
            session.terminal_tab_count = terminals.tab_count();
            session.active_terminal_idx = terminals.active_tab_index();
        }

        session.save()
    }

    /// Restores session state from disk if available.
    ///
    /// # Errors
    /// Returns error if restore fails.
    pub fn restore_session(&mut self) -> io::Result<bool> {
        if !Session::exists() {
            return Ok(false);
        }

        let session = Session::load()?;

        // Restore open files
        for persisted_file in &session.open_files {
            if persisted_file.path.exists() {
                let _ = self.open_file(persisted_file.path.clone());
            }
        }

        // Restore active file
        if session.active_file_idx < self.open_files.len() {
            self.current_file_idx = session.active_file_idx;
            if let Some(file) = self.open_files.get(self.current_file_idx) {
                let _ = self.editor.open(&file.path);
            }
        }

        // Restore cursor position for active file if we have one
        if let Some(persisted_file) = session.open_files.get(session.active_file_idx) {
            let pos = Position::new(persisted_file.cursor_line, persisted_file.cursor_col);
            self.editor.set_cursor_position(pos);
            self.editor.goto_line(persisted_file.scroll_offset);
        }

        // Restore working directory
        let _ = self.file_browser.change_dir(&session.cwd);

        // Restore focused pane
        let focused = match session.focused_pane {
            0 => FocusedPane::Terminal,
            _ => FocusedPane::Editor,
        };
        self.layout.set_focused(focused);

        self.set_status("Session restored".to_string());

        Ok(true)
    }
}
