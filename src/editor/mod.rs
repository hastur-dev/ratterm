//! Code editor module.
//!
//! Provides text editing with syntax highlighting and LSP support.

pub mod buffer;
pub mod cursor;
pub mod edit;
mod editing;
pub mod find;
mod movement;
mod selection;
pub mod view;

use std::path::PathBuf;

use self::buffer::Buffer;
use self::cursor::Cursor;
use self::edit::Position;
use self::view::View;

use crate::remote::RemoteFile;

/// Editor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// Normal mode (navigation).
    #[default]
    Normal,
    /// Insert mode (typing).
    Insert,
    /// Visual mode (selection).
    Visual,
    /// Command mode.
    Command,
}

/// Editor instance.
pub struct Editor {
    /// Text buffer.
    pub(crate) buffer: Buffer,
    /// Cursor.
    pub(crate) cursor: Cursor,
    /// Viewport.
    pub(crate) view: View,
    /// Editor mode.
    mode: EditorMode,
    /// File path.
    pub(crate) path: Option<PathBuf>,
    /// Status message.
    status: String,
    /// Remote file metadata (if editing a file via SSH).
    remote_file: Option<RemoteFile>,
}

impl Editor {
    /// Creates a new empty editor.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        assert!(width > 0, "Width must be positive");
        assert!(height > 0, "Height must be positive");

        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            view: View::new(width as usize, height as usize),
            mode: EditorMode::Normal,
            path: None,
            status: String::new(),
            remote_file: None,
        }
    }

    /// Returns the buffer.
    #[must_use]
    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Returns a mutable buffer reference.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Returns the cursor.
    #[must_use]
    pub const fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Returns a mutable cursor reference.
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Returns the view.
    #[must_use]
    pub const fn view(&self) -> &View {
        &self.view
    }

    /// Returns a mutable view reference.
    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    /// Returns the current mode.
    #[must_use]
    pub const fn mode(&self) -> EditorMode {
        self.mode
    }

    /// Sets the editor mode.
    pub fn set_mode(&mut self, mode: EditorMode) {
        self.mode = mode;
    }

    /// Returns the file path.
    #[must_use]
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// Returns the status message.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Sets the status message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    /// Opens a file.
    ///
    /// # Errors
    /// Returns error if file cannot be read.
    pub fn open(&mut self, path: impl Into<PathBuf>) -> std::io::Result<()> {
        let path = path.into();
        let content = std::fs::read_to_string(&path)?;

        self.buffer = Buffer::from_str(&content);
        self.cursor = Cursor::new();
        self.path = Some(path);
        self.remote_file = None;
        self.view.reset_scroll();
        self.view.update_gutter_width(self.buffer.len_lines());

        Ok(())
    }

    /// Saves the file.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            std::fs::write(path, self.buffer.text())?;
            self.buffer.mark_saved();
            self.set_status(format!("Saved {}", path.display()));
        } else {
            self.set_status("No file path set");
        }
        Ok(())
    }

    /// Saves the file with a new path.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> std::io::Result<()> {
        let path = path.into();
        std::fs::write(&path, self.buffer.text())?;
        self.buffer.mark_saved();
        self.path = Some(path.clone());
        self.remote_file = None;
        self.set_status(format!("Saved {}", path.display()));
        Ok(())
    }

    /// Opens a remote file with the given content.
    pub fn open_remote(&mut self, content: &str, remote_file: RemoteFile) {
        self.buffer = Buffer::from_str(content);
        self.cursor = Cursor::new();
        self.path = Some(remote_file.local_cache_path.clone());
        self.remote_file = Some(remote_file);
        self.view.reset_scroll();
        self.view.update_gutter_width(self.buffer.len_lines());
        self.mode = EditorMode::Normal;
    }

    /// Returns true if the editor is editing a remote file.
    #[must_use]
    pub fn is_remote(&self) -> bool {
        self.remote_file.is_some()
    }

    /// Returns the remote file metadata if editing a remote file.
    #[must_use]
    pub fn remote_file(&self) -> Option<&RemoteFile> {
        self.remote_file.as_ref()
    }

    /// Clears the remote file metadata (converts to local file).
    pub fn clear_remote(&mut self) {
        self.remote_file = None;
    }

    /// Creates a new empty buffer, clearing any existing content.
    pub fn new_buffer(&mut self) {
        self.buffer = Buffer::new();
        self.cursor = Cursor::new();
        self.path = None;
        self.remote_file = None;
        self.view.reset_scroll();
        self.view.update_gutter_width(self.buffer.len_lines());
        self.mode = EditorMode::Normal;
    }

    /// Resizes the editor viewport.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.view.resize(width as usize, height as usize);
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Ensures the cursor is visible in the viewport.
    pub fn ensure_cursor_visible(&mut self) {
        self.view.ensure_cursor_visible(self.cursor.position());
    }

    /// Inserts a character at the cursor.
    pub fn insert_char(&mut self, c: char) {
        let pos = self.cursor.position();
        self.buffer.insert_char(pos, c);

        if c == '\n' {
            self.cursor.set_position(Position::new(pos.line + 1, 0));
        } else {
            self.cursor
                .set_position(Position::new(pos.line, pos.col + 1));
        }

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Inserts a string at the cursor.
    pub fn insert_str(&mut self, s: &str) {
        let pos = self.cursor.position();
        self.buffer.insert_str(pos, s);

        let new_pos = self
            .buffer
            .index_to_position(self.buffer.position_to_index(pos) + s.chars().count());
        self.cursor.set_position(new_pos);

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Deletes the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        let pos = self.cursor.position();

        if pos.col > 0 {
            let new_pos = Position::new(pos.line, pos.col - 1);
            self.buffer.delete_char_backward(pos);
            self.cursor.set_position(new_pos);
        } else if pos.line > 0 {
            let prev_line_len = self.buffer.line_len_chars(pos.line - 1);
            self.buffer.delete_char_backward(pos);
            self.cursor
                .set_position(Position::new(pos.line - 1, prev_line_len));
        }

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Deletes the character at the cursor (delete).
    pub fn delete(&mut self) {
        let pos = self.cursor.position();
        self.buffer.delete_char(pos);
        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Deletes the selected text.
    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.cursor.selection_range() {
            self.buffer.delete_range(start, end);
            self.cursor.move_to(start);
            self.view.update_gutter_width(self.buffer.len_lines());
            self.ensure_cursor_visible();
        }
    }

    /// Deletes from the cursor to the end of the line (Emacs Ctrl+K).
    pub fn delete_to_line_end(&mut self) {
        let pos = self.cursor.position();
        let line_len = self.buffer.line_len_chars(pos.line);

        if pos.col < line_len {
            let end = Position::new(pos.line, line_len);
            self.buffer.delete_range(pos, end);
        } else if pos.line < self.buffer.len_lines().saturating_sub(1) {
            let next_line_start = Position::new(pos.line + 1, 0);
            self.buffer.delete_range(pos, next_line_start);
        }

        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Undoes the last edit.
    pub fn undo(&mut self) {
        self.buffer.undo();
        self.cursor.clamp(&self.buffer);
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Redoes the last undone edit.
    pub fn redo(&mut self) {
        self.buffer.redo();
        self.cursor.clamp(&self.buffer);
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Returns true if the buffer is modified.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = Editor::new(80, 24);
        assert!(editor.buffer().is_empty());
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn test_editor_insert() {
        let mut editor = Editor::new(80, 24);
        editor.insert_char('H');
        editor.insert_char('i');
        assert_eq!(editor.buffer().text(), "Hi");
    }

    #[test]
    fn test_editor_backspace() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("Hello");
        editor.backspace();
        assert_eq!(editor.buffer().text(), "Hell");
    }

    #[test]
    fn test_editor_undo() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("Hello");
        editor.undo();
        assert_eq!(editor.buffer().text(), "");
    }
}
