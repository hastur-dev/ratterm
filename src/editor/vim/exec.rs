//! Applying a resolved Vim command to the editor.
//!
//! [`VimState`](super::VimState) turns keys into a [`VimCommand`]; this file is
//! the other half — it takes one and does it. Nothing here parses keys, so the
//! two halves can be tested apart.

use crate::editor::indent::{apply_indent_edits, indent_lines, outdent_lines};
use crate::editor::{Editor, EditorMode, Position};

use super::command::{Operator, OperatorTarget, Substitute, VimAction, VimCommand};
use super::keys::VimKey;
use super::motion::{MotionKind, last_line, resolve_motion};
use super::registers::RegisterContent;
use super::state::VimOutcome;
use super::textobject::resolve_text_object;

/// Something the editor cannot do on its own, handed back to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimEffect {
    /// Nothing further is needed.
    None,
    /// `:w` — write the file.
    Save,
    /// `:q` — close the file.
    Quit,
    /// `:wq` or `:x` — write, then close.
    SaveAndQuit,
    /// `:q!` — close without writing.
    QuitWithoutSaving,
    /// An `:` command the editor does not implement, without the colon.
    Ex(String),
}

/// What feeding one key to the editor produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VimFeed {
    /// True while a partial key sequence is in progress.
    pub pending: bool,
    /// True when the sequence resolved and was applied.
    pub executed: bool,
    /// Anything the application has to finish.
    pub effect: VimEffect,
}

impl VimFeed {
    /// A key that extended an unfinished sequence.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            pending: true,
            executed: false,
            effect: VimEffect::None,
        }
    }

    /// A key that did not spell a command.
    #[must_use]
    pub const fn rejected() -> Self {
        Self {
            pending: false,
            executed: false,
            effect: VimEffect::None,
        }
    }
}

/// Parses an `:` command line into an effect.
///
/// `:12` jumps to a line, so that case is handled by the caller before this.
#[must_use]
pub fn ex_effect(command: &str) -> VimEffect {
    match command.trim() {
        "w" | "write" => VimEffect::Save,
        "q" | "quit" => VimEffect::Quit,
        "q!" | "quit!" => VimEffect::QuitWithoutSaving,
        "wq" | "x" | "xit" | "wq!" => VimEffect::SaveAndQuit,
        other => VimEffect::Ex(other.to_string()),
    }
}

impl Editor {
    /// Returns the Vim parser, for the status line and for tests.
    #[must_use]
    pub const fn vim(&self) -> &super::state::VimState {
        &self.vim
    }

    /// Returns the Vim registers.
    #[must_use]
    pub const fn registers(&self) -> &super::registers::Registers {
        &self.registers
    }

    /// Returns the `:` line typed so far.
    #[must_use]
    pub fn vim_command_line(&self) -> &str {
        self.vim.command_line()
    }

    /// Feeds one key to the Vim state machine, applying whatever it resolves to.
    pub fn feed_vim_key(&mut self, key: VimKey) -> VimFeed {
        if self.mode() == EditorMode::Visual {
            return self.feed_vim_visual_key(key);
        }

        let mut vim = std::mem::take(&mut self.vim);
        let outcome = vim.feed(key);
        self.vim = vim;
        self.sync_command_mode();

        match outcome {
            VimOutcome::Pending => VimFeed::pending(),
            VimOutcome::Rejected => VimFeed::rejected(),
            VimOutcome::Command(command) => {
                let effect = self.execute_vim(&command);
                self.sync_command_mode();
                VimFeed {
                    pending: false,
                    executed: true,
                    effect,
                }
            }
        }
    }

    /// Keeps [`EditorMode::Command`] in step with the parser's `:` state.
    fn sync_command_mode(&mut self) {
        if self.vim.in_command_line() {
            self.set_mode(EditorMode::Command);
        } else if self.mode() == EditorMode::Command {
            self.set_mode(EditorMode::Normal);
        }
    }

    /// Applies a resolved command.
    pub fn execute_vim(&mut self, command: &VimCommand) -> VimEffect {
        match &command.action {
            VimAction::Motion(motion) => {
                if let Some((target, _)) =
                    resolve_motion(&self.buffer, self.cursor.position(), motion, command.count)
                {
                    self.cursor.move_to(self.buffer.clamp_position(target));
                    self.clamp_cursor_out_of_folds();
                    self.ensure_cursor_visible();
                }
                VimEffect::None
            }
            VimAction::Operate { operator, target } => {
                self.apply_operator(*operator, target, command.count, command.register);
                VimEffect::None
            }
            VimAction::Simple(simple) => {
                self.apply_simple(*simple, command.count, command.register);
                VimEffect::None
            }
            VimAction::SetMark(name) => {
                let pos = self.cursor.position();
                self.vim.set_mark(*name, pos);
                VimEffect::None
            }
            VimAction::GotoMark { mark, linewise } => {
                if let Some(pos) = self.vim.mark(*mark) {
                    let target = if *linewise {
                        self.buffer
                            .first_non_whitespace(pos.line)
                            .unwrap_or(Position::new(pos.line, 0))
                    } else {
                        pos
                    };
                    self.cursor.move_to(self.buffer.clamp_position(target));
                    self.clamp_cursor_out_of_folds();
                    self.ensure_cursor_visible();
                }
                VimEffect::None
            }
            VimAction::Substitute(sub) => {
                let count = self.apply_substitute(sub);
                self.set_status(format!("{count} substitutions"));
                VimEffect::None
            }
            VimAction::Ex(line) => {
                if let Ok(line_number) = line.trim().parse::<usize>() {
                    self.goto_line(line_number.saturating_sub(1));
                    return VimEffect::None;
                }
                ex_effect(line)
            }
        }
    }

    /// Returns the half-open range an operator target covers, and whether it is
    /// linewise.
    fn operator_range(
        &self,
        target: &OperatorTarget,
        count: usize,
    ) -> Option<(Position, Position, bool)> {
        let pos = self.cursor.position();
        match target {
            OperatorTarget::Line => {
                let last = (pos.line + count - 1).min(last_line(&self.buffer));
                let (start, end) = self.line_span(pos.line, last);
                Some((start, end, true))
            }
            OperatorTarget::TextObject(object) => {
                let (start, end) = resolve_text_object(&self.buffer, pos, object)?;
                Some((start, end, false))
            }
            OperatorTarget::Motion(motion) => {
                let (found, kind) = resolve_motion(&self.buffer, pos, motion, count)?;
                match kind {
                    MotionKind::Linewise => {
                        let first = pos.line.min(found.line);
                        let last = pos.line.max(found.line);
                        let (start, end) = self.line_span(first, last);
                        Some((start, end, true))
                    }
                    MotionKind::Inclusive => {
                        let (start, end) = order(pos, found);
                        let end_index = self.buffer.position_to_index(end) + 1;
                        Some((start, self.buffer.index_to_position(end_index), false))
                    }
                    MotionKind::Exclusive => {
                        let (start, end) = order(pos, found);
                        Some((start, end, false))
                    }
                }
            }
        }
    }

    /// Returns the half-open range covering whole lines `first` through `last`.
    fn line_span(&self, first: usize, last: usize) -> (Position, Position) {
        let end_of_buffer = self.buffer.len_lines().saturating_sub(1);
        let end = if last < end_of_buffer {
            Position::new(last + 1, 0)
        } else {
            Position::new(last, self.buffer.line_len_chars(last))
        };
        (Position::new(first, 0), end)
    }

    /// Applies one operator to one range.
    fn apply_operator(
        &mut self,
        operator: Operator,
        target: &OperatorTarget,
        count: usize,
        register: Option<char>,
    ) {
        let Some((start, end, linewise)) = self.operator_range(target, count) else {
            return;
        };
        if operator != Operator::Yank && self.reject_edit() {
            return;
        }
        let text = self.buffer.get_range(start, end).unwrap_or_default();

        match operator {
            Operator::Yank => {
                self.registers.set_yank(register, content(text, linewise));
                self.cursor.move_to(start);
            }
            Operator::Delete | Operator::Change => {
                self.registers.set_delete(register, content(text, linewise));
                self.buffer.begin_undo_group();
                self.apply_delete(start, end);
                if operator == Operator::Change && linewise {
                    // `cc` keeps a line to type on.
                    self.apply_insert(start, "\n");
                }
                self.buffer.end_undo_group();
                self.cursor.move_to(self.buffer.clamp_position(start));
                if operator == Operator::Change {
                    self.set_mode(EditorMode::Insert);
                } else if linewise {
                    let line = self.cursor.position().line;
                    let landing = self
                        .buffer
                        .first_non_whitespace(line)
                        .unwrap_or(Position::new(line, 0));
                    self.cursor.move_to(landing);
                }
                self.after_edit();
            }
            Operator::Indent | Operator::Outdent => {
                let lines = start.line..=inclusive_last(start, end).min(last_line(&self.buffer));
                let edits = if operator == Operator::Indent {
                    indent_lines(&self.buffer, lines, self.indent_style)
                } else {
                    outdent_lines(&self.buffer, lines, self.indent_style)
                };
                apply_indent_edits(&mut self.buffer, &edits);
                self.note_syntax_reset();
                self.cursor.clamp(&self.buffer);
                self.after_edit();
            }
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                let replaced = transform_case(&text, operator);
                self.buffer.begin_undo_group();
                self.apply_delete(start, end);
                self.apply_insert(start, &replaced);
                self.buffer.end_undo_group();
                self.cursor.move_to(self.buffer.clamp_position(start));
                self.after_edit();
            }
        }
    }

    /// Runs a parsed `:s` command, returning how many matches changed.
    pub fn apply_substitute(&mut self, sub: &Substitute) -> usize {
        if self.reject_edit() || sub.pattern.is_empty() {
            return 0;
        }
        let lines: Vec<usize> = if sub.whole_file {
            (0..self.buffer.len_lines()).collect()
        } else {
            vec![self.cursor.position().line]
        };
        let width = sub.pattern.chars().count();

        let mut hits: Vec<Position> = Vec::new();
        for line in lines {
            let text = self.buffer.line_text(line);
            let haystack: Vec<char> = if sub.ignore_case {
                text.to_lowercase().chars().collect()
            } else {
                text.chars().collect()
            };
            let needle: Vec<char> = if sub.ignore_case {
                sub.pattern.to_lowercase().chars().collect()
            } else {
                sub.pattern.chars().collect()
            };
            if needle.is_empty() || needle.len() > haystack.len() {
                continue;
            }
            let mut col = 0usize;
            while col + needle.len() <= haystack.len() {
                if haystack[col..col + needle.len()] == needle[..] {
                    hits.push(Position::new(line, col));
                    if !sub.all_occurrences {
                        break;
                    }
                    col += needle.len();
                } else {
                    col += 1;
                }
            }
        }

        if hits.is_empty() {
            return 0;
        }
        hits.sort_by_key(|p| std::cmp::Reverse((p.line, p.col)));

        self.buffer.begin_undo_group();
        for start in &hits {
            let end = Position::new(start.line, start.col + width);
            self.apply_delete(*start, end);
            self.apply_insert(*start, &sub.replacement);
        }
        self.buffer.end_undo_group();
        self.cursor.clamp(&self.buffer);
        self.after_edit();
        hits.len()
    }
}

/// Returns the last line a half-open range actually covers.
///
/// A line-wise range ends at column zero of the line *after* the last one it
/// covers, so indenting `start..end` verbatim would shift one line too many.
fn inclusive_last(start: Position, end: Position) -> usize {
    if end.col == 0 && end.line > start.line {
        end.line - 1
    } else {
        end.line
    }
}

/// Returns the two positions in buffer order.
fn order(a: Position, b: Position) -> (Position, Position) {
    if (a.line, a.col) <= (b.line, b.col) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Builds register content, tagging it linewise when it covers whole lines.
fn content(text: String, linewise: bool) -> RegisterContent {
    if linewise {
        let text = if text.ends_with('\n') {
            text
        } else {
            format!("{text}\n")
        };
        RegisterContent::linewise(text)
    } else {
        RegisterContent::charwise(text)
    }
}

/// Applies a case operator to a string.
fn transform_case(text: &str, operator: Operator) -> String {
    text.chars()
        .map(|c| match operator {
            Operator::Lowercase => c.to_lowercase().next().unwrap_or(c),
            Operator::Uppercase => c.to_uppercase().next().unwrap_or(c),
            _ if c.is_uppercase() => c.to_lowercase().next().unwrap_or(c),
            _ => c.to_uppercase().next().unwrap_or(c),
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ex_commands_map_to_effects() {
        assert_eq!(ex_effect("w"), VimEffect::Save);
        assert_eq!(ex_effect(" write "), VimEffect::Save);
        assert_eq!(ex_effect("q"), VimEffect::Quit);
        assert_eq!(ex_effect("q!"), VimEffect::QuitWithoutSaving);
        assert_eq!(ex_effect("wq"), VimEffect::SaveAndQuit);
        assert_eq!(ex_effect("x"), VimEffect::SaveAndQuit);
        assert_eq!(ex_effect("nohl"), VimEffect::Ex("nohl".to_string()));
    }

    #[test]
    fn case_transforms_cover_all_three_operators() {
        assert_eq!(transform_case("aB", Operator::Lowercase), "ab");
        assert_eq!(transform_case("aB", Operator::Uppercase), "AB");
        assert_eq!(transform_case("aB", Operator::ToggleCase), "Ab");
        assert_eq!(transform_case("", Operator::Uppercase), "");
    }

    #[test]
    fn linewise_content_always_ends_in_a_newline() {
        assert!(content("a".to_string(), true).text.ends_with('\n'));
        assert!(content("a\n".to_string(), true).text.ends_with('\n'));
        assert!(!content("a".to_string(), false).linewise);
    }

    #[test]
    fn ordering_positions_is_symmetric() {
        let a = Position::new(1, 0);
        let b = Position::new(0, 5);
        assert_eq!(order(a, b), (b, a));
        assert_eq!(order(b, a), (b, a));
        assert_eq!(order(a, a), (a, a));
    }
}
