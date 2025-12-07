//! Code editor module.
//!
//! Provides text editing with syntax highlighting and LSP support.

pub mod buffer;
pub mod cursor;
pub mod edit;
pub mod find;
pub mod view;

use std::path::PathBuf;

use self::buffer::Buffer;
use self::edit::Position;
use self::cursor::Cursor;
use self::view::View;

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
    buffer: Buffer,
    /// Cursor.
    cursor: Cursor,
    /// Viewport.
    view: View,
    /// Editor mode.
    mode: EditorMode,
    /// File path.
    path: Option<PathBuf>,
    /// Status message.
    status: String,
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
        self.set_status(format!("Saved {}", path.display()));
        Ok(())
    }

    /// Creates a new empty buffer, clearing any existing content.
    pub fn new_buffer(&mut self) {
        self.buffer = Buffer::new();
        self.cursor = Cursor::new();
        self.path = None;
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

        // Move cursor after the inserted character
        if c == '\n' {
            self.cursor.set_position(Position::new(pos.line + 1, 0));
        } else {
            self.cursor.set_position(Position::new(pos.line, pos.col + 1));
        }

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Inserts a string at the cursor.
    pub fn insert_str(&mut self, s: &str) {
        let pos = self.cursor.position();
        self.buffer.insert_str(pos, s);

        // Calculate new cursor position
        let new_pos = self.buffer.index_to_position(
            self.buffer.position_to_index(pos) + s.chars().count(),
        );
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
            // Join with previous line
            let prev_line_len = self.buffer.line_len_chars(pos.line - 1);
            self.buffer.delete_char_backward(pos);
            self.cursor.set_position(Position::new(pos.line - 1, prev_line_len));
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
            // Delete from cursor to end of line (but not newline)
            let end = Position::new(pos.line, line_len);
            self.buffer.delete_range(pos, end);
        } else if pos.line < self.buffer.len_lines().saturating_sub(1) {
            // At end of line - delete the newline (join with next line)
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

    /// Moves cursor left.
    pub fn move_left(&mut self) {
        self.cursor.move_left(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor right.
    pub fn move_right(&mut self) {
        self.cursor.move_right(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor up.
    pub fn move_up(&mut self) {
        self.cursor.move_up(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor down.
    pub fn move_down(&mut self) {
        self.cursor.move_down(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to line start.
    pub fn move_to_line_start(&mut self) {
        self.cursor.move_to_line_start();
        self.ensure_cursor_visible();
    }

    /// Moves cursor to line end.
    pub fn move_to_line_end(&mut self) {
        self.cursor.move_to_line_end(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to buffer start.
    pub fn move_to_buffer_start(&mut self) {
        self.cursor.move_to_buffer_start();
        self.ensure_cursor_visible();
    }

    /// Moves cursor to buffer end.
    pub fn move_to_buffer_end(&mut self) {
        self.cursor.move_to_buffer_end(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor up by a page.
    pub fn page_up(&mut self) {
        let page_size = self.view.height().saturating_sub(2);
        self.cursor.move_page_up(&self.buffer, page_size);
        self.ensure_cursor_visible();
    }

    /// Moves cursor down by a page.
    pub fn page_down(&mut self) {
        let page_size = self.view.height().saturating_sub(2);
        self.cursor.move_page_down(&self.buffer, page_size);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to previous word.
    pub fn move_word_left(&mut self) {
        self.cursor.move_word_left(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Moves cursor to next word.
    pub fn move_word_right(&mut self) {
        self.cursor.move_word_right(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Gets the word at the cursor position.
    #[must_use]
    pub fn word_at_cursor(&self) -> Option<String> {
        let start = self.buffer.word_start(self.cursor.position());
        let end = self.buffer.word_end(self.cursor.position());
        self.buffer.get_range(start, end)
    }

    /// Returns true if the buffer is modified.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Returns the cursor position.
    #[must_use]
    pub fn cursor_position(&self) -> Position {
        self.cursor.position()
    }

    /// Sets the cursor position.
    pub fn set_cursor_position(&mut self, pos: Position) {
        self.cursor.set_position(self.buffer.clamp_position(pos));
        self.ensure_cursor_visible();
    }

    /// Goes to a specific line.
    pub fn goto_line(&mut self, line: usize) {
        let line = line.min(self.buffer.len_lines().saturating_sub(1));
        self.cursor.set_position(Position::new(line, 0));
        self.view.center_on_line(line, self.buffer.len_lines());
    }

    /// Returns the currently selected text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        self.cursor
            .selection_range()
            .and_then(|(start, end)| self.buffer.get_range(start, end))
    }

    /// Returns the current line text.
    #[must_use]
    pub fn current_line(&self) -> String {
        let line = self.cursor.position().line;
        self.buffer.line(line).unwrap_or_default()
    }

    // -------------------------------------------------------------------------
    // VSCode-style selection methods
    // -------------------------------------------------------------------------

    /// Extends selection left by one character.
    pub fn select_left(&mut self) {
        let buffer = &self.buffer;
        let old_pos = self.cursor.position();

        // Calculate new position (move left logic)
        let new_pos = if old_pos.col > 0 {
            Position::new(old_pos.line, old_pos.col - 1)
        } else if old_pos.line > 0 {
            let prev_line_len = buffer.line_len_chars(old_pos.line - 1);
            Position::new(old_pos.line - 1, prev_line_len)
        } else {
            old_pos
        };

        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection right by one character.
    pub fn select_right(&mut self) {
        let buffer = &self.buffer;
        let old_pos = self.cursor.position();
        let line_len = buffer.line_len_chars(old_pos.line);

        let new_pos = if old_pos.col < line_len {
            Position::new(old_pos.line, old_pos.col + 1)
        } else if old_pos.line < buffer.len_lines().saturating_sub(1) {
            Position::new(old_pos.line + 1, 0)
        } else {
            old_pos
        };

        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection up by one line.
    pub fn select_up(&mut self) {
        let old_pos = self.cursor.position();
        if old_pos.line == 0 {
            return;
        }

        let new_line = old_pos.line - 1;
        let line_len = self.buffer.line_len_chars(new_line);
        let new_col = old_pos.col.min(line_len);

        self.cursor.extend_to(Position::new(new_line, new_col));
        self.ensure_cursor_visible();
    }

    /// Extends selection down by one line.
    pub fn select_down(&mut self) {
        let old_pos = self.cursor.position();
        let last_line = self.buffer.len_lines().saturating_sub(1);

        if old_pos.line >= last_line {
            return;
        }

        let new_line = old_pos.line + 1;
        let line_len = self.buffer.line_len_chars(new_line);
        let new_col = old_pos.col.min(line_len);

        self.cursor.extend_to(Position::new(new_line, new_col));
        self.ensure_cursor_visible();
    }

    /// Extends selection left by one word.
    pub fn select_word_left(&mut self) {
        let mut idx = self.buffer.position_to_index(self.cursor.position());

        if idx == 0 {
            return;
        }

        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip whitespace
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        // Skip word characters
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        let new_pos = self.buffer.index_to_position(idx);
        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection right by one word.
    pub fn select_word_right(&mut self) {
        let mut idx = self.buffer.position_to_index(self.cursor.position());
        let len = self.buffer.len_chars();

        if idx >= len {
            return;
        }

        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip word characters
        while idx < len && !chars[idx].is_whitespace() {
            idx += 1;
        }

        // Skip whitespace
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }

        let new_pos = self.buffer.index_to_position(idx);
        self.cursor.extend_to(new_pos);
        self.ensure_cursor_visible();
    }

    /// Extends selection to line start.
    pub fn select_to_line_start(&mut self) {
        let pos = self.cursor.position();
        self.cursor.extend_to(Position::new(pos.line, 0));
        self.ensure_cursor_visible();
    }

    /// Extends selection to line end.
    pub fn select_to_line_end(&mut self) {
        let pos = self.cursor.position();
        let line_len = self.buffer.line_len_chars(pos.line);
        self.cursor.extend_to(Position::new(pos.line, line_len));
        self.ensure_cursor_visible();
    }

    /// Selects all text in the buffer.
    pub fn select_all(&mut self) {
        self.cursor.select_all(&self.buffer);
        self.ensure_cursor_visible();
    }

    /// Selects the current line.
    pub fn select_line(&mut self) {
        self.cursor.select_line(&self.buffer);
        self.ensure_cursor_visible();
    }

    // -------------------------------------------------------------------------
    // VSCode-style editing methods
    // -------------------------------------------------------------------------

    /// Duplicates the current line.
    pub fn duplicate_line(&mut self) {
        let line_idx = self.cursor.position().line;
        let line_content = self.buffer.line(line_idx).unwrap_or_default();

        // Insert newline at end of current line, then the line content
        let line_len = self.buffer.line_len_chars(line_idx);
        let end_pos = Position::new(line_idx, line_len);

        // Insert newline + duplicate content
        let insert_text = format!("\n{}", line_content.trim_end_matches('\n'));
        self.buffer.insert_str(end_pos, &insert_text);

        // Move cursor to duplicated line
        self.cursor.set_position(Position::new(line_idx + 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Deletes the current line.
    pub fn delete_line(&mut self) {
        let line_idx = self.cursor.position().line;
        let total_lines = self.buffer.len_lines();

        if total_lines == 0 {
            return;
        }

        // Calculate range to delete (entire line including newline)
        let start = Position::new(line_idx, 0);
        let end = if line_idx < total_lines - 1 {
            // Not the last line - delete up to start of next line
            Position::new(line_idx + 1, 0)
        } else if line_idx > 0 {
            // Last line - delete from end of previous line
            let prev_line_len = self.buffer.line_len_chars(line_idx - 1);
            let new_start = Position::new(line_idx - 1, prev_line_len);
            self.buffer.delete_range(new_start, Position::new(line_idx, self.buffer.line_len_chars(line_idx)));
            self.cursor.set_position(Position::new(line_idx - 1, prev_line_len.min(self.cursor.position().col)));
            self.view.update_gutter_width(self.buffer.len_lines());
            self.ensure_cursor_visible();
            return;
        } else {
            // Only line - just clear it
            Position::new(0, self.buffer.line_len_chars(0))
        };

        self.buffer.delete_range(start, end);

        // Adjust cursor position
        let new_line = line_idx.min(self.buffer.len_lines().saturating_sub(1));
        let new_col = self.cursor.position().col.min(self.buffer.line_len_chars(new_line));
        self.cursor.set_position(Position::new(new_line, new_col));

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Moves the current line up.
    pub fn move_line_up(&mut self) {
        let line_idx = self.cursor.position().line;
        if line_idx == 0 {
            return;
        }

        // Get both lines
        let current_line = self.buffer.line(line_idx).unwrap_or_default();
        let prev_line = self.buffer.line(line_idx - 1).unwrap_or_default();

        // Delete both lines and reinsert in swapped order
        let start = Position::new(line_idx - 1, 0);
        let end = if line_idx < self.buffer.len_lines() - 1 {
            Position::new(line_idx + 1, 0)
        } else {
            Position::new(line_idx, self.buffer.line_len_chars(line_idx))
        };

        self.buffer.delete_range(start, end);

        // Insert swapped content
        let new_content = if line_idx < self.buffer.len_lines() {
            format!("{}{}", current_line.trim_end_matches('\n'), prev_line)
        } else {
            format!("{}\n{}", current_line.trim_end_matches('\n'), prev_line.trim_end_matches('\n'))
        };
        self.buffer.insert_str(start, &new_content);

        // Move cursor up
        self.cursor.set_position(Position::new(line_idx - 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Moves the current line down.
    pub fn move_line_down(&mut self) {
        let line_idx = self.cursor.position().line;
        let last_line = self.buffer.len_lines().saturating_sub(1);

        if line_idx >= last_line {
            return;
        }

        // Get both lines
        let current_line = self.buffer.line(line_idx).unwrap_or_default();
        let next_line = self.buffer.line(line_idx + 1).unwrap_or_default();

        // Delete both lines and reinsert in swapped order
        let start = Position::new(line_idx, 0);
        let end = if line_idx + 1 < last_line {
            Position::new(line_idx + 2, 0)
        } else {
            Position::new(line_idx + 1, self.buffer.line_len_chars(line_idx + 1))
        };

        self.buffer.delete_range(start, end);

        // Insert swapped content
        let new_content = if line_idx + 1 < last_line {
            format!("{}{}", next_line.trim_end_matches('\n'), current_line)
        } else {
            format!("{}\n{}", next_line.trim_end_matches('\n'), current_line.trim_end_matches('\n'))
        };
        self.buffer.insert_str(start, &new_content);

        // Move cursor down
        self.cursor.set_position(Position::new(line_idx + 1, self.cursor.position().col));
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Toggles line comment on current line or selection.
    pub fn toggle_comment(&mut self) {
        let line_idx = self.cursor.position().line;
        let line = self.buffer.line(line_idx).unwrap_or_default();
        let trimmed = line.trim_start();

        // Detect comment prefix based on file extension or use //
        let comment_prefix = self.detect_comment_prefix();

        if trimmed.starts_with(&comment_prefix) {
            // Uncomment: remove the comment prefix
            if let Some(idx) = line.find(&comment_prefix) {
                let remove_len = if line.len() > idx + comment_prefix.len()
                    && line.chars().nth(idx + comment_prefix.len()) == Some(' ')
                {
                    comment_prefix.len() + 1
                } else {
                    comment_prefix.len()
                };

                let start = Position::new(line_idx, idx);
                let end = Position::new(line_idx, idx + remove_len);
                self.buffer.delete_range(start, end);

                // Adjust cursor if needed
                let cursor_col = self.cursor.position().col;
                if cursor_col > idx {
                    let new_col = cursor_col.saturating_sub(remove_len);
                    self.cursor.set_position(Position::new(line_idx, new_col));
                }
            }
        } else {
            // Comment: add comment prefix at first non-whitespace
            let indent = line.len() - trimmed.len();
            let insert_pos = Position::new(line_idx, indent);
            let comment_text = format!("{} ", comment_prefix);
            self.buffer.insert_str(insert_pos, &comment_text);

            // Adjust cursor
            let cursor_col = self.cursor.position().col;
            if cursor_col >= indent {
                self.cursor.set_position(Position::new(line_idx, cursor_col + comment_text.len()));
            }
        }

        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Detects the appropriate comment prefix based on file extension.
    fn detect_comment_prefix(&self) -> String {
        let ext = self
            .path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "py" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" | "rb" | "pl" => "#".to_string(),
            "html" | "xml" => "<!--".to_string(),
            "css" | "scss" | "less" => "/*".to_string(),
            "sql" => "--".to_string(),
            "lua" => "--".to_string(),
            "vim" => "\"".to_string(),
            _ => "//".to_string(), // Default for C-style languages
        }
    }

    /// Indents the current line or selection.
    pub fn indent(&mut self) {
        let line_idx = self.cursor.position().line;
        let indent_str = "    "; // 4 spaces

        let insert_pos = Position::new(line_idx, 0);
        self.buffer.insert_str(insert_pos, indent_str);

        // Move cursor by indent amount
        let new_col = self.cursor.position().col + indent_str.len();
        self.cursor.set_position(Position::new(line_idx, new_col));

        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Removes indentation from the current line.
    pub fn outdent(&mut self) {
        let line_idx = self.cursor.position().line;
        let line = self.buffer.line(line_idx).unwrap_or_default();

        // Count leading spaces/tabs to remove (up to 4 spaces or 1 tab)
        let mut remove_count = 0;
        for (i, c) in line.chars().enumerate() {
            if c == ' ' && remove_count < 4 {
                remove_count += 1;
            } else if c == '\t' && remove_count == 0 {
                remove_count = 1;
                break;
            } else {
                break;
            }
            if i >= 3 {
                break;
            }
        }

        if remove_count > 0 {
            let start = Position::new(line_idx, 0);
            let end = Position::new(line_idx, remove_count);
            self.buffer.delete_range(start, end);

            // Adjust cursor position
            let cursor_col = self.cursor.position().col;
            let new_col = cursor_col.saturating_sub(remove_count);
            self.cursor.set_position(Position::new(line_idx, new_col));

            self.view.update_gutter_width(self.buffer.len_lines());
        }
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
