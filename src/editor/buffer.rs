//! Text buffer implementation using ropey.
//!
//! Provides efficient text storage and manipulation with undo/redo support.

use ropey::Rope;
use thiserror::Error;

use super::edit::Edit;
pub use super::edit::Position;

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
#[derive(Debug, Clone)]
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
    /// Monotonic counter bumped by every mutation.
    ///
    /// Caches that are derived from the text — the syntax tree, the fold
    /// ranges — compare this against the revision they were built from to know
    /// whether they are stale. It changes on undo and redo too, because those
    /// change the text.
    revision: u64,
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
            revision: 0,
        }
    }

    /// Creates a buffer from a string.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: Vec::new(),
            grouping: false,
            modified: false,
            revision: 0,
        }
    }

    /// Returns the mutation counter.
    ///
    /// Two buffers are unrelated, so the value is only meaningful when compared
    /// against an earlier reading of the same buffer.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the text length in bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Converts a character index to a byte offset.
    ///
    /// Tree-sitter works in bytes while every position in this editor is a
    /// character offset, so the conversion has to be cheap: the rope answers it
    /// in logarithmic time rather than by walking the text.
    #[must_use]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.rope.len_chars()))
    }

    /// Converts a byte offset to a character index.
    #[must_use]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.rope.len_bytes()))
    }

    /// Returns the byte offset at which a line starts.
    #[must_use]
    pub fn line_to_byte(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_bytes();
        }
        self.rope.line_to_byte(line)
    }

    /// Returns the contiguous chunk of text that starts at `byte`.
    ///
    /// The slice is whatever the rope holds contiguously from that offset; a
    /// reader must call again with the next offset until it gets an empty
    /// slice. This is what lets tree-sitter reparse without the whole document
    /// being copied into a `String` first.
    #[must_use]
    pub fn chunk_at_byte(&self, byte: usize) -> &str {
        if byte >= self.rope.len_bytes() {
            return "";
        }
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(byte);
        let offset = byte - chunk_start;
        // A caller that asks for the middle of a multi-byte character has a
        // bug; returning nothing ends the read instead of panicking mid-render.
        if !chunk.is_char_boundary(offset) {
            return "";
        }
        &chunk[offset..]
    }

    /// Returns the bytes covering `range`, clamped to the buffer.
    #[must_use]
    pub fn byte_range(&self, range: std::ops::Range<usize>) -> Vec<u8> {
        let len = self.rope.len_bytes();
        let start = range.start.min(len);
        let end = range.end.clamp(start, len);
        let mut out = Vec::with_capacity(end - start);
        let mut at = start;
        while at < end {
            let chunk = self.chunk_at_byte(at);
            if chunk.is_empty() {
                break;
            }
            let take = chunk.len().min(end - at);
            out.extend_from_slice(&chunk.as_bytes()[..take]);
            at += take;
        }
        out
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
        self.revision = self.revision.wrapping_add(1);
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
        self.revision = self.revision.wrapping_add(1);
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
        self.revision = self.revision.wrapping_add(1);
        match edit {
            Edit::Insert { pos, text } => {
                let pos = (*pos).min(self.rope.len_chars());
                self.rope.insert(pos, text);
            }
            Edit::Delete { pos, text } => {
                let pos = (*pos).min(self.rope.len_chars());
                // Rope indices are character offsets, so the span to remove is
                // the character count of the recorded text, not its byte length.
                let end = (pos + text.chars().count()).min(self.rope.len_chars());
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

        let pattern_chars = pattern.chars().count();
        for pos in matches.into_iter().rev() {
            let end_idx = self.position_to_index(pos) + pattern_chars;
            let end = self.index_to_position(end_idx);
            self.delete_range(pos, end);
            self.insert_str(pos, replacement);
        }

        self.end_undo_group();

        count
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

mod query;

#[cfg(test)]
mod tests;
