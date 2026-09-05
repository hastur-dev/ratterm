//! The editor's primitive mutations.
//!
//! Everything that changes the buffer goes through [`Editor::apply_insert`] or
//! [`Editor::apply_delete`], which report the change to the syntax tree before
//! the text moves. That is what makes highlighting incremental: tree-sitter
//! gets a description of the edit and reuses the rest of the tree.

use super::highlight::{deletion_edit, insertion_edit};
use super::{Editor, Position};

impl Editor {
    /// Inserts `text` at `at`, keeping the syntax tree in step.
    ///
    /// Returns the position just past the inserted text.
    pub(crate) fn apply_insert(&mut self, at: Position, text: &str) -> Position {
        if text.is_empty() {
            return at;
        }
        let at = self.buffer.clamp_position(at);
        let edit = insertion_edit(&self.buffer, at, text);
        self.buffer.insert_str(at, text);
        self.note_syntax_edit(&edit);
        let end = self.buffer.position_to_index(at) + text.chars().count();
        self.buffer.index_to_position(end)
    }

    /// Deletes `start..end`, keeping the syntax tree in step.
    ///
    /// Returns the text that was removed, which the kill ring and the Vim
    /// registers both need.
    pub(crate) fn apply_delete(&mut self, start: Position, end: Position) -> String {
        let start = self.buffer.clamp_position(start);
        let end = self.buffer.clamp_position(end);
        let (start, end) =
            if self.buffer.position_to_index(start) <= self.buffer.position_to_index(end) {
                (start, end)
            } else {
                (end, start)
            };
        let removed = self.buffer.get_range(start, end).unwrap_or_default();
        if removed.is_empty() {
            return removed;
        }
        let edit = deletion_edit(&self.buffer, start, end);
        self.buffer.delete_range(start, end);
        self.note_syntax_edit(&edit);
        removed
    }

    /// Finishes a mutation: gutter width, folds, cursor visibility.
    pub(crate) fn after_edit(&mut self) {
        self.view.update_gutter_width(self.buffer.len_lines());
        self.refresh_folds_if_needed();
        self.search.mark_stale();
        self.ensure_cursor_visible();
    }

    /// Inserts a character at the cursor.
    ///
    /// This is the primitive: it does not auto-pair and does not auto-indent.
    /// [`Editor::type_char`] is what a keystroke should call.
    pub fn insert_char(&mut self, c: char) {
        if self.reject_edit() {
            return;
        }
        if self.cursors.is_empty() {
            let pos = self.cursor.position();
            let end = self.apply_insert(pos, &c.to_string());
            self.cursor.set_position(end);
        } else {
            self.insert_at_every_cursor(&c.to_string());
        }
        self.after_edit();
    }

    /// Inserts a string at the cursor.
    pub fn insert_str(&mut self, s: &str) {
        if self.reject_edit() {
            return;
        }
        if self.cursors.is_empty() {
            let pos = self.cursor.position();
            let end = self.apply_insert(pos, s);
            self.cursor.set_position(end);
        } else {
            self.insert_at_every_cursor(s);
        }
        self.after_edit();
    }

    /// Deletes the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.reject_edit() {
            return;
        }
        if !self.cursors.is_empty() {
            self.backspace_at_every_cursor();
            self.after_edit();
            return;
        }
        let pos = self.cursor.position();

        if pos.col > 0 {
            let start = Position::new(pos.line, pos.col - 1);
            self.apply_delete(start, pos);
            self.cursor.set_position(start);
        } else if pos.line > 0 {
            let prev_line_len = self.buffer.line_len_chars(pos.line - 1);
            let start = Position::new(pos.line - 1, prev_line_len);
            self.apply_delete(start, pos);
            self.cursor.set_position(start);
        }

        self.after_edit();
    }

    /// Deletes the character at the cursor (delete).
    pub fn delete(&mut self) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let end = if pos.col < self.buffer.line_len_chars(pos.line) {
            Position::new(pos.line, pos.col + 1)
        } else if pos.line + 1 < self.buffer.len_lines() {
            Position::new(pos.line + 1, 0)
        } else {
            return;
        };
        self.apply_delete(pos, end);
        self.after_edit();
    }

    /// Deletes the selected text.
    pub fn delete_selection(&mut self) {
        if self.reject_edit() {
            return;
        }
        if let Some((start, end)) = self.cursor.selection_range() {
            self.apply_delete(start, end);
            self.cursor.move_to(start);
            self.after_edit();
        }
    }

    /// Deletes from the cursor to the end of the line (Emacs Ctrl+K).
    ///
    /// On an empty tail this swallows the line break instead, which is what
    /// makes repeated `C-k` delete a paragraph.
    pub fn delete_to_line_end(&mut self) -> String {
        if self.reject_edit() {
            return String::new();
        }
        let pos = self.cursor.position();
        let line_len = self.buffer.line_len_chars(pos.line);

        let removed = if pos.col < line_len {
            self.apply_delete(pos, Position::new(pos.line, line_len))
        } else if pos.line + 1 < self.buffer.len_lines() {
            self.apply_delete(pos, Position::new(pos.line + 1, 0))
        } else {
            String::new()
        };

        self.after_edit();
        removed
    }

    /// Replaces `start..end` with `text` as one undo step.
    pub fn replace_range(&mut self, start: Position, end: Position, text: &str) -> bool {
        if self.reject_edit() {
            return false;
        }
        self.buffer.begin_undo_group();
        self.apply_delete(start, end);
        let cursor = self.apply_insert(start, text);
        self.buffer.end_undo_group();
        self.cursor.move_to(cursor);
        self.after_edit();
        true
    }

    /// Undoes the last edit.
    pub fn undo(&mut self) {
        if self.reject_edit() {
            return;
        }
        self.buffer.undo();
        // An undo moves text the tree cannot be told about incrementally.
        self.note_syntax_reset();
        self.cursor.clamp(&self.buffer);
        self.cursors.clear();
        self.after_edit();
    }

    /// Redoes the last undone edit.
    pub fn redo(&mut self) {
        if self.reject_edit() {
            return;
        }
        self.buffer.redo();
        self.note_syntax_reset();
        self.cursor.clamp(&self.buffer);
        self.cursors.clear();
        self.after_edit();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::highlight::{HighlightKind, Language};

    fn rust_editor(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    #[test]
    fn test_editor_insert() {
        let mut editor = Editor::new(80, 24);
        editor.insert_char('H');
        editor.insert_char('i');
        assert_eq!(editor.buffer().text(), "Hi");
        assert_eq!(editor.cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn inserting_a_newline_moves_to_the_next_line() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab");
        editor.insert_char('\n');
        assert_eq!(editor.cursor_position(), Position::new(1, 0));
        assert_eq!(editor.buffer().text(), "ab\n");
    }

    #[test]
    fn test_editor_backspace() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("Hello");
        editor.backspace();
        assert_eq!(editor.buffer().text(), "Hell");
    }

    #[test]
    fn backspace_at_the_start_of_a_line_joins_it_to_the_previous_one() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab\ncd");
        editor.set_cursor_position(Position::new(1, 0));
        editor.backspace();
        assert_eq!(editor.buffer().text(), "abcd");
        assert_eq!(editor.cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn backspace_at_the_very_start_does_nothing() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab");
        editor.set_cursor_position(Position::new(0, 0));
        editor.backspace();
        assert_eq!(editor.buffer().text(), "ab");
    }

    #[test]
    fn delete_joins_the_next_line_at_the_end_of_one() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab\ncd");
        editor.set_cursor_position(Position::new(0, 2));
        editor.delete();
        assert_eq!(editor.buffer().text(), "abcd");
    }

    #[test]
    fn delete_at_the_very_end_does_nothing() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab");
        editor.delete();
        assert_eq!(editor.buffer().text(), "ab");
    }

    #[test]
    fn test_editor_undo() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("Hello");
        editor.undo();
        assert_eq!(editor.buffer().text(), "");
    }

    #[test]
    fn delete_to_line_end_takes_the_break_when_the_tail_is_empty() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("ab\ncd\n");
        editor.set_cursor_position(Position::new(0, 2));
        assert_eq!(editor.delete_to_line_end(), "\n");
        assert_eq!(editor.buffer().text(), "abcd\n");
        editor.set_cursor_position(Position::new(0, 2));
        assert_eq!(editor.delete_to_line_end(), "cd");
        assert_eq!(editor.buffer().text(), "ab\n");
    }

    #[test]
    fn replace_range_is_one_undo_step() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("one two");
        assert!(editor.replace_range(Position::new(0, 0), Position::new(0, 3), "seven"));
        assert_eq!(editor.buffer().text(), "seven two");
        editor.undo();
        assert_eq!(editor.buffer().text(), "one two");
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
        editor.replace_range(Position::new(0, 0), Position::new(0, 1), "z");

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
    fn typing_keeps_the_syntax_tree_correct() {
        let mut editor = rust_editor("fn main() {\n    let a = 1;\n}\n");
        editor.set_cursor_position(Position::new(1, 4));
        for c in "// ".chars() {
            editor.insert_char(c);
        }
        let spans = editor.highlight_line(1);
        assert!(
            spans.iter().any(|s| s.kind == HighlightKind::Comment),
            "the commented-out line must highlight as a comment"
        );
    }

    #[test]
    fn undo_restores_the_previous_highlighting() {
        let mut editor = rust_editor("fn main() {\n    let a = 1;\n}\n");
        editor.set_cursor_position(Position::new(1, 4));
        editor.insert_str("// ");
        assert!(
            editor
                .highlight_line(1)
                .iter()
                .any(|s| s.kind == HighlightKind::Comment)
        );
        editor.undo();
        assert!(
            !editor
                .highlight_line(1)
                .iter()
                .any(|s| s.kind == HighlightKind::Comment),
            "undoing the comment marker must un-grey the line"
        );
    }

    #[test]
    fn an_edit_made_straight_on_the_buffer_is_still_highlighted_correctly() {
        let mut editor = rust_editor("fn main() {\n    let a = 1;\n}\n");
        editor.buffer_mut().insert_str(Position::new(1, 4), "// ");
        assert!(
            editor
                .highlight_line(1)
                .iter()
                .any(|s| s.kind == HighlightKind::Comment),
            "the revision check must catch a change that bypassed the editor"
        );
    }
}
