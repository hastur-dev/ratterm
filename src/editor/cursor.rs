//! Editor cursor management.
//!
//! Handles cursor position, movement, and selection.

use super::buffer::{Buffer, Position};

/// Selection anchor direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionDirection {
    /// No selection (cursor only).
    #[default]
    None,
    /// Selection extends forward from anchor.
    Forward,
    /// Selection extends backward from anchor.
    Backward,
}

/// Editor cursor with optional selection.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// Current cursor position.
    position: Position,
    /// Selection anchor (if any).
    anchor: Option<Position>,
    /// Preferred column for vertical movement.
    preferred_col: Option<usize>,
}

impl Cursor {
    /// Creates a new cursor at the origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            position: Position::new(0, 0),
            anchor: None,
            preferred_col: None,
        }
    }

    /// Creates a cursor at the given position.
    #[must_use]
    pub fn at(pos: Position) -> Self {
        Self {
            position: pos,
            anchor: None,
            preferred_col: None,
        }
    }

    /// Returns the current position.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    /// Returns the selection anchor if any.
    #[must_use]
    pub const fn anchor(&self) -> Option<Position> {
        self.anchor
    }

    /// Returns true if there is a selection.
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        self.anchor.is_some()
    }

    /// Returns the selection range (start, end) if any.
    #[must_use]
    pub fn selection_range(&self) -> Option<(Position, Position)> {
        self.anchor.map(|anchor| {
            if self.compare_positions(anchor, self.position) <= 0 {
                (anchor, self.position)
            } else {
                (self.position, anchor)
            }
        })
    }

    /// Compares two positions (-1 if a < b, 0 if equal, 1 if a > b).
    fn compare_positions(&self, a: Position, b: Position) -> i32 {
        if a.line < b.line {
            -1
        } else if a.line > b.line {
            1
        } else if a.col < b.col {
            -1
        } else if a.col > b.col {
            1
        } else {
            0
        }
    }

    /// Sets the cursor position.
    pub fn set_position(&mut self, pos: Position) {
        self.position = pos;
        self.preferred_col = None;
    }

    /// Moves to a position and clears selection.
    pub fn move_to(&mut self, pos: Position) {
        self.position = pos;
        self.anchor = None;
        self.preferred_col = None;
    }

    /// Extends selection to a position.
    pub fn extend_to(&mut self, pos: Position) {
        if self.anchor.is_none() {
            self.anchor = Some(self.position);
        }
        self.position = pos;
    }

    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Starts a new selection at the current position.
    pub fn start_selection(&mut self) {
        self.anchor = Some(self.position);
    }

    /// Moves cursor left by one character.
    pub fn move_left(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        if self.position.col > 0 {
            self.position.col -= 1;
        } else if self.position.line > 0 {
            self.position.line -= 1;
            self.position.col = buffer.line_len_chars(self.position.line);
        }
    }

    /// Moves cursor right by one character.
    pub fn move_right(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        let line_len = buffer.line_len_chars(self.position.line);
        if self.position.col < line_len {
            self.position.col += 1;
        } else if self.position.line < buffer.len_lines().saturating_sub(1) {
            self.position.line += 1;
            self.position.col = 0;
        }
    }

    /// Moves cursor up by one line.
    pub fn move_up(&mut self, buffer: &Buffer) {
        self.anchor = None;

        if self.position.line == 0 {
            return;
        }

        // Remember preferred column
        if self.preferred_col.is_none() {
            self.preferred_col = Some(self.position.col);
        }

        self.position.line -= 1;
        let line_len = buffer.line_len_chars(self.position.line);
        self.position.col = self.preferred_col.unwrap_or(0).min(line_len);
    }

    /// Moves cursor down by one line.
    pub fn move_down(&mut self, buffer: &Buffer) {
        self.anchor = None;

        if self.position.line >= buffer.len_lines().saturating_sub(1) {
            return;
        }

        // Remember preferred column
        if self.preferred_col.is_none() {
            self.preferred_col = Some(self.position.col);
        }

        self.position.line += 1;
        let line_len = buffer.line_len_chars(self.position.line);
        self.position.col = self.preferred_col.unwrap_or(0).min(line_len);
    }

    /// Moves cursor to start of line.
    pub fn move_to_line_start(&mut self) {
        self.anchor = None;
        self.preferred_col = None;
        self.position.col = 0;
    }

    /// Moves cursor to end of line.
    pub fn move_to_line_end(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;
        self.position.col = buffer.line_len_chars(self.position.line);
    }

    /// Moves cursor to first non-whitespace on line.
    pub fn move_to_first_non_whitespace(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        if let Some(pos) = buffer.first_non_whitespace(self.position.line) {
            self.position.col = pos.col;
        }
    }

    /// Moves cursor to start of word.
    pub fn move_to_word_start(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        let new_pos = buffer.word_start(self.position);
        self.position = new_pos;
    }

    /// Moves cursor to end of word.
    pub fn move_to_word_end(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        let new_pos = buffer.word_end(self.position);
        self.position = new_pos;
    }

    /// Moves cursor to previous word.
    pub fn move_word_left(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        // First, skip any whitespace to the left
        let mut idx = buffer.position_to_index(self.position);

        if idx == 0 {
            return;
        }

        let text = buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip whitespace
        while idx > 0 && chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        // Skip word characters
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        self.position = buffer.index_to_position(idx);
    }

    /// Moves cursor to next word.
    pub fn move_word_right(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        let mut idx = buffer.position_to_index(self.position);
        let len = buffer.len_chars();

        if idx >= len {
            return;
        }

        let text = buffer.text();
        let chars: Vec<char> = text.chars().collect();

        // Skip word characters
        while idx < len && !chars[idx].is_whitespace() {
            idx += 1;
        }

        // Skip whitespace
        while idx < len && chars[idx].is_whitespace() {
            idx += 1;
        }

        self.position = buffer.index_to_position(idx);
    }

    /// Moves cursor to start of buffer.
    pub fn move_to_buffer_start(&mut self) {
        self.anchor = None;
        self.preferred_col = None;
        self.position = Position::new(0, 0);
    }

    /// Moves cursor to end of buffer.
    pub fn move_to_buffer_end(&mut self, buffer: &Buffer) {
        self.anchor = None;
        self.preferred_col = None;

        let last_line = buffer.len_lines().saturating_sub(1);
        let last_col = buffer.line_len_chars(last_line);
        self.position = Position::new(last_line, last_col);
    }

    /// Moves cursor up by a page.
    pub fn move_page_up(&mut self, buffer: &Buffer, page_size: usize) {
        self.anchor = None;

        if self.preferred_col.is_none() {
            self.preferred_col = Some(self.position.col);
        }

        self.position.line = self.position.line.saturating_sub(page_size);
        let line_len = buffer.line_len_chars(self.position.line);
        self.position.col = self.preferred_col.unwrap_or(0).min(line_len);
    }

    /// Moves cursor down by a page.
    pub fn move_page_down(&mut self, buffer: &Buffer, page_size: usize) {
        self.anchor = None;

        if self.preferred_col.is_none() {
            self.preferred_col = Some(self.position.col);
        }

        let last_line = buffer.len_lines().saturating_sub(1);
        self.position.line = (self.position.line + page_size).min(last_line);
        let line_len = buffer.line_len_chars(self.position.line);
        self.position.col = self.preferred_col.unwrap_or(0).min(line_len);
    }

    /// Selects the word at the cursor position.
    pub fn select_word(&mut self, buffer: &Buffer) {
        let start = buffer.word_start(self.position);
        let end = buffer.word_end(self.position);

        self.anchor = Some(start);
        self.position = end;
        self.preferred_col = None;
    }

    /// Selects the entire current line.
    pub fn select_line(&mut self, buffer: &Buffer) {
        self.anchor = Some(Position::new(self.position.line, 0));

        let line_end = buffer.line_len_chars(self.position.line);
        self.position = Position::new(self.position.line, line_end);
        self.preferred_col = None;
    }

    /// Selects all text in the buffer.
    pub fn select_all(&mut self, buffer: &Buffer) {
        self.anchor = Some(Position::new(0, 0));

        let last_line = buffer.len_lines().saturating_sub(1);
        let last_col = buffer.line_len_chars(last_line);
        self.position = Position::new(last_line, last_col);
        self.preferred_col = None;
    }

    /// Clamps cursor position to buffer bounds.
    pub fn clamp(&mut self, buffer: &Buffer) {
        self.position = buffer.clamp_position(self.position);
        if let Some(anchor) = self.anchor {
            self.anchor = Some(buffer.clamp_position(anchor));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new();
        assert_eq!(cursor.position(), Position::new(0, 0));
        assert!(!cursor.has_selection());
    }

    #[test]
    fn test_cursor_move() {
        let buffer = Buffer::from_str("Hello\nWorld");
        let mut cursor = Cursor::new();

        cursor.move_right(&buffer);
        assert_eq!(cursor.position(), Position::new(0, 1));

        cursor.move_down(&buffer);
        assert_eq!(cursor.position(), Position::new(1, 1));
    }

    #[test]
    fn test_cursor_selection() {
        let _buffer = Buffer::from_str("Hello World");
        let mut cursor = Cursor::new();

        cursor.start_selection();
        cursor.extend_to(Position::new(0, 5));

        let (start, end) = cursor.selection_range().expect("has selection");
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(0, 5));
    }
}
