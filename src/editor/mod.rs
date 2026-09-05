//! Code editor module.
//!
//! The editor is an API layer: every behaviour a user can trigger is a method
//! here or in one of the sibling modules, tested without a terminal. The input
//! layer translates key events into these calls and the widget draws what they
//! produce; neither holds editing logic of its own.

pub mod bracket_scan;
pub mod brackets;
pub mod buffer;
pub mod cursor;
pub mod decor;
pub mod edit;
mod editing;
pub mod emacs;
pub mod emacs_commands;
pub mod emacs_exec;
pub mod emacs_keys;
pub mod find;
pub mod fold;
pub mod fold_ops;
pub mod highlight;
pub mod highlight_edit;
pub mod indent;
pub mod indent_ops;
pub mod language;
mod movement;
pub mod multicursor;
pub mod multicursor_ops;
pub mod search;
pub mod search_ops;
mod selection;
pub mod state;
pub mod syntax;
mod text_ops;
pub mod typing;
pub mod view;
pub mod vim;

mod document;

use std::cell::RefCell;
use std::path::PathBuf;

use self::brackets::AutoPair;
use self::buffer::Buffer;
use self::cursor::Cursor;
use self::emacs_exec::EmacsState;
use self::fold::FoldState;
use self::indent::IndentStyle;
use self::language::Language;
use self::multicursor::MultiCursor;
use self::search::SearchState;
use self::syntax::Syntax;
use self::view::View;
use self::vim::{Registers, VimState};

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

impl EditorMode {
    /// Returns the short name shown in the status bar.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::Command => "COMMAND",
        }
    }
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
    /// Parsed syntax tree, behind a cell so the renderer can repair it.
    pub(crate) syntax: RefCell<Syntax>,
    /// Which regions are foldable and which are collapsed.
    pub(crate) folds: FoldState,
    /// Line count the fold regions were derived from.
    pub(crate) folds_line_count: usize,
    /// True when an edit may have moved a fold boundary.
    pub(crate) folds_stale: bool,
    /// Indentation style detected for this document.
    pub(crate) indent_style: IndentStyle,
    /// Language the document is treated as.
    pub(crate) language: Language,
    /// Auto-pairing policy.
    pub(crate) auto_pair: AutoPair,
    /// Search and replace state.
    pub(crate) search: SearchState,
    /// Secondary cursors.
    pub(crate) cursors: MultiCursor,
    /// Vim key-sequence parser.
    pub(crate) vim: VimState,
    /// Vim registers.
    pub(crate) registers: Registers,
    /// Emacs kill ring, mark, and prefix state.
    pub(crate) emacs: EmacsState,
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
            syntax: RefCell::new(Syntax::new()),
            folds: FoldState::default(),
            folds_line_count: 1,
            folds_stale: false,
            indent_style: IndentStyle::default(),
            language: Language::PlainText,
            auto_pair: AutoPair::default(),
            search: SearchState::new(),
            cursors: MultiCursor::new(),
            vim: VimState::new(),
            registers: Registers::new(),
            emacs: EmacsState::new(),
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
    pub(crate) fn reject_edit(&mut self) -> bool {
        if self.read_only {
            self.set_status("Read-only buffer");
            true
        } else {
            false
        }
    }

    /// Returns the buffer.
    #[must_use]
    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Returns a mutable buffer reference.
    ///
    /// Changes made this way are noticed by the next render through the
    /// buffer's revision counter, at the cost of one full reparse.
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

    /// Returns the auto-pairing policy.
    #[must_use]
    pub const fn auto_pair(&self) -> AutoPair {
        self.auto_pair
    }

    /// Replaces the auto-pairing policy.
    pub fn set_auto_pair(&mut self, pairs: AutoPair) {
        self.auto_pair = pairs;
    }

    /// Returns true if the buffer is modified.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
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
        assert_eq!(editor.language(), Language::PlainText);
    }

    #[test]
    fn mode_labels_are_stable() {
        assert_eq!(EditorMode::Normal.label(), "NORMAL");
        assert_eq!(EditorMode::Insert.label(), "INSERT");
        assert_eq!(EditorMode::Visual.label(), "VISUAL");
        assert_eq!(EditorMode::Command.label(), "COMMAND");
    }

    #[test]
    fn size_limits_are_ordered_even_when_supplied_backwards() {
        let mut editor = Editor::new(80, 24);
        editor.set_size_limits(1000, 10);
        assert_eq!(editor.read_only_threshold_bytes(), 10);
        assert_eq!(editor.max_open_bytes(), 1000);
    }

    #[test]
    fn resizing_keeps_the_cursor_on_screen() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(&"x\n".repeat(100));
        editor.resize(40, 10);
        assert_eq!(editor.view().width(), 40);
        assert_eq!(editor.view().height(), 10);
        assert!(editor.view().is_line_visible(editor.cursor_position().line));
    }

    #[test]
    fn the_status_line_round_trips() {
        let mut editor = Editor::new(80, 24);
        assert_eq!(editor.status(), "");
        editor.set_status("hello");
        assert_eq!(editor.status(), "hello");
    }

    #[test]
    fn auto_pairing_can_be_turned_off() {
        let mut editor = Editor::new(80, 24);
        assert!(editor.auto_pair().enabled);
        editor.set_auto_pair(AutoPair {
            enabled: false,
            ..AutoPair::default()
        });
        assert!(!editor.auto_pair().enabled);
    }
}
