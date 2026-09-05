//! What one printable keystroke does.
//!
//! [`Editor::type_char`] is the entry point every non-modal key handler uses.
//! It applies the auto-pairing policy from [`brackets`](super::brackets) and
//! snaps a closing bracket onto its opener, so the input layer stays a
//! translation of key events and nothing else.

use super::brackets::{TypeAction, opening_for, type_action};
use super::{Editor, Position};

impl Editor {
    /// Types one character at the cursor, honouring auto-pairing.
    ///
    /// With secondary cursors active the character goes in at every one of
    /// them; auto-pairing is skipped in that case, because the answer can
    /// differ per cursor and a fan-out has to be one uniform edit.
    pub fn type_char(&mut self, c: char) {
        if self.reject_edit() {
            return;
        }
        if !self.cursors.is_empty() {
            self.insert_char(c);
            return;
        }
        if self.cursor.has_selection() && self.mode() != super::EditorMode::Visual {
            self.delete_selection();
        }

        let pos = self.cursor.position();
        match type_action(&self.buffer, pos, c, self.auto_pair) {
            TypeAction::StepOver => {
                self.cursor
                    .move_to(Position::new(pos.line, pos.col.saturating_add(1)));
                self.ensure_cursor_visible();
            }
            TypeAction::InsertPair(open, close) => {
                self.buffer.begin_undo_group();
                self.apply_insert(pos, &format!("{open}{close}"));
                self.buffer.end_undo_group();
                self.cursor
                    .set_position(Position::new(pos.line, pos.col + 1));
                self.after_edit();
            }
            TypeAction::Insert(ch) => {
                self.insert_char(ch);
                if opening_for(ch).is_some() {
                    self.reindent_current_line();
                }
            }
        }
    }

    /// Types a string one character at a time, so auto-pairing and
    /// auto-indentation apply exactly as they would to real keystrokes.
    pub fn type_str(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                self.insert_newline_smart();
            } else {
                self.type_char(c);
            }
        }
    }

    /// Returns the bracket pair to highlight for the current cursor.
    ///
    /// The bracket under the cursor wins; a cursor just past a bracket still
    /// highlights the pair, which is what insert mode needs.
    #[must_use]
    pub fn matching_bracket_pair(&self) -> Option<(Position, Position)> {
        let config = super::brackets::BracketConfig::for_language(self.language);
        super::brackets::highlight_pair(&self.buffer, self.cursor.position(), &config)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::brackets::AutoPair;
    use crate::editor::highlight::Language;

    fn rust_editor(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_language(Language::Rust);
        editor
    }

    #[test]
    fn an_opening_bracket_brings_its_partner_with_it() {
        let mut editor = rust_editor("let a = ");
        editor.type_char('(');
        assert_eq!(editor.buffer().text(), "let a = ()");
        assert_eq!(editor.cursor_position(), Position::new(0, 9));
    }

    #[test]
    fn typing_the_closing_bracket_steps_over_the_auto_inserted_one() {
        let mut editor = rust_editor("let a = ");
        editor.type_char('(');
        editor.type_char(')');
        assert_eq!(editor.buffer().text(), "let a = ()");
        assert_eq!(editor.cursor_position(), Position::new(0, 10));
    }

    #[test]
    fn a_quote_pairs_and_then_closes() {
        let mut editor = rust_editor("let s = ");
        editor.type_char('"');
        assert_eq!(editor.buffer().text(), "let s = \"\"");
        editor.type_char('x');
        editor.type_char('"');
        assert_eq!(editor.buffer().text(), "let s = \"x\"");
        assert_eq!(editor.cursor_position(), Position::new(0, 11));
    }

    #[test]
    fn a_bracket_typed_before_a_word_is_not_paired() {
        let mut editor = rust_editor("abc");
        editor.set_cursor_position(Position::new(0, 0));
        editor.type_char('(');
        assert_eq!(editor.buffer().text(), "(abc");
    }

    #[test]
    fn auto_pairing_can_be_switched_off() {
        let mut editor = rust_editor("");
        editor.set_auto_pair(AutoPair {
            enabled: false,
            ..AutoPair::default()
        });
        editor.type_char('(');
        assert_eq!(editor.buffer().text(), "(");
    }

    #[test]
    fn a_closing_brace_snaps_the_line_onto_its_opener() {
        let mut editor = rust_editor("fn f() {\n    g();\n        ");
        editor.set_cursor_position(Position::new(2, 8));
        editor.type_char('}');
        assert_eq!(editor.buffer().text(), "fn f() {\n    g();\n}");
    }

    #[test]
    fn typing_over_a_selection_replaces_it() {
        let mut editor = rust_editor("hello");
        editor.set_cursor_position(Position::new(0, 0));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(0, 5));
        editor.type_char('x');
        assert_eq!(editor.buffer().text(), "x");
    }

    #[test]
    fn typing_at_several_cursors_skips_auto_pairing() {
        let mut editor = rust_editor("a\nb\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.add_cursor_below();
        editor.type_char('(');
        assert_eq!(editor.buffer().text(), "(a\n(b\n");
    }

    #[test]
    fn type_str_runs_the_same_rules_and_handles_newlines() {
        let mut editor = rust_editor("");
        // The `(` and `{` each bring a partner, and the newline between the
        // braces opens an indented body.
        editor.type_str("fn f() {\n");
        assert_eq!(editor.buffer().text(), "fn f() {\n    \n}");
        assert_eq!(editor.cursor_position(), Position::new(1, 4));
    }

    #[test]
    fn the_bracket_under_the_cursor_reports_its_partner() {
        let mut editor = rust_editor("fn f() {\n    g();\n}\n");
        editor.set_cursor_position(Position::new(0, 7));
        assert_eq!(
            editor.matching_bracket_pair(),
            Some((Position::new(0, 7), Position::new(2, 0)))
        );
        editor.set_cursor_position(Position::new(2, 0));
        assert_eq!(
            editor.matching_bracket_pair(),
            Some((Position::new(0, 7), Position::new(2, 0)))
        );
    }

    #[test]
    fn a_cursor_away_from_any_bracket_reports_no_pair() {
        let mut editor = rust_editor("let a = 1;\n");
        editor.set_cursor_position(Position::new(0, 4));
        assert_eq!(editor.matching_bracket_pair(), None);
    }

    #[test]
    fn a_read_only_document_refuses_typed_characters() {
        let mut editor = rust_editor("abc");
        editor.set_read_only(true);
        editor.type_char('(');
        editor.type_str("xyz");
        assert_eq!(editor.buffer().text(), "abc");
    }
}
