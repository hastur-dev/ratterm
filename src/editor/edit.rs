//! Edit operations and position types for the text buffer.
//!
//! Contains the Position type and Edit enum for undo/redo support.

/// A position in the buffer (line, column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column (character offset, 0-indexed).
    pub col: usize,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// An edit operation for undo/redo.
#[derive(Debug, Clone)]
pub enum Edit {
    /// Insert text at position.
    Insert { pos: usize, text: String },
    /// Delete text at position.
    Delete { pos: usize, text: String },
}

impl Edit {
    /// Returns the inverse of this edit.
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { pos, text } => Self::Delete {
                pos: *pos,
                text: text.clone(),
            },
            Self::Delete { pos, text } => Self::Insert {
                pos: *pos,
                text: text.clone(),
            },
        }
    }

    /// Returns the position of this edit.
    #[must_use]
    pub const fn pos(&self) -> usize {
        match self {
            Self::Insert { pos, .. } | Self::Delete { pos, .. } => *pos,
        }
    }

    /// Returns the text of this edit.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Insert { text, .. } | Self::Delete { text, .. } => text,
        }
    }
}
