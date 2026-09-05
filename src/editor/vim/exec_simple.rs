//! The Vim commands that need neither an operator nor a motion.
//!
//! Split out of `exec.rs` so neither file grows past the project's size limit.

use crate::editor::indent::indent_of_line;
use crate::editor::{Editor, EditorMode, Position};

use super::command::SimpleCommand;
use super::motion::last_line;
use super::registers::RegisterContent;

impl Editor {
    /// Applies a standalone command `count` times where that makes sense.
    pub(super) fn apply_simple(
        &mut self,
        simple: SimpleCommand,
        count: usize,
        register: Option<char>,
    ) {
        match simple {
            SimpleCommand::InsertBefore => self.set_mode(EditorMode::Insert),
            SimpleCommand::InsertAfter => {
                let pos = self.cursor.position();
                let col = (pos.col + 1).min(self.buffer.line_len_chars(pos.line));
                self.cursor.move_to(Position::new(pos.line, col));
                self.set_mode(EditorMode::Insert);
            }
            SimpleCommand::InsertLineStart => {
                let line = self.cursor.position().line;
                let landing = self
                    .buffer
                    .first_non_whitespace(line)
                    .unwrap_or(Position::new(line, 0));
                self.cursor.move_to(landing);
                self.set_mode(EditorMode::Insert);
            }
            SimpleCommand::AppendLineEnd => {
                let line = self.cursor.position().line;
                self.cursor
                    .move_to(Position::new(line, self.buffer.line_len_chars(line)));
                self.set_mode(EditorMode::Insert);
            }
            SimpleCommand::OpenBelow => self.open_line(false),
            SimpleCommand::OpenAbove => self.open_line(true),
            SimpleCommand::DeleteChar => self.delete_chars(count, register, true),
            SimpleCommand::DeleteCharBefore => self.delete_chars(count, register, false),
            SimpleCommand::DeleteToLineEnd | SimpleCommand::ChangeToLineEnd => {
                self.kill_to_line_end(register);
                if simple == SimpleCommand::ChangeToLineEnd {
                    self.set_mode(EditorMode::Insert);
                }
            }
            SimpleCommand::SubstituteChar => {
                self.delete_chars(count, register, true);
                self.set_mode(EditorMode::Insert);
            }
            SimpleCommand::SubstituteLine => self.substitute_line(register),
            SimpleCommand::ReplaceChar(c) => self.replace_chars(c, count),
            SimpleCommand::PasteAfter => self.paste(register, true, count),
            SimpleCommand::PasteBefore => self.paste(register, false, count),
            SimpleCommand::JoinLines => self.join_lines(count.max(2) - 1),
            SimpleCommand::Undo => self.undo(),
            SimpleCommand::Redo => self.redo(),
            SimpleCommand::VisualMode | SimpleCommand::VisualLineMode => {
                let linewise = simple == SimpleCommand::VisualLineMode;
                self.start_visual(linewise);
            }
            SimpleCommand::ToggleCaseChar => self.toggle_case_chars(count),
        }
    }

    /// `o` and `O`: opens an indented blank line and enters insert mode.
    fn open_line(&mut self, above: bool) {
        if self.reject_edit() {
            return;
        }
        let line = self.cursor.position().line;
        let indent = indent_of_line(&self.buffer, line);
        if above {
            let at = Position::new(line, 0);
            self.apply_insert(at, &format!("{indent}\n"));
            self.cursor
                .move_to(Position::new(line, indent.chars().count()));
        } else {
            let at = Position::new(line, self.buffer.line_len_chars(line));
            self.apply_insert(at, &format!("\n{indent}"));
            self.cursor
                .move_to(Position::new(line + 1, indent.chars().count()));
        }
        self.set_mode(EditorMode::Insert);
        self.after_edit();
    }

    /// `x` and `X`.
    fn delete_chars(&mut self, count: usize, register: Option<char>, forward: bool) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let len = self.buffer.line_len_chars(pos.line);
        let (start, end) = if forward {
            (pos, Position::new(pos.line, (pos.col + count).min(len)))
        } else {
            (Position::new(pos.line, pos.col.saturating_sub(count)), pos)
        };
        if start == end {
            return;
        }
        let removed = self.apply_delete(start, end);
        self.registers
            .set_delete(register, RegisterContent::charwise(removed));
        self.cursor.move_to(self.buffer.clamp_position(start));
        self.after_edit();
    }

    /// `D` and the delete half of `C`.
    fn kill_to_line_end(&mut self, register: Option<char>) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let end = Position::new(pos.line, self.buffer.line_len_chars(pos.line));
        if pos == end {
            return;
        }
        let removed = self.apply_delete(pos, end);
        self.registers
            .set_delete(register, RegisterContent::charwise(removed));
        self.after_edit();
    }

    /// `S`: clears the line's text but keeps its indentation.
    fn substitute_line(&mut self, register: Option<char>) {
        if self.reject_edit() {
            return;
        }
        let line = self.cursor.position().line;
        let indent = indent_of_line(&self.buffer, line);
        let start = Position::new(line, indent.chars().count());
        let end = Position::new(line, self.buffer.line_len_chars(line));
        let removed = self.buffer.get_range(start, end).unwrap_or_default();
        self.registers
            .set_delete(register, RegisterContent::charwise(removed));
        if start != end {
            self.apply_delete(start, end);
        }
        self.cursor.move_to(start);
        self.set_mode(EditorMode::Insert);
        self.after_edit();
    }

    /// `r{char}`: overwrites characters without leaving normal mode.
    fn replace_chars(&mut self, c: char, count: usize) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let len = self.buffer.line_len_chars(pos.line);
        if pos.col + count > len {
            return;
        }
        let end = Position::new(pos.line, pos.col + count);
        self.buffer.begin_undo_group();
        self.apply_delete(pos, end);
        self.apply_insert(pos, &c.to_string().repeat(count));
        self.buffer.end_undo_group();
        self.cursor
            .move_to(Position::new(pos.line, pos.col + count - 1));
        self.after_edit();
    }

    /// `~`: flips the case of the characters under the cursor.
    fn toggle_case_chars(&mut self, count: usize) {
        if self.reject_edit() {
            return;
        }
        let pos = self.cursor.position();
        let len = self.buffer.line_len_chars(pos.line);
        let end = Position::new(pos.line, (pos.col + count).min(len));
        if pos == end {
            return;
        }
        let text = self.buffer.get_range(pos, end).unwrap_or_default();
        let flipped: String = text
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else {
                    c.to_uppercase().next().unwrap_or(c)
                }
            })
            .collect();
        self.buffer.begin_undo_group();
        self.apply_delete(pos, end);
        self.apply_insert(pos, &flipped);
        self.buffer.end_undo_group();
        self.cursor.move_to(end);
        self.after_edit();
    }

    /// `p` and `P`.
    fn paste(&mut self, register: Option<char>, after: bool, count: usize) {
        if self.reject_edit() {
            return;
        }
        let Some(content) = self.registers.get(register).cloned() else {
            return;
        };
        if content.text.is_empty() {
            return;
        }
        let pos = self.cursor.position();

        self.buffer.begin_undo_group();
        let landing = if content.linewise {
            let line = if after { pos.line + 1 } else { pos.line };
            let at = if line >= self.buffer.len_lines() {
                let last = self.buffer.len_lines().saturating_sub(1);
                let end = Position::new(last, self.buffer.line_len_chars(last));
                self.apply_insert(end, "\n");
                Position::new(line, 0)
            } else {
                Position::new(line, 0)
            };
            self.apply_insert(at, &content.text.repeat(count));
            Position::new(line, 0)
        } else {
            let at = if after {
                Position::new(
                    pos.line,
                    (pos.col + 1).min(self.buffer.line_len_chars(pos.line)),
                )
            } else {
                pos
            };
            let end = self.apply_insert(at, &content.text.repeat(count));
            Position::new(end.line, end.col.saturating_sub(1))
        };
        self.buffer.end_undo_group();
        self.cursor.move_to(self.buffer.clamp_position(landing));
        self.after_edit();
    }

    /// `J`: joins `count` following lines onto the cursor's, with one space.
    fn join_lines(&mut self, count: usize) {
        if self.reject_edit() {
            return;
        }
        self.buffer.begin_undo_group();
        let mut landing = self.cursor.position();
        for _ in 0..count.max(1) {
            let line = self.cursor.position().line;
            if line >= last_line(&self.buffer) {
                break;
            }
            let end_of_line = self.buffer.line_len_chars(line);
            let next_indent = indent_of_line(&self.buffer, line + 1).chars().count();
            let start = Position::new(line, end_of_line);
            let end = Position::new(line + 1, next_indent);
            self.apply_delete(start, end);
            let separator = if end_of_line == 0 { "" } else { " " };
            self.apply_insert(start, separator);
            landing = Position::new(line, end_of_line);
        }
        self.buffer.end_undo_group();
        self.cursor.move_to(self.buffer.clamp_position(landing));
        self.after_edit();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::vim::keys::keys;

    fn editor_with(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_cursor_position(Position::new(0, 0));
        editor.set_mode(EditorMode::Normal);
        editor
    }

    fn feed(editor: &mut Editor, sequence: &str) {
        for key in keys(sequence) {
            editor.feed_vim_key(key);
        }
    }

    #[test]
    fn o_opens_an_indented_line_below_and_enters_insert() {
        let mut editor = editor_with("    a\nb\n");
        feed(&mut editor, "o");
        assert_eq!(editor.mode(), EditorMode::Insert);
        assert_eq!(editor.buffer().text(), "    a\n    \nb\n");
        assert_eq!(editor.cursor_position(), Position::new(1, 4));
    }

    #[test]
    fn shift_o_opens_a_line_above() {
        let mut editor = editor_with("  a\n");
        feed(&mut editor, "O");
        assert_eq!(editor.buffer().text(), "  \n  a\n");
        assert_eq!(editor.cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn x_deletes_forward_with_a_count() {
        let mut editor = editor_with("abcdef\n");
        feed(&mut editor, "3x");
        assert_eq!(editor.buffer().text(), "def\n");
        assert_eq!(
            editor.registers().get(None).map(|c| c.text.clone()),
            Some("abc".to_string())
        );
    }

    #[test]
    fn shift_x_deletes_backward() {
        let mut editor = editor_with("abcdef\n");
        editor.set_cursor_position(Position::new(0, 3));
        feed(&mut editor, "X");
        assert_eq!(editor.buffer().text(), "abdef\n");
    }

    #[test]
    fn deleting_past_the_end_of_a_line_stops_there() {
        let mut editor = editor_with("ab\ncd\n");
        feed(&mut editor, "9x");
        assert_eq!(editor.buffer().text(), "\ncd\n");
    }

    #[test]
    fn shift_d_and_shift_c_clear_the_rest_of_the_line() {
        let mut editor = editor_with("hello world\n");
        editor.set_cursor_position(Position::new(0, 5));
        feed(&mut editor, "D");
        assert_eq!(editor.buffer().text(), "hello\n");
        assert_eq!(editor.mode(), EditorMode::Normal);

        let mut editor = editor_with("hello world\n");
        editor.set_cursor_position(Position::new(0, 5));
        feed(&mut editor, "C");
        assert_eq!(editor.buffer().text(), "hello\n");
        assert_eq!(editor.mode(), EditorMode::Insert);
    }

    #[test]
    fn shift_s_clears_the_line_but_keeps_the_indent() {
        let mut editor = editor_with("    let a = 1;\n");
        editor.set_cursor_position(Position::new(0, 8));
        feed(&mut editor, "S");
        assert_eq!(editor.buffer().text(), "    \n");
        assert_eq!(editor.cursor_position(), Position::new(0, 4));
        assert_eq!(editor.mode(), EditorMode::Insert);
    }

    #[test]
    fn r_replaces_characters_in_place() {
        let mut editor = editor_with("abcdef\n");
        feed(&mut editor, "3rz");
        assert_eq!(editor.buffer().text(), "zzzdef\n");
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn r_past_the_end_of_the_line_does_nothing() {
        let mut editor = editor_with("ab\n");
        feed(&mut editor, "9rz");
        assert_eq!(editor.buffer().text(), "ab\n");
    }

    #[test]
    fn tilde_flips_case_and_moves_on() {
        let mut editor = editor_with("aBc\n");
        feed(&mut editor, "3~");
        assert_eq!(editor.buffer().text(), "AbC\n");
        assert_eq!(editor.cursor_position(), Position::new(0, 3));
    }

    #[test]
    fn yy_then_p_duplicates_a_line_below() {
        let mut editor = editor_with("one\ntwo\n");
        feed(&mut editor, "yyp");
        assert_eq!(editor.buffer().text(), "one\none\ntwo\n");
        assert_eq!(editor.cursor_position().line, 1);
    }

    #[test]
    fn shift_p_puts_a_yanked_line_above() {
        let mut editor = editor_with("one\ntwo\n");
        editor.set_cursor_position(Position::new(1, 0));
        feed(&mut editor, "yyP");
        assert_eq!(editor.buffer().text(), "one\ntwo\ntwo\n");
    }

    #[test]
    fn a_charwise_put_lands_after_the_cursor() {
        let mut editor = editor_with("abcd\n");
        feed(&mut editor, "ylp");
        assert_eq!(editor.buffer().text(), "aabcd\n");
    }

    #[test]
    fn pasting_from_an_empty_register_does_nothing() {
        let mut editor = editor_with("abc\n");
        feed(&mut editor, "p");
        assert_eq!(editor.buffer().text(), "abc\n");
    }

    #[test]
    fn j_joins_lines_with_one_space() {
        let mut editor = editor_with("one\n    two\nthree\n");
        feed(&mut editor, "J");
        assert_eq!(editor.buffer().text(), "one two\nthree\n");
        feed(&mut editor, "J");
        assert_eq!(editor.buffer().text(), "one two three\n");
    }

    #[test]
    fn joining_at_the_last_line_does_nothing() {
        let mut editor = editor_with("only\n");
        feed(&mut editor, "J");
        assert_eq!(editor.buffer().text(), "only\n");
    }

    #[test]
    fn i_a_shift_i_and_shift_a_all_land_where_vim_puts_them() {
        let mut editor = editor_with("    hello\n");
        editor.set_cursor_position(Position::new(0, 6));
        feed(&mut editor, "I");
        assert_eq!(editor.cursor_position(), Position::new(0, 4));

        editor.set_mode(EditorMode::Normal);
        feed(&mut editor, "A");
        assert_eq!(editor.cursor_position(), Position::new(0, 9));

        editor.set_mode(EditorMode::Normal);
        editor.set_cursor_position(Position::new(0, 4));
        feed(&mut editor, "a");
        assert_eq!(editor.cursor_position(), Position::new(0, 5));
    }

    #[test]
    fn undo_and_redo_run_through_the_state_machine() {
        let mut editor = editor_with("abc\n");
        feed(&mut editor, "x");
        assert_eq!(editor.buffer().text(), "bc\n");
        feed(&mut editor, "u");
        assert_eq!(editor.buffer().text(), "abc\n");
        editor.feed_vim_key(crate::editor::vim::VimKey::Ctrl('r'));
        assert_eq!(editor.buffer().text(), "bc\n");
    }

    #[test]
    fn a_read_only_document_refuses_every_mutating_command() {
        let mut editor = editor_with("abc\n");
        editor.set_read_only(true);
        for sequence in ["x", "X", "D", "S", "rz", "~", "o", "O", "J", "p"] {
            feed(&mut editor, sequence);
            editor.set_mode(EditorMode::Normal);
        }
        assert_eq!(editor.buffer().text(), "abc\n");
    }
}
