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
pub mod state;
pub mod view;

use std::path::PathBuf;

use self::buffer::Buffer;
use self::cursor::Cursor;
use self::view::View;

pub use self::edit::Position;
pub use self::state::EditorState;

use crate::remote::RemoteFile;

/// Files at or above this size open read-only.
///
/// Editing multi-megabyte files through a rope is possible but the undo
/// history and the per-keystroke re-render make it unpleasant, and the usual
/// reason a file this big is opened in an IDE is to look at it.
pub const DEFAULT_READ_ONLY_THRESHOLD_BYTES: u64 = 8 * 1024 * 1024;

/// Files at or above this size are refused outright.
pub const DEFAULT_MAX_OPEN_BYTES: u64 = 512 * 1024 * 1024;

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
    /// Whether edits are rejected for the current document.
    read_only: bool,
    /// Size at or above which a file opens read-only.
    read_only_threshold_bytes: u64,
    /// Size at or above which a file is refused.
    max_open_bytes: u64,
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
            read_only: false,
            read_only_threshold_bytes: DEFAULT_READ_ONLY_THRESHOLD_BYTES,
            max_open_bytes: DEFAULT_MAX_OPEN_BYTES,
        }
    }

    /// Overrides the large-file thresholds.
    ///
    /// `read_only` must not exceed `max_open`; the values are swapped if they
    /// arrive the wrong way round so a misconfiguration cannot make every file
    /// unopenable.
    pub fn set_size_limits(&mut self, read_only: u64, max_open: u64) {
        let (lo, hi) = if read_only <= max_open {
            (read_only, max_open)
        } else {
            (max_open, read_only)
        };
        self.read_only_threshold_bytes = lo;
        self.max_open_bytes = hi;
    }

    /// Returns the size at or above which a file opens read-only.
    #[must_use]
    pub const fn read_only_threshold_bytes(&self) -> u64 {
        self.read_only_threshold_bytes
    }

    /// Returns the size at or above which a file is refused.
    #[must_use]
    pub const fn max_open_bytes(&self) -> u64 {
        self.max_open_bytes
    }

    /// Returns true if the current document rejects edits.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Marks the current document read-only (or writable again).
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Returns true when an edit must be refused, recording why.
    fn reject_edit(&mut self) -> bool {
        if self.read_only {
            self.set_status("Read-only buffer");
            true
        } else {
            false
        }
    }

    /// Detaches the current document, leaving the editor empty.
    ///
    /// The viewport size is preserved so the empty editor still matches the
    /// pane it is drawn into.
    pub fn take_state(&mut self) -> EditorState {
        let width = self.view.width();
        let height = self.view.height();
        self.swap_state(EditorState::empty(width, height))
    }

    /// Installs `state` as the current document, returning the previous one.
    ///
    /// The incoming viewport is resized to the editor's current dimensions:
    /// scroll position belongs to the document, but width and height belong to
    /// the pane on screen.
    pub fn swap_state(&mut self, mut state: EditorState) -> EditorState {
        let width = self.view.width();
        let height = self.view.height();
        state.view.resize(width, height);

        let previous = EditorState {
            buffer: std::mem::replace(&mut self.buffer, state.buffer),
            cursor: std::mem::replace(&mut self.cursor, state.cursor),
            view: std::mem::replace(&mut self.view, state.view),
            mode: std::mem::replace(&mut self.mode, state.mode),
            path: std::mem::replace(&mut self.path, state.path),
            remote_file: std::mem::replace(&mut self.remote_file, state.remote_file),
            read_only: std::mem::replace(&mut self.read_only, state.read_only),
        };

        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
        previous
    }

    /// Installs `state` as the current document, discarding the previous one.
    pub fn restore_state(&mut self, state: EditorState) {
        let _ = self.swap_state(state);
    }

    /// Returns a snapshot of the current document.
    ///
    /// Cloning a rope is cheap; this is used to seed a new tab from the live
    /// editor without disturbing it.
    #[must_use]
    pub fn state_snapshot(&self) -> EditorState {
        EditorState {
            buffer: self.buffer.clone(),
            cursor: self.cursor.clone(),
            view: self.view.clone(),
            mode: self.mode,
            path: self.path.clone(),
            remote_file: self.remote_file.clone(),
            read_only: self.read_only,
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
    /// Files at or above [`Editor::read_only_threshold_bytes`] load but reject
    /// edits; files at or above [`Editor::max_open_bytes`] are refused with
    /// [`std::io::ErrorKind::FileTooLarge`] rather than being read into memory.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read, is not valid UTF-8, or
    /// exceeds the maximum open size.
    pub fn open(&mut self, path: impl Into<PathBuf>) -> std::io::Result<()> {
        let path = path.into();
        let mut state = self.load_state_from_disk(&path)?;
        // Opening a file does not change how the user is editing.
        state.mode = self.mode;
        self.restore_state(state);
        Ok(())
    }

    /// Reads `path` into a detached document without touching the editor.
    ///
    /// # Errors
    /// Same conditions as [`Editor::open`].
    pub fn load_state_from_disk(&self, path: &std::path::Path) -> std::io::Result<EditorState> {
        let size = std::fs::metadata(path)?.len();

        if size >= self.max_open_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                format!(
                    "{} is {} bytes; the maximum ratterm will open is {} bytes",
                    path.display(),
                    size,
                    self.max_open_bytes
                ),
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let read_only = size >= self.read_only_threshold_bytes;

        let mut state = EditorState::empty(self.view.width(), self.view.height());
        state.buffer = Buffer::from_str(&content);
        state.path = Some(path.to_path_buf());
        state.read_only = read_only;
        state.view.update_gutter_width(state.buffer.len_lines());
        Ok(state)
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
        self.restore_state(self.remote_state(content, remote_file));
    }

    /// Builds a detached document from remote-file content.
    #[must_use]
    pub fn remote_state(&self, content: &str, remote_file: RemoteFile) -> EditorState {
        let mut state = EditorState::empty(self.view.width(), self.view.height());
        state.buffer = Buffer::from_str(content);
        state.path = Some(remote_file.local_cache_path.clone());
        state.remote_file = Some(remote_file);
        state.view.update_gutter_width(state.buffer.len_lines());
        state
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
        let width = self.view.width();
        let height = self.view.height();
        self.restore_state(EditorState::empty(width, height));
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
        if self.reject_edit() {
            return;
        }
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
        if self.reject_edit() {
            return;
        }
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
        if self.reject_edit() {
            return;
        }
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
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        self.buffer.delete_char(pos);
        self.view.update_gutter_width(self.buffer.len_lines());
    }

    /// Deletes the selected text.
    pub fn delete_selection(&mut self) {
        if self.reject_edit() {
            return;
        }
        if let Some((start, end)) = self.cursor.selection_range() {
            self.buffer.delete_range(start, end);
            self.cursor.move_to(start);
            self.view.update_gutter_width(self.buffer.len_lines());
            self.ensure_cursor_visible();
        }
    }

    /// Deletes from the cursor to the end of the line (Emacs Ctrl+K).
    pub fn delete_to_line_end(&mut self) {
        if self.reject_edit() {
            return;
        }
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
        if self.reject_edit() {
            return;
        }
        self.buffer.undo();
        self.cursor.clamp(&self.buffer);
        self.view.update_gutter_width(self.buffer.len_lines());
        self.ensure_cursor_visible();
    }

    /// Redoes the last undone edit.
    pub fn redo(&mut self) {
        if self.reject_edit() {
            return;
        }
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
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

    #[test]
    fn swap_state_round_trips_unsaved_text_and_undo_history() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("first document");

        let parked = editor.take_state();
        assert_eq!(editor.buffer().text(), "");
        assert!(!editor.is_modified());

        editor.insert_str("second document");
        assert_eq!(editor.buffer().text(), "second document");

        let second = editor.swap_state(parked);
        assert_eq!(editor.buffer().text(), "first document");
        assert!(editor.is_modified());
        // Undo history came back with the document.
        editor.undo();
        assert_eq!(editor.buffer().text(), "");
        assert_eq!(second.buffer.text(), "second document");
    }

    #[test]
    fn swap_state_keeps_the_live_viewport_size() {
        let mut editor = Editor::new(120, 40);
        let mut parked = EditorState::empty(20, 5);
        parked.buffer = Buffer::from_str("x\n".repeat(200).as_str());

        editor.restore_state(parked);
        assert_eq!(editor.view().width(), 120);
        assert_eq!(editor.view().height(), 40);
    }

    #[test]
    fn swap_state_preserves_cursor_position() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("line one\nline two");
        let pos = editor.cursor_position();
        let parked = editor.take_state();
        editor.restore_state(parked);
        assert_eq!(editor.cursor_position(), pos);
    }

    #[test]
    fn state_snapshot_does_not_disturb_the_editor() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("abc");
        let snap = editor.state_snapshot();
        assert_eq!(snap.buffer.text(), "abc");
        assert_eq!(editor.buffer().text(), "abc");
    }

    #[test]
    fn read_only_buffer_rejects_every_mutation() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("original");
        editor.set_read_only(true);

        editor.insert_char('x');
        editor.insert_str("more");
        editor.backspace();
        editor.delete();
        editor.delete_to_line_end();
        editor.duplicate_line();
        editor.delete_line();
        editor.indent();
        editor.outdent();
        editor.toggle_comment();
        editor.undo();
        editor.redo();

        assert_eq!(editor.buffer().text(), "original");
        assert_eq!(editor.status(), "Read-only buffer");
    }

    #[test]
    fn clearing_read_only_restores_editing() {
        let mut editor = Editor::new(80, 24);
        editor.set_read_only(true);
        editor.insert_char('a');
        assert_eq!(editor.buffer().text(), "");
        editor.set_read_only(false);
        editor.insert_char('a');
        assert_eq!(editor.buffer().text(), "a");
    }

    #[test]
    fn small_files_open_writable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("small.txt");
        std::fs::write(&path, "hello").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.open(&path).expect("open");
        assert!(!editor.is_read_only());
        assert_eq!(editor.buffer().text(), "hello");
    }

    #[test]
    fn files_over_the_threshold_open_read_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("big.txt");
        std::fs::write(&path, "0123456789").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.set_size_limits(4, 1024);
        editor.open(&path).expect("open");
        assert!(editor.is_read_only());
        editor.insert_char('x');
        assert_eq!(editor.buffer().text(), "0123456789");
    }

    #[test]
    fn files_over_the_hard_limit_are_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("huge.txt");
        std::fs::write(&path, "0123456789").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.set_size_limits(2, 4);
        let err = editor.open(&path).expect_err("must refuse");
        assert_eq!(err.kind(), std::io::ErrorKind::FileTooLarge);
        // The refused open left the editor untouched.
        assert!(editor.buffer().is_empty());
    }

    #[test]
    fn size_limits_are_ordered_even_when_supplied_backwards() {
        let mut editor = Editor::new(80, 24);
        editor.set_size_limits(1000, 10);
        assert_eq!(editor.read_only_threshold_bytes(), 10);
        assert_eq!(editor.max_open_bytes(), 1000);
    }

    #[test]
    fn opening_a_missing_file_is_an_error_and_leaves_state_alone() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("keep me");
        let err = editor.open("definitely-not-a-real-path-9f3a.txt");
        assert!(err.is_err());
        assert_eq!(editor.buffer().text(), "keep me");
    }

    #[test]
    fn opening_a_file_keeps_the_current_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("m.txt");
        std::fs::write(&path, "x").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.set_mode(EditorMode::Insert);
        editor.open(&path).expect("open");
        assert_eq!(editor.mode(), EditorMode::Insert);
    }
}
