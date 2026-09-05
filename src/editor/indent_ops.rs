//! Automatic indentation as the editor applies it.
//!
//! [`indent`](super::indent) computes what the indent should be;
//! this file decides when to apply it and does so through the editor's own
//! mutations, so the syntax tree stays in step.

use super::brackets::{BracketConfig, opening_for};
use super::indent::{
    IndentEdit, IndentStyle, indent_for_new_line, indent_lines, indent_of_line, outdent_lines,
    reindent_line,
};
use super::language::Language;
use super::{Editor, Position};

/// Returns the indent a line ending in a closing bracket should have.
///
/// The answer is the indentation of the line holding the matching opener, so
/// `}` lines up under the `if` or `fn` that opened the block. Returns `None`
/// when the line does not start with a closer, when the language does not use
/// braces, or when the opener cannot be found.
#[must_use]
pub fn indent_for_closing_bracket(
    buffer: &super::buffer::Buffer,
    line: usize,
    language: Language,
) -> Option<String> {
    if !language.uses_braces() {
        return None;
    }
    let text = buffer.line_text(line);
    let trimmed = text.trim_start();
    let closer = trimmed.chars().next()?;
    opening_for(closer)?;

    let col = text.chars().count() - trimmed.chars().count();
    let config = BracketConfig::for_language(language);
    let open = super::brackets::matching_bracket_with(buffer, Position::new(line, col), &config)?;
    if open.line == line {
        return None;
    }
    Some(indent_of_line(buffer, open.line))
}

impl Editor {
    /// Inserts a line break and the indentation the new line should start with.
    ///
    /// This is what `Enter` runs. The indent comes from the text around the
    /// cursor: one level deeper after an opening bracket or a Python `:`, one
    /// level shallower when a closing bracket is about to start the new line,
    /// and otherwise the same as the current line.
    pub fn insert_newline(&mut self) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let indent = indent_for_new_line(&self.buffer, pos, &self.indent_style, self.language);

        self.buffer.begin_undo_group();
        let text = format!("\n{indent}");
        let end = self.apply_insert(pos, &text);
        self.buffer.end_undo_group();
        self.cursor.set_position(end);
        self.after_edit();
    }

    /// Inserts a line break, indents the new line, and pushes a closing bracket
    /// that was directly after the cursor onto a line of its own.
    ///
    /// Typing `Enter` between `{` and `}` should leave the cursor on an indented
    /// blank line with the brace below it.
    pub fn insert_newline_smart(&mut self) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let after = self.buffer.line_chars(pos.line);
        let next = after.get(pos.col).copied();
        let splits = matches!(next, Some(')' | ']' | '}'))
            && pos.col > 0
            && matches!(after.get(pos.col - 1), Some('(' | '[' | '{'));

        if !splits {
            self.insert_newline();
            return;
        }

        let base = indent_of_line(&self.buffer, pos.line);
        let inner = format!("{base}{}", self.indent_style.unit());

        self.buffer.begin_undo_group();
        let text = format!("\n{inner}\n{base}");
        self.apply_insert(pos, &text);
        self.buffer.end_undo_group();
        self.cursor
            .set_position(Position::new(pos.line + 1, inner.chars().count()));
        self.after_edit();
    }

    /// Re-indents the cursor's line to line up with its opening bracket.
    ///
    /// Returns true when the line moved. Called after a closing bracket is
    /// typed, which is when an editor is expected to snap the line into place.
    pub fn reindent_current_line(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let line = self.cursor.position().line;
        let Some(target) = indent_for_closing_bracket(&self.buffer, line, self.language) else {
            return false;
        };
        let Some(edit) = reindent_line(&self.buffer, line, &target) else {
            return false;
        };
        self.apply_indent_edit(&edit);
        self.after_edit();
        true
    }

    /// Applies one indent edit, moving the cursor with the text.
    fn apply_indent_edit(&mut self, edit: &IndentEdit) {
        let cursor = self.cursor.position();
        self.buffer.begin_undo_group();
        if edit.old_len > 0 {
            self.apply_delete(
                Position::new(edit.line, 0),
                Position::new(edit.line, edit.old_len),
            );
        }
        let added = edit.new_indent.chars().count();
        if added > 0 {
            self.apply_insert(Position::new(edit.line, 0), &edit.new_indent);
        }
        self.buffer.end_undo_group();

        if cursor.line == edit.line {
            let shifted = cursor.col + added - edit.old_len.min(cursor.col);
            let col = shifted.min(self.buffer.line_len_chars(edit.line));
            self.cursor.set_position(Position::new(edit.line, col));
        }
    }

    /// Applies a batch of indent edits as one undo step.
    fn apply_indent_batch(&mut self, edits: &[IndentEdit]) -> bool {
        if edits.is_empty() {
            return false;
        }
        let mut ordered: Vec<&IndentEdit> = edits.iter().collect();
        ordered.sort_by_key(|e| std::cmp::Reverse(e.line));

        let cursor = self.cursor.position();
        let mut cursor_shift: isize = 0;
        self.buffer.begin_undo_group();
        for edit in ordered {
            if edit.line == cursor.line {
                cursor_shift = edit.new_indent.chars().count() as isize - edit.old_len as isize;
            }
            if edit.old_len > 0 {
                self.apply_delete(
                    Position::new(edit.line, 0),
                    Position::new(edit.line, edit.old_len),
                );
            }
            if !edit.new_indent.is_empty() {
                self.apply_insert(Position::new(edit.line, 0), &edit.new_indent);
            }
        }
        self.buffer.end_undo_group();

        let col = (cursor.col as isize + cursor_shift).max(0) as usize;
        self.cursor.set_position(Position::new(
            cursor.line,
            col.min(self.buffer.line_len_chars(cursor.line)),
        ));
        self.after_edit();
        true
    }

    /// Returns the inclusive line range an indent command applies to.
    fn indent_target_lines(&self) -> std::ops::RangeInclusive<usize> {
        self.cursor.selection_range().map_or_else(
            || {
                let line = self.cursor.position().line;
                line..=line
            },
            |(start, end)| {
                // A selection ending at column zero does not include that line.
                let last = if end.col == 0 && end.line > start.line {
                    end.line - 1
                } else {
                    end.line
                };
                start.line..=last
            },
        )
    }

    /// Adds one indentation level to the current line or selection.
    pub fn indent_selection(&mut self) -> bool {
        if self.reject_edit() {
            return false;
        }
        let edits = indent_lines(&self.buffer, self.indent_target_lines(), self.indent_style);
        self.apply_indent_batch(&edits)
    }

    /// Removes one indentation level from the current line or selection.
    pub fn outdent_selection(&mut self) -> bool {
        if self.reject_edit() {
            return false;
        }
        let edits = outdent_lines(&self.buffer, self.indent_target_lines(), self.indent_style);
        self.apply_indent_batch(&edits)
    }

    /// Inserts one indentation unit at the cursor.
    ///
    /// This is what `Tab` does when there is no selection: it types an indent
    /// in the document's own style rather than four literal spaces.
    pub fn insert_indent_unit(&mut self) {
        if self.reject_edit() {
            return;
        }
        let unit = self.indent_style.unit();
        self.insert_str(&unit);
    }

    /// Returns the style one indentation level is written in.
    #[must_use]
    pub fn indent_unit(&self) -> String {
        self.indent_style.unit()
    }

    /// Re-detects the indentation style from the current text.
    pub fn redetect_indent(&mut self) -> IndentStyle {
        self.indent_style = super::indent::detect_indent(&self.buffer);
        self.indent_style
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::buffer::Buffer;

    fn rust_editor(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_language(Language::Rust);
        editor
    }

    #[test]
    fn enter_after_an_opening_brace_indents_one_level() {
        let mut editor = rust_editor("fn f() {");
        editor.insert_newline();
        assert_eq!(editor.buffer().text(), "fn f() {\n    ");
        assert_eq!(editor.cursor_position(), Position::new(1, 4));
    }

    #[test]
    fn enter_copies_the_current_indent_by_default() {
        let mut editor = rust_editor("    let a = 1;");
        editor.insert_newline();
        assert_eq!(editor.buffer().text(), "    let a = 1;\n    ");
    }

    #[test]
    fn enter_before_a_closing_brace_outdents() {
        let mut editor = rust_editor("fn f() {\n    g();\n    }");
        editor.set_cursor_position(Position::new(2, 4));
        editor.insert_newline();
        assert_eq!(editor.buffer().text(), "fn f() {\n    g();\n    \n}");
    }

    #[test]
    fn enter_between_a_bracket_pair_opens_a_body() {
        let mut editor = rust_editor("fn f() {}");
        editor.set_cursor_position(Position::new(0, 8));
        editor.insert_newline_smart();
        assert_eq!(editor.buffer().text(), "fn f() {\n    \n}");
        assert_eq!(editor.cursor_position(), Position::new(1, 4));
    }

    #[test]
    fn smart_enter_falls_back_to_a_plain_indented_newline() {
        let mut editor = rust_editor("fn f() {");
        editor.insert_newline_smart();
        assert_eq!(editor.buffer().text(), "fn f() {\n    ");
    }

    #[test]
    fn a_closing_brace_snaps_onto_its_opener() {
        let mut editor = rust_editor("fn f() {\n    g();\n        }");
        editor.set_cursor_position(Position::new(2, 9));
        assert!(editor.reindent_current_line());
        assert_eq!(editor.buffer().text(), "fn f() {\n    g();\n}");
        // The cursor followed the text it was sitting after.
        assert_eq!(editor.cursor_position(), Position::new(2, 1));
    }

    #[test]
    fn re_indenting_a_line_that_is_already_right_reports_no_change() {
        let mut editor = rust_editor("fn f() {\n    g();\n}");
        editor.set_cursor_position(Position::new(2, 1));
        assert!(!editor.reindent_current_line());
        // Nor does a line that is not a closer.
        editor.set_cursor_position(Position::new(1, 4));
        assert!(!editor.reindent_current_line());
    }

    #[test]
    fn a_closing_bracket_on_its_openers_own_line_is_left_alone() {
        let buffer = Buffer::from_str("fn f() {}\n");
        assert_eq!(indent_for_closing_bracket(&buffer, 0, Language::Rust), None);
    }

    #[test]
    fn indentation_based_languages_do_not_snap_brackets() {
        let buffer = Buffer::from_str("def f():\n    pass\n");
        assert_eq!(
            indent_for_closing_bracket(&buffer, 1, Language::Python),
            None
        );
    }

    #[test]
    fn indent_and_outdent_move_the_selected_lines() {
        let mut editor = rust_editor("a\nb\nc\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(2, 1));

        assert!(editor.indent_selection());
        assert_eq!(editor.buffer().text(), "    a\n    b\n    c\n");
        assert!(editor.outdent_selection());
        assert_eq!(editor.buffer().text(), "a\nb\nc\n");
    }

    #[test]
    fn indent_with_no_selection_moves_only_the_cursor_line() {
        let mut editor = rust_editor("a\nb\n");
        editor.set_cursor_position(Position::new(1, 1));
        assert!(editor.indent_selection());
        assert_eq!(editor.buffer().text(), "a\n    b\n");
        assert_eq!(editor.cursor_position(), Position::new(1, 5));
    }

    #[test]
    fn outdenting_an_unindented_line_reports_no_change() {
        let mut editor = rust_editor("a\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert!(!editor.outdent_selection());
    }

    #[test]
    fn indenting_is_one_undo_step() {
        let mut editor = rust_editor("a\nb\nc\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(2, 1));
        editor.indent_selection();
        editor.undo();
        assert_eq!(editor.buffer().text(), "a\nb\nc\n");
    }

    #[test]
    fn tab_types_the_documents_own_indent_unit() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("fn f() {\n\tg();\n}\n");
        editor.redetect_indent();
        assert_eq!(editor.indent_unit(), "\t");
        editor.set_cursor_position(Position::new(2, 0));
        editor.insert_indent_unit();
        assert!(editor.buffer().text().contains("\n\t}"));
    }

    #[test]
    fn a_read_only_document_refuses_every_indent_command() {
        let mut editor = rust_editor("fn f() {\n        }");
        editor.set_read_only(true);
        editor.set_cursor_position(Position::new(1, 8));
        assert!(!editor.reindent_current_line());
        assert!(!editor.indent_selection());
        assert!(!editor.outdent_selection());
        editor.insert_newline();
        editor.insert_newline_smart();
        editor.insert_indent_unit();
        assert_eq!(editor.buffer().text(), "fn f() {\n        }");
    }
}
