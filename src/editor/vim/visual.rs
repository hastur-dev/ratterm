//! Visual mode.
//!
//! Visual mode reuses the same parser as normal mode: a motion extends the
//! selection instead of moving the cursor, and an operator applies to the
//! selection instead of waiting for a target. Keeping that in one place means
//! `d`, `y`, `>` and `gU` behave identically in both modes.

use crate::editor::indent::{apply_indent_edits, indent_lines, outdent_lines};
use crate::editor::{Editor, EditorMode, Position};

use super::command::{Operator, SimpleCommand, VimAction};
use super::exec::{VimEffect, VimFeed};
use super::keys::VimKey;
use super::motion::resolve_motion;
use super::registers::RegisterContent;
use super::state::VimOutcome;

impl Editor {
    /// Enters visual mode, anchoring the selection at the cursor.
    pub fn start_visual(&mut self, linewise: bool) {
        let pos = self.cursor.position();
        if linewise {
            self.cursor.move_to(Position::new(pos.line, 0));
            self.cursor.start_selection();
            let end = self.buffer.line_len_chars(pos.line);
            self.cursor.extend_to(Position::new(pos.line, end));
        } else {
            self.cursor.start_selection();
        }
        self.set_mode(EditorMode::Visual);
    }

    /// Leaves visual mode, dropping the selection.
    pub fn end_visual(&mut self) {
        self.cursor.clear_selection();
        self.set_mode(EditorMode::Normal);
    }

    /// Returns the selection as an inclusive-of-start, exclusive-of-end range.
    ///
    /// Visual mode in Vim includes the character under the cursor, so the end
    /// is pushed one character past it.
    #[must_use]
    pub fn visual_range(&self) -> Option<(Position, Position)> {
        let (start, end) = self.cursor.selection_range()?;
        let end_index = self.buffer.position_to_index(end);
        let inclusive = (end_index + 1).min(self.buffer.len_chars());
        Some((start, self.buffer.index_to_position(inclusive)))
    }

    /// Feeds one key while in visual mode.
    pub(super) fn feed_vim_visual_key(&mut self, key: VimKey) -> VimFeed {
        if key == VimKey::Escape {
            self.vim.reset();
            self.end_visual();
            return VimFeed {
                pending: false,
                executed: true,
                effect: VimEffect::None,
            };
        }

        let mut vim = std::mem::take(&mut self.vim);
        let outcome = vim.feed(key);
        self.vim = vim;

        match outcome {
            VimOutcome::Pending => {
                // An operator has all it needs here: the selection is its
                // target, so it runs rather than waiting for a motion.
                match self.vim.take_visual_operator() {
                    Some((operator, register, _)) => {
                        self.apply_visual_operator(operator, register);
                        VimFeed {
                            pending: false,
                            executed: true,
                            effect: VimEffect::None,
                        }
                    }
                    None => VimFeed::pending(),
                }
            }
            VimOutcome::Rejected => VimFeed::rejected(),
            VimOutcome::Command(command) => {
                let effect = self.apply_visual(&command.action, command.count, command.register);
                VimFeed {
                    pending: false,
                    executed: true,
                    effect,
                }
            }
        }
    }

    /// Applies one resolved action to the selection.
    fn apply_visual(
        &mut self,
        action: &VimAction,
        count: usize,
        register: Option<char>,
    ) -> VimEffect {
        match action {
            VimAction::Motion(motion) => {
                if let Some((target, _)) =
                    resolve_motion(&self.buffer, self.cursor.position(), motion, count)
                {
                    self.cursor.extend_to(self.buffer.clamp_position(target));
                    self.ensure_cursor_visible();
                }
                VimEffect::None
            }
            VimAction::Operate { operator, .. } => {
                self.apply_visual_operator(*operator, register);
                VimEffect::None
            }
            VimAction::Simple(simple) => {
                self.apply_visual_simple(*simple, register);
                VimEffect::None
            }
            VimAction::SetMark(name) => {
                let pos = self.cursor.position();
                self.vim.set_mark(*name, pos);
                VimEffect::None
            }
            VimAction::GotoMark { mark, .. } => {
                if let Some(pos) = self.vim.mark(*mark) {
                    self.cursor.extend_to(self.buffer.clamp_position(pos));
                    self.ensure_cursor_visible();
                }
                VimEffect::None
            }
            VimAction::Substitute(sub) => {
                let count = self.apply_substitute(sub);
                self.end_visual();
                self.set_status(format!("{count} substitutions"));
                VimEffect::None
            }
            VimAction::Ex(line) => {
                self.end_visual();
                super::exec::ex_effect(line)
            }
        }
    }

    /// Applies an operator to the selection and returns to normal mode.
    fn apply_visual_operator(&mut self, operator: Operator, register: Option<char>) {
        let Some((start, end)) = self.visual_range() else {
            return;
        };
        if operator != Operator::Yank && self.reject_edit() {
            return;
        }
        let text = self.buffer.get_range(start, end).unwrap_or_default();

        match operator {
            Operator::Yank => {
                self.registers
                    .set_yank(register, RegisterContent::charwise(text));
                self.cursor.move_to(start);
                self.end_visual();
            }
            Operator::Delete | Operator::Change => {
                self.registers
                    .set_delete(register, RegisterContent::charwise(text));
                self.apply_delete(start, end);
                self.cursor.move_to(self.buffer.clamp_position(start));
                self.after_edit();
                if operator == Operator::Change {
                    self.cursor.clear_selection();
                    self.set_mode(EditorMode::Insert);
                } else {
                    self.end_visual();
                }
            }
            Operator::Indent | Operator::Outdent => {
                // The range ends one past its last character, so a selection
                // stopping at column zero does not include that line.
                let last = if end.col == 0 && end.line > start.line {
                    end.line - 1
                } else {
                    end.line
                };
                let lines = start.line..=last.min(self.buffer.len_lines().saturating_sub(1));
                let edits = if operator == Operator::Indent {
                    indent_lines(&self.buffer, lines, self.indent_style)
                } else {
                    outdent_lines(&self.buffer, lines, self.indent_style)
                };
                apply_indent_edits(&mut self.buffer, &edits);
                self.note_syntax_reset();
                self.cursor.clamp(&self.buffer);
                self.after_edit();
                self.end_visual();
            }
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                let replaced: String = text
                    .chars()
                    .map(|c| match operator {
                        Operator::Lowercase => c.to_lowercase().next().unwrap_or(c),
                        Operator::Uppercase => c.to_uppercase().next().unwrap_or(c),
                        _ if c.is_uppercase() => c.to_lowercase().next().unwrap_or(c),
                        _ => c.to_uppercase().next().unwrap_or(c),
                    })
                    .collect();
                self.buffer.begin_undo_group();
                self.apply_delete(start, end);
                self.apply_insert(start, &replaced);
                self.buffer.end_undo_group();
                self.cursor.move_to(self.buffer.clamp_position(start));
                self.after_edit();
                self.end_visual();
            }
        }
    }

    /// Handles the standalone commands that mean something in visual mode.
    fn apply_visual_simple(&mut self, simple: SimpleCommand, register: Option<char>) {
        match simple {
            SimpleCommand::DeleteChar | SimpleCommand::SubstituteChar => {
                self.apply_visual_operator(Operator::Delete, register);
                if simple == SimpleCommand::SubstituteChar {
                    self.set_mode(EditorMode::Insert);
                }
            }
            SimpleCommand::DeleteToLineEnd
            | SimpleCommand::ChangeToLineEnd
            | SimpleCommand::SubstituteLine => {
                self.select_whole_lines();
                self.apply_visual_operator(Operator::Delete, register);
                if simple != SimpleCommand::DeleteToLineEnd {
                    self.set_mode(EditorMode::Insert);
                }
            }
            SimpleCommand::ToggleCaseChar => {
                self.apply_visual_operator(Operator::ToggleCase, register);
            }
            SimpleCommand::VisualLineMode => self.select_whole_lines(),
            SimpleCommand::VisualMode => self.end_visual(),
            SimpleCommand::PasteAfter | SimpleCommand::PasteBefore => {
                let Some(content) = self.registers.get(register).cloned() else {
                    return;
                };
                let Some((start, end)) = self.visual_range() else {
                    return;
                };
                if self.reject_edit() {
                    return;
                }
                self.buffer.begin_undo_group();
                self.apply_delete(start, end);
                let landing = self.apply_insert(start, &content.text);
                self.buffer.end_undo_group();
                self.cursor.move_to(self.buffer.clamp_position(landing));
                self.after_edit();
                self.end_visual();
            }
            SimpleCommand::Undo => {
                self.end_visual();
                self.undo();
            }
            SimpleCommand::Redo => {
                self.end_visual();
                self.redo();
            }
            _ => {
                // Insert-mode entries leave visual mode where Vim does.
                self.end_visual();
                self.apply_simple(simple, 1, register);
            }
        }
    }

    /// Grows the selection to cover whole lines, which is what `V` means.
    fn select_whole_lines(&mut self) {
        let Some((start, end)) = self.cursor.selection_range() else {
            return;
        };
        self.cursor.move_to(Position::new(start.line, 0));
        self.cursor.start_selection();
        let last_col = self.buffer.line_len_chars(end.line);
        self.cursor.extend_to(Position::new(end.line, last_col));
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
    fn v_then_a_motion_selects_and_d_deletes_it() {
        let mut editor = editor_with("hello world\n");
        feed(&mut editor, "vlld");
        assert_eq!(editor.buffer().text(), "lo world\n");
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn a_visual_yank_fills_the_unnamed_register() {
        let mut editor = editor_with("abcdef\n");
        feed(&mut editor, "vlly");
        assert_eq!(
            editor.registers().get(None).map(|c| c.text.clone()),
            Some("abc".to_string())
        );
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn shift_v_selects_the_whole_line() {
        let mut editor = editor_with("one\ntwo\n");
        feed(&mut editor, "V");
        assert_eq!(
            editor.cursor().selection_range(),
            Some((Position::new(0, 0), Position::new(0, 3)))
        );
        // A line-wise delete takes the line break with it, as Vim's `Vd` does.
        feed(&mut editor, "d");
        assert_eq!(editor.buffer().text(), "two\n");
    }

    #[test]
    fn escape_leaves_visual_mode_without_editing() {
        let mut editor = editor_with("abc\n");
        feed(&mut editor, "vl");
        editor.feed_vim_key(VimKey::Escape);
        assert_eq!(editor.mode(), EditorMode::Normal);
        assert!(!editor.cursor().has_selection());
        assert_eq!(editor.buffer().text(), "abc\n");
    }

    #[test]
    fn c_in_visual_mode_deletes_and_enters_insert() {
        let mut editor = editor_with("abcdef\n");
        feed(&mut editor, "vlc");
        assert_eq!(editor.buffer().text(), "cdef\n");
        assert_eq!(editor.mode(), EditorMode::Insert);
    }

    #[test]
    fn a_visual_case_operator_applies_to_the_selection() {
        let mut editor = editor_with("hello\n");
        feed(&mut editor, "vllgU");
        assert_eq!(editor.buffer().text(), "HELlo\n");
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn a_visual_indent_shifts_the_selected_lines() {
        let mut editor = editor_with("a\nb\nc\n");
        feed(&mut editor, "vj>");
        assert_eq!(editor.buffer().text(), "    a\n    b\nc\n");
        assert_eq!(editor.mode(), EditorMode::Normal);
    }

    #[test]
    fn x_in_visual_mode_deletes_the_selection() {
        let mut editor = editor_with("abcdef\n");
        feed(&mut editor, "vllx");
        assert_eq!(editor.buffer().text(), "def\n");
    }

    #[test]
    fn a_visual_paste_replaces_the_selection() {
        let mut editor = editor_with("abc def\n");
        feed(&mut editor, "vlly");
        assert_eq!(
            editor.registers().get(None).map(|c| c.text.clone()),
            Some("abc".to_string())
        );
        editor.set_cursor_position(Position::new(0, 4));
        feed(&mut editor, "vllp");
        assert_eq!(editor.buffer().text(), "abc abc\n");
    }

    #[test]
    fn tilde_in_visual_mode_flips_the_selection() {
        let mut editor = editor_with("aBcD\n");
        feed(&mut editor, "vlll~");
        assert_eq!(editor.buffer().text(), "AbCd\n");
    }

    #[test]
    fn a_read_only_document_refuses_a_visual_delete() {
        let mut editor = editor_with("abc\n");
        editor.set_read_only(true);
        feed(&mut editor, "vld");
        assert_eq!(editor.buffer().text(), "abc\n");
    }

    #[test]
    fn the_visual_range_includes_the_character_under_the_cursor() {
        let mut editor = editor_with("abcd\n");
        feed(&mut editor, "vl");
        assert_eq!(
            editor.visual_range(),
            Some((Position::new(0, 0), Position::new(0, 2)))
        );
    }
}
