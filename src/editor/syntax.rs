//! The editor's view of the syntax tree.
//!
//! [`Syntax`] pairs a [`Highlighter`] with the buffer revision it was parsed
//! from. Edits made through the editor's own methods report themselves, so the
//! tree is updated incrementally; anything that reaches the buffer another way
//! — `buffer_mut()`, the IPC API — is caught by the revision check and repaired
//! with one full reparse rather than being silently drawn with stale colours.

use tree_sitter::InputEdit;

use super::buffer::Buffer;
use super::fold::{FoldState, compute_folds};
use super::highlight::{HighlightSpan, Highlighter, Language};
use super::indent::detect_indent;
use super::{Editor, Position};

/// A parsed tree plus the buffer revision it belongs to.
#[derive(Debug)]
pub struct Syntax {
    highlighter: Highlighter,
    /// Buffer revision the tree's *edits* have been carried up to.
    revision: u64,
    /// True when edits have been recorded against the tree but not yet parsed.
    ///
    /// Parsing is deferred to the first request for spans, so a compound edit —
    /// a replace-all over a hundred matches, a fan-out to twenty cursors —
    /// costs one reparse rather than one per sub-edit.
    pending: bool,
    /// False once a change reached the buffer without a matching edit report,
    /// so the next sync throws the tree away instead of reusing it.
    tree_trusted: bool,
}

impl Default for Syntax {
    fn default() -> Self {
        Self::new()
    }
}

impl Syntax {
    /// Creates a plain-text syntax state with nothing parsed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::plain(),
            revision: u64::MAX,
            pending: false,
            tree_trusted: true,
        }
    }

    /// Returns the language currently parsed.
    #[must_use]
    pub fn language(&self) -> Language {
        self.highlighter.language()
    }

    /// Returns true once a tree exists.
    #[must_use]
    pub fn has_tree(&self) -> bool {
        self.highlighter.has_tree()
    }

    /// Switches language and reparses `buffer` from scratch.
    ///
    /// A grammar that fails to load leaves the state on plain text rather than
    /// propagating an error into the render path; the file still opens, it just
    /// has no colours.
    pub fn set_language(&mut self, language: Language, buffer: &Buffer) {
        if let Err(error) = self.highlighter.set_language(language) {
            tracing::warn!("syntax highlighting unavailable: {error}");
            let _ = self.highlighter.set_language(Language::PlainText);
        }
        self.tree_trusted = true;
        self.reparse(buffer);
    }

    /// Records an edit against the tree, deferring the reparse.
    pub fn note_edit(&mut self, edit: &InputEdit, buffer: &Buffer) {
        if self.revision == buffer.revision() {
            return;
        }
        self.highlighter.edit(edit);
        self.revision = buffer.revision();
        self.pending = true;
    }

    /// Records that the buffer changed in a way the tree cannot absorb.
    ///
    /// The next sync starts from scratch.
    pub fn invalidate(&mut self) {
        self.highlighter.invalidate();
        self.tree_trusted = false;
        self.pending = true;
    }

    /// Reparses and marks the tree trustworthy again.
    fn reparse(&mut self, buffer: &Buffer) {
        if !self.tree_trusted {
            self.highlighter.invalidate();
        }
        self.highlighter.parse(buffer);
        self.revision = buffer.revision();
        self.pending = false;
        self.tree_trusted = true;
    }

    /// Brings the tree up to date with `buffer`.
    ///
    /// A revision the tree has not been told about means something bypassed
    /// [`Syntax::note_edit`], so the tree is thrown away rather than reused.
    pub fn sync(&mut self, buffer: &Buffer) {
        if self.revision != buffer.revision() {
            self.tree_trusted = false;
            self.reparse(buffer);
        } else if self.pending {
            self.reparse(buffer);
        }
    }

    /// Returns true when a reparse is owed. Used by tests.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    /// Returns the spans for one line, syncing first if needed.
    pub fn highlight_line(&mut self, buffer: &Buffer, line: usize) -> Vec<HighlightSpan> {
        self.sync(buffer);
        self.highlighter.highlight_line(buffer, line)
    }
}

impl Editor {
    /// Returns the language the current document is highlighted as.
    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Switches the document's language, reparsing and refolding.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        self.syntax
            .borrow_mut()
            .set_language(language, &self.buffer);
        self.refresh_folds();
    }

    /// Re-derives everything that depends on the document as a whole.
    ///
    /// Called after a file is opened or a tab is switched in: the language
    /// comes from the path, the indent style from the text, and the fold
    /// ranges from both.
    pub fn retune_for_document(&mut self) {
        let language = self
            .path
            .as_deref()
            .map_or(Language::PlainText, Language::from_path);
        self.language = language;
        self.indent_style = detect_indent(&self.buffer);
        self.syntax
            .borrow_mut()
            .set_language(language, &self.buffer);
        self.refresh_folds();
    }

    /// Recomputes the foldable regions, keeping which ones are collapsed.
    ///
    /// This is a whole-file scan, so it is not something a keystroke can
    /// afford; [`Editor::refresh_folds_if_needed`] decides when to run it.
    pub fn refresh_folds(&mut self) {
        let ranges = compute_folds(&self.buffer, self.language);
        self.folds.set_ranges(ranges);
        self.folds_line_count = self.buffer.len_lines();
        self.folds_stale = false;
        self.clamp_cursor_out_of_folds();
    }

    /// Recomputes the fold regions only when the answer can have changed.
    ///
    /// Typing inside a line cannot move a region's boundaries, so the common
    /// keystroke skips the scan entirely. A change to the line count can, and
    /// so can any edit while something is collapsed, because a stale range
    /// would hide the wrong lines.
    pub fn refresh_folds_if_needed(&mut self) {
        self.folds_stale = true;
        if self.buffer.len_lines() != self.folds_line_count || self.folds.any_collapsed() {
            self.refresh_folds();
        }
    }

    /// Brings the fold regions up to date before a command that reads them.
    pub fn ensure_folds(&mut self) {
        if self.folds_stale {
            self.refresh_folds();
        }
    }

    /// Returns the highlight spans for one buffer line.
    ///
    /// Safe to call from a renderer: it takes `&self` and repairs a stale tree
    /// behind a `RefCell`.
    #[must_use]
    pub fn highlight_line(&self, line: usize) -> Vec<HighlightSpan> {
        match self.syntax.try_borrow_mut() {
            Ok(mut syntax) => syntax.highlight_line(&self.buffer, line),
            // A reentrant render cannot happen in practice; drawing the line
            // without colour beats panicking inside a frame.
            Err(_) => Vec::new(),
        }
    }

    /// Returns true once the document has a parsed tree.
    #[must_use]
    pub fn has_syntax_tree(&self) -> bool {
        self.syntax.borrow().has_tree()
    }

    /// Reports an edit to the syntax tree so the next parse is incremental.
    pub(crate) fn note_syntax_edit(&mut self, edit: &InputEdit) {
        self.syntax.borrow_mut().note_edit(edit, &self.buffer);
    }

    /// Reports a change the tree cannot absorb, such as undo or redo.
    pub(crate) fn note_syntax_reset(&mut self) {
        self.syntax.borrow_mut().invalidate();
    }

    /// Returns the indentation style detected for this document.
    #[must_use]
    pub const fn indent_style(&self) -> super::indent::IndentStyle {
        self.indent_style
    }

    /// Overrides the detected indentation style.
    pub fn set_indent_style(&mut self, style: super::indent::IndentStyle) {
        self.indent_style = style;
    }

    /// Returns the fold state.
    ///
    /// The regions may be one edit out of date; the renderer only needs them to
    /// know what is hidden, and nothing can be hidden that was not collapsed.
    /// Use [`Editor::folds_mut`] when the exact set matters.
    #[must_use]
    pub const fn folds(&self) -> &FoldState {
        &self.folds
    }

    /// Returns the fold state for mutation, recomputing the regions first.
    pub fn folds_mut(&mut self) -> &mut FoldState {
        self.ensure_folds();
        &mut self.folds
    }

    /// Moves the cursor out of any region that is currently collapsed.
    ///
    /// The cursor lands on the line that starts the outermost collapsed region
    /// hiding it, which is the line still on screen.
    pub fn clamp_cursor_out_of_folds(&mut self) {
        let pos = self.cursor.position();
        if let Some(anchor) = self.folds.visible_anchor(pos.line)
            && anchor != pos.line
        {
            let col = pos.col.min(self.buffer.line_len_chars(anchor));
            self.cursor.set_position(Position::new(anchor, col));
            self.ensure_cursor_visible();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::editor::highlight::{HighlightKind, insertion_edit};

    fn rust_buffer(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn a_fresh_state_parses_on_first_use() {
        let buffer = rust_buffer("fn main() {}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::Rust, &buffer);
        assert!(syntax.has_tree());
        assert!(!syntax.highlight_line(&buffer, 0).is_empty());
    }

    #[test]
    fn an_unreported_edit_is_caught_by_the_revision_check() {
        let mut buffer = rust_buffer("fn main() {}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::Rust, &buffer);

        // Simulates a caller reaching past the editor into the buffer.
        buffer.insert_str(Position::new(0, 0), "// \n");
        let spans = syntax.highlight_line(&buffer, 0);
        assert!(
            spans.iter().any(|s| s.kind == HighlightKind::Comment),
            "a stale tree would still report line 0 as a function"
        );
    }

    #[test]
    fn a_reported_edit_keeps_the_spans_correct() {
        let mut buffer = rust_buffer("fn main() {\n    let a = 1;\n}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::Rust, &buffer);

        let at = Position::new(1, 4);
        let edit = insertion_edit(&buffer, at, "// ");
        buffer.insert_str(at, "// ");
        syntax.note_edit(&edit, &buffer);

        assert!(
            syntax
                .highlight_line(&buffer, 1)
                .iter()
                .any(|s| s.kind == HighlightKind::Comment)
        );
    }

    #[test]
    fn several_edits_cost_one_reparse() {
        let mut buffer = rust_buffer("fn main() {\n    let a = 1;\n}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::Rust, &buffer);
        assert!(!syntax.is_pending());

        for i in 0..5 {
            let at = Position::new(1, 4 + i);
            let edit = insertion_edit(&buffer, at, "z");
            buffer.insert_str(at, "z");
            syntax.note_edit(&edit, &buffer);
            assert!(syntax.is_pending(), "the parse is deferred");
        }

        // Asking for spans is what pays for it, once.
        let _ = syntax.highlight_line(&buffer, 1);
        assert!(!syntax.is_pending());
        assert!(buffer.text().contains("zzzzz"));
    }

    #[test]
    fn invalidate_forces_the_next_parse_to_start_over() {
        let buffer = rust_buffer("fn main() {}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::Rust, &buffer);
        syntax.invalidate();
        assert!(!syntax.has_tree());
        // Reading a line repairs it.
        assert!(!syntax.highlight_line(&buffer, 0).is_empty());
        assert!(syntax.has_tree());
    }

    #[test]
    fn plain_text_documents_have_no_spans() {
        let buffer = rust_buffer("fn main() {}\n");
        let mut syntax = Syntax::new();
        syntax.set_language(Language::PlainText, &buffer);
        assert!(!syntax.has_tree());
        assert!(syntax.highlight_line(&buffer, 0).is_empty());
        assert_eq!(syntax.language(), Language::PlainText);
    }
}
