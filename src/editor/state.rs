//! Detachable editor state.
//!
//! One [`Editor`](super::Editor) is shared by every open tab. Without a place
//! to park the state of the tabs that are not on screen, switching tabs had to
//! re-read the file from disk, which silently discarded unsaved edits and the
//! whole undo history.
//!
//! [`EditorState`] is that place: it owns everything that belongs to a single
//! document, so the application can swap documents in and out of the editor
//! without touching the filesystem.

use std::path::{Path, PathBuf};

use super::buffer::Buffer;
use super::cursor::Cursor;
use super::fold::FoldState;
use super::indent::IndentStyle;
use super::view::View;
use super::{EditorMode, Position};
use crate::remote::RemoteFile;

/// Everything that belongs to one open document.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Text buffer, including its undo and redo stacks.
    pub buffer: Buffer,
    /// Cursor position and selection.
    pub cursor: Cursor,
    /// Viewport scroll position and gutter width.
    pub view: View,
    /// Editing mode (Vim normal/insert/visual/command).
    pub mode: EditorMode,
    /// On-disk path, if the document has one.
    pub path: Option<PathBuf>,
    /// Remote-file metadata when the document was fetched over SFTP.
    pub remote_file: Option<RemoteFile>,
    /// Whether edits are rejected (set for oversized files).
    pub read_only: bool,
    /// Foldable regions and which of them the user collapsed.
    pub folds: FoldState,
    /// Indentation style detected for this document.
    pub indent_style: IndentStyle,
}

impl EditorState {
    /// Creates an empty state sized for the given viewport.
    #[must_use]
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            view: View::new(width.max(1), height.max(1)),
            mode: EditorMode::Normal,
            path: None,
            remote_file: None,
            read_only: false,
            folds: FoldState::default(),
            indent_style: IndentStyle::default(),
        }
    }

    /// Returns true if the document has unsaved changes.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Returns the document path, if any.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the cursor position.
    #[must_use]
    pub fn cursor_position(&self) -> Position {
        self.cursor.position()
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::empty(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_is_clean() {
        let state = EditorState::empty(80, 24);
        assert!(!state.is_modified());
        assert!(state.path().is_none());
        assert!(!state.read_only);
        assert_eq!(state.cursor_position(), Position::new(0, 0));
    }

    #[test]
    fn empty_state_clamps_zero_dimensions() {
        let state = EditorState::empty(0, 0);
        assert_eq!(state.view.width(), 1);
        assert_eq!(state.view.height(), 1);
    }

    #[test]
    fn modified_flag_follows_the_buffer() {
        let mut state = EditorState::empty(80, 24);
        state.buffer.insert_char(Position::new(0, 0), 'x');
        assert!(state.is_modified());
        state.buffer.mark_saved();
        assert!(!state.is_modified());
    }

    #[test]
    fn clone_detaches_the_buffer() {
        let mut state = EditorState::empty(80, 24);
        state.buffer.insert_char(Position::new(0, 0), 'a');
        let other = state.clone();
        state.buffer.insert_char(Position::new(0, 1), 'b');
        assert_eq!(state.buffer.text(), "ab");
        assert_eq!(other.buffer.text(), "a");
    }
}
