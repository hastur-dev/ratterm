//! Text buffer implementation using ropey.
//!
//! Provides efficient text storage and manipulation with undo/redo support.

use ropey::Rope;
use thiserror::Error;

use super::edit::Edit;
pub use super::edit::Position;
use super::find::{FindCaseInsensitiveIterator, FindIterator};

/// Maximum undo history size.
const MAX_UNDO_HISTORY: usize = 1000;

/// Buffer error type.
#[derive(Debug, Error)]
pub enum BufferError {
    /// Position out of bounds.
    #[error("Position out of bounds: line {line}, column {col}")]
    OutOfBounds { line: usize, col: usize },

    /// Invalid range.
    #[error("Invalid range")]
    InvalidRange,

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Text buffer with undo/redo support.
pub struct Buffer {
    /// The rope holding the text.
    rope: Rope,
    /// Undo stack.
    undo_stack: Vec<Vec<Edit>>,
    /// Redo stack.
    redo_stack: Vec<Vec<Edit>>,
    /// Current undo group.
    current_group: Vec<Edit>,
    /// Is grouping active.
    grouping: bool,
    /// Modified flag.
    modified: bool,
}

impl Buffer {
    /// Creates a new empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: Vec::new(),
            grouping: false,
            modified: false,
        }
    }

    /// Creates a buffer from a string.
    #[must_use]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: Vec::new(),
            grouping: false,
            modified: false,
        }
    }

    /// Returns the number of lines.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    /// Returns the number of characters.
    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Returns true if the buffer has been modified.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Marks the buffer as saved.
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Returns the full text.
    #[must_use]
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Returns a line by index.
    #[must_use]
    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx >= self.rope.len_lines() {
            return None;
        }
        Some(self.rope.line(line_idx).to_string())
    }

    /// Returns the length of a line including newline.
    #[must_use]
    pub fn line_len(&self, line_idx: usize) -> usize {
        if line_idx >= self.rope.len_lines() {
            return 0;
        }
        self.rope.line(line_idx).len_chars()
    }

    /// Returns the length of a line excluding trailing newline.
    #[must_use]
    pub fn line_len_chars(&self, line_idx: usize) -> usize {
        if line_idx >= self.rope.len_lines() {
            return 0;
        }
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    /// Converts a position to a character index.
    #[must_use]
    pub fn position_to_index(&self, pos: Position) -> usize {
        if pos.line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        let line_start = self.rope.line_to_char(pos.line);
        let line_len = self.line_len(pos.line);
        line_start + pos.col.min(line_len)
    }

    /// Converts a character index to a position.
    #[must_use]
    pub fn index_to_position(&self, idx: usize) -> Position {
        let idx = idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(idx);
        let line_start = self.rope.line_to_char(line);
        Position::new(line, idx - line_start)
    }

    /// Clamps a position to valid bounds.
    #[must_use]
    pub fn clamp_position(&self, pos: Position) -> Position {
        let line = pos.line.min(self.rope.len_lines().saturating_sub(1));
        let col = pos.col.min(self.line_len_chars(line));
        Position::new(line, col)
    }

    /// Inserts a character at the given position.
    pub fn insert_char(&mut self, pos: Position, c: char) {
        let idx = self.position_to_index(pos);
        self.insert_at_index(idx, &c.to_string());
    }

    /// Inserts a string at the given position.
    pub fn insert_str(&mut self, pos: Position, text: &str) {
        let idx = self.position_to_index(pos);
        self.insert_at_index(idx, text);
    }

    /// Inserts text at a character index.
    fn insert_at_index(&mut self, idx: usize, text: &str) {
        if text.is_empty() {
            return;
        }

        let idx = idx.min(self.rope.len_chars());

        let edit = Edit::Insert {
            pos: idx,
            text: text.to_string(),
        };
        self.push_edit(edit);

        self.rope.insert(idx, text);
        self.modified = true;
    }

    /// Deletes the character at the given position (forward delete).
    pub fn delete_char(&mut self, pos: Position) {
        let idx = self.position_to_index(pos);
        if idx < self.rope.len_chars() {
            self.delete_at_index(idx, 1);
        }
    }

    /// Deletes the character before the given position (backspace).
    pub fn delete_char_backward(&mut self, pos: Position) {
        let idx = self.position_to_index(pos);
        if idx > 0 {
            self.delete_at_index(idx - 1, 1);
        }
    }

    /// Deletes a range of text.
    pub fn delete_range(&mut self, start: Position, end: Position) {
        let start_idx = self.position_to_index(start);
        let end_idx = self.position_to_index(end);

        if start_idx >= end_idx {
            return;
        }

        self.delete_at_index(start_idx, end_idx - start_idx);
    }

    /// Deletes text at a character index.
    fn delete_at_index(&mut self, idx: usize, len: usize) {
        if len == 0 || idx >= self.rope.len_chars() {
            return;
        }

        let end_idx = (idx + len).min(self.rope.len_chars());
        let deleted = self.rope.slice(idx..end_idx).to_string();

        let edit = Edit::Delete {
            pos: idx,
            text: deleted,
        };
        self.push_edit(edit);

        self.rope.remove(idx..end_idx);
        self.modified = true;
    }

    /// Pushes an edit to the undo stack.
    fn push_edit(&mut self, edit: Edit) {
        self.redo_stack.clear();

        if self.grouping {
            self.current_group.push(edit);
        } else {
            self.undo_stack.push(vec![edit]);
            self.trim_undo_stack();
        }
    }

    /// Trims the undo stack to the maximum size.
    fn trim_undo_stack(&mut self) {
        while self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
    }

    /// Begins a group of edits for undo.
    pub fn begin_undo_group(&mut self) {
        if !self.grouping {
            self.grouping = true;
            self.current_group.clear();
        }
    }

    /// Ends a group of edits for undo.
    pub fn end_undo_group(&mut self) {
        if self.grouping {
            self.grouping = false;
            if !self.current_group.is_empty() {
                let group = std::mem::take(&mut self.current_group);
                self.undo_stack.push(group);
                self.trim_undo_stack();
            }
        }
    }

    /// Undoes the last edit or group.
    pub fn undo(&mut self) {
        if let Some(edits) = self.undo_stack.pop() {
            let mut inverses = Vec::new();

            for edit in edits.iter().rev() {
                self.apply_edit_raw(&edit.inverse());
                inverses.push(edit.clone());
            }

            self.redo_stack.push(inverses);
            self.modified = true;
        }
    }

    /// Redoes the last undone edit or group.
    pub fn redo(&mut self) {
        if let Some(edits) = self.redo_stack.pop() {
            let mut group = Vec::new();

            for edit in &edits {
                self.apply_edit_raw(edit);
                group.push(edit.clone());
            }

            self.undo_stack.push(group);
            self.modified = true;
        }
    }

    /// Applies an edit without recording it.
    fn apply_edit_raw(&mut self, edit: &Edit) {
        match edit {
            Edit::Insert { pos, text } => {
                let pos = (*pos).min(self.rope.len_chars());
                self.rope.insert(pos, text);
            }
            Edit::Delete { pos, text } => {
                let pos = (*pos).min(self.rope.len_chars());
                let end = (pos + text.len()).min(self.rope.len_chars());
                if pos < end {
                    self.rope.remove(pos..end);
                }
            }
        }
    }

    /// Replaces text in a range.
    pub fn replace(&mut self, start: Position, end: Position, text: &str) {
        self.begin_undo_group();
        self.delete_range(start, end);
        self.insert_str(start, text);
        self.end_undo_group();
    }

    /// Replaces all occurrences of a pattern.
    pub fn replace_all(&mut self, pattern: &str, replacement: &str) -> usize {
        if pattern.is_empty() {
            return 0;
        }

        let matches: Vec<_> = self.find(pattern).collect();
        let count = matches.len();

        if count == 0 {
            return 0;
        }

        self.begin_undo_group();

        for pos in matches.into_iter().rev() {
            let end_idx = self.position_to_index(pos) + pattern.len();
            let end = self.index_to_position(end_idx);
            self.delete_range(pos, end);
            self.insert_str(pos, replacement);
        }

        self.end_undo_group();

        count
    }

    /// Finds all occurrences of a pattern.
    pub fn find<'a>(&'a self, pattern: &'a str) -> impl Iterator<Item = Position> + 'a {
        FindIterator::new(self, pattern)
    }

    /// Finds all occurrences of a pattern (case insensitive).
    pub fn find_case_insensitive<'a>(
        &'a self,
        pattern: &'a str,
    ) -> impl Iterator<Item = Position> + 'a {
        FindCaseInsensitiveIterator::new(self, pattern)
    }

    /// Gets text in a range.
    #[must_use]
    pub fn get_range(&self, start: Position, end: Position) -> Option<String> {
        let start_idx = self.position_to_index(start);
        let end_idx = self.position_to_index(end);

        if start_idx >= end_idx {
            return None;
        }

        Some(self.rope.slice(start_idx..end_idx).to_string())
    }

    /// Returns the start of the word at position.
    #[must_use]
    pub fn word_start(&self, pos: Position) -> Position {
        let idx = self.position_to_index(pos);
        let mut start = idx;

        let chars: Vec<char> = self.rope.chars().collect();
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }

        self.index_to_position(start)
    }

    /// Returns the end of the word at position.
    #[must_use]
    pub fn word_end(&self, pos: Position) -> Position {
        let idx = self.position_to_index(pos);
        let mut end = idx;
        let len = self.rope.len_chars();

        let chars: Vec<char> = self.rope.chars().collect();
        while end < len && !chars[end].is_whitespace() {
            end += 1;
        }

        self.index_to_position(end)
    }

    /// Returns the position at the start of a line.
    #[must_use]
    pub fn line_start(&self, line: usize) -> Position {
        Position::new(line, 0)
    }

    /// Returns the position at the end of a line.
    #[must_use]
    pub fn line_end(&self, line: usize) -> Position {
        Position::new(line, self.line_len_chars(line))
    }

    /// Returns the first non-whitespace position on a line.
    #[must_use]
    pub fn first_non_whitespace(&self, line: usize) -> Option<Position> {
        if line >= self.len_lines() {
            return None;
        }

        let line_text = self.rope.line(line);
        for (i, c) in line_text.chars().enumerate() {
            if !c.is_whitespace() {
                return Some(Position::new(line, i));
            }
        }

        None
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len_lines(), 1);
    }

    #[test]
    fn test_buffer_from_str() {
        let buffer = Buffer::from_str("Hello\nWorld");
        assert_eq!(buffer.len_lines(), 2);
        assert_eq!(buffer.line(0), Some("Hello\n".to_string()));
        assert_eq!(buffer.line(1), Some("World".to_string()));
    }

    #[test]
    fn test_buffer_insert() {
        let mut buffer = Buffer::from_str("Hello");
        buffer.insert_char(Position::new(0, 5), '!');
        assert_eq!(buffer.text(), "Hello!");
    }

    #[test]
    fn test_buffer_undo() {
        let mut buffer = Buffer::from_str("Hello");
        buffer.insert_char(Position::new(0, 5), '!');
        buffer.undo();
        assert_eq!(buffer.text(), "Hello");
    }

    #[test]
    fn test_buffer_redo() {
        let mut buffer = Buffer::from_str("Hello");
        buffer.insert_char(Position::new(0, 5), '!');
        buffer.undo();
        buffer.redo();
        assert_eq!(buffer.text(), "Hello!");
    }
}
