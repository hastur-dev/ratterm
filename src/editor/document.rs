//! Opening, saving, and swapping the document the editor holds.
//!
//! Split out of `mod.rs` so neither file grows past the project's size limit.

use std::path::PathBuf;

use super::Editor;
use super::buffer::Buffer;
use super::state::EditorState;
use crate::remote::RemoteFile;

impl Editor {
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
    /// the pane on screen. Language, folds, and indentation travel with the
    /// document; search and secondary cursors do not, because they describe
    /// what the user is doing right now rather than the file.
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
            folds: std::mem::replace(&mut self.folds, state.folds),
            indent_style: std::mem::replace(&mut self.indent_style, state.indent_style),
        };

        self.search.clear();
        self.cursors.clear();
        self.vim.reset();
        self.emacs.reset_transient();
        self.view.update_gutter_width(self.buffer.len_lines());
        self.retune_for_document();
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
            folds: self.folds.clone(),
            indent_style: self.indent_style,
        }
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
        // The extension may have changed, and with it the language.
        self.retune_for_document();
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
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::EditorMode;
    use crate::editor::highlight::Language;

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
    fn swap_state_carries_the_language_and_folds_of_each_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rust = dir.path().join("a.rs");
        std::fs::write(&rust, "fn f() {\n    g();\n}\n").expect("write");
        let text = dir.path().join("b.txt");
        std::fs::write(&text, "plain\n").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.open(&rust).expect("open rust");
        assert_eq!(editor.language(), Language::Rust);
        assert!(editor.toggle_fold(), "the rust file has a foldable region");
        let parked = editor.take_state();

        editor.open(&text).expect("open text");
        assert_eq!(editor.language(), Language::PlainText);

        editor.restore_state(parked);
        assert_eq!(editor.language(), Language::Rust);
        assert!(
            editor.folds().is_collapsed(0),
            "the collapsed region came back with the document"
        );
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
    fn small_files_open_writable_and_pick_up_their_language() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("small.rs");
        std::fs::write(&path, "fn main() {}\n").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.open(&path).expect("open");
        assert!(!editor.is_read_only());
        assert_eq!(editor.buffer().text(), "fn main() {}\n");
        assert_eq!(editor.language(), Language::Rust);
        assert!(editor.has_syntax_tree());
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

    #[test]
    fn save_as_re_reads_the_language_from_the_new_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut editor = Editor::new(80, 24);
        editor.insert_str("fn main() {}\n");
        assert_eq!(editor.language(), Language::PlainText);

        editor.save_as(dir.path().join("out.rs")).expect("save");
        assert_eq!(editor.language(), Language::Rust);
        assert!(!editor.is_modified());
    }

    #[test]
    fn saving_without_a_path_reports_it_instead_of_failing() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("x");
        editor.save().expect("no io happens");
        assert_eq!(editor.status(), "No file path set");
        assert!(editor.is_modified());
    }

    #[test]
    fn a_new_buffer_clears_everything_about_the_old_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("x.rs");
        std::fs::write(&path, "fn f() {\n    g();\n}\n").expect("write");

        let mut editor = Editor::new(80, 24);
        editor.open(&path).expect("open");
        editor.new_buffer();
        assert!(editor.buffer().is_empty());
        assert!(editor.path().is_none());
        assert_eq!(editor.language(), Language::PlainText);
        assert!(editor.folds().ranges().is_empty());
    }
}
