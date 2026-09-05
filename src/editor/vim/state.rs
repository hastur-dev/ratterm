//! The Vim key-sequence parser itself.
//!
//! Split out of `mod.rs` so neither file grows past the project's size limit.

use std::collections::HashMap;

use crate::editor::buffer::Position;

use super::command::{
    Operator, OperatorTarget, SimpleCommand, VimAction, VimCommand, parse_substitute,
};
use super::keys::VimKey;
use super::motion::{MAX_MOTION_COUNT, Motion};
use super::tables::{FindKind, Stage, case_double_key, motion_for_key, simple_for_key};
use super::textobject::{TextObject, TextObjectKind};

/// What feeding one key produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimOutcome {
    /// The sequence is incomplete; feed more keys.
    Pending,
    /// The sequence resolved.
    Command(VimCommand),
    /// The sequence is not a command. The state machine has reset itself.
    Rejected,
}

/// The Vim key-sequence parser.
#[derive(Debug, Clone, Default)]
pub struct VimState {
    stage: Stage,
    register: Option<char>,
    count_before: Option<usize>,
    count_after: Option<usize>,
    operator: Option<Operator>,
    command_line: String,
    last_change: Option<VimCommand>,
    last_find: Option<(FindKind, char)>,
    marks: HashMap<char, Position>,
}

impl VimState {
    /// Creates a state machine with nothing pending.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when a partial sequence is in progress.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.stage != Stage::Start
            || self.register.is_some()
            || self.count_before.is_some()
            || self.operator.is_some()
    }

    /// Returns the `:` line typed so far, empty when not in command-line mode.
    #[must_use]
    pub fn command_line(&self) -> &str {
        &self.command_line
    }

    /// Returns the command `.` would replay.
    #[must_use]
    pub const fn last_change(&self) -> Option<&VimCommand> {
        self.last_change.as_ref()
    }

    /// Records where a mark points, after the caller executed `m{char}`.
    pub fn set_mark(&mut self, name: char, pos: Position) {
        self.marks.insert(name, pos);
    }

    /// Returns a mark's position.
    #[must_use]
    pub fn mark(&self, name: char) -> Option<Position> {
        self.marks.get(&name).copied()
    }

    /// Drops any partial sequence. Marks, registers named so far, the last find
    /// and the last change survive.
    pub fn reset(&mut self) {
        self.stage = Stage::Start;
        self.register = None;
        self.count_before = None;
        self.count_after = None;
        self.operator = None;
        self.command_line.clear();
    }

    /// Feeds one key.
    pub fn feed(&mut self, key: VimKey) -> VimOutcome {
        if key == VimKey::Escape && self.stage != Stage::CommandLine {
            self.reset();
            return VimOutcome::Pending;
        }
        match self.stage {
            Stage::Start | Stage::AfterOperator => self.feed_normal(key),
            Stage::AwaitingRegister => self.feed_register(key),
            Stage::AwaitingTextObject { around } => self.feed_text_object(key, around),
            Stage::AwaitingFind(kind) => self.feed_find(key, kind),
            Stage::AwaitingMark { goto, linewise } => self.feed_mark(key, goto, linewise),
            Stage::AwaitingReplaceChar => self.feed_replace(key),
            Stage::AwaitingG => self.feed_g(key),
            Stage::CommandLine => self.feed_command_line(key),
        }
    }

    fn reject(&mut self) -> VimOutcome {
        self.reset();
        VimOutcome::Rejected
    }

    fn total_count(&self) -> usize {
        self.count_before
            .unwrap_or(1)
            .saturating_mul(self.count_after.unwrap_or(1))
            .clamp(1, MAX_MOTION_COUNT)
    }

    fn explicit_count(&self) -> Option<usize> {
        if self.count_before.is_none() && self.count_after.is_none() {
            None
        } else {
            Some(self.total_count())
        }
    }

    fn active_count(&self) -> Option<usize> {
        if self.operator.is_some() {
            self.count_after
        } else {
            self.count_before
        }
    }

    fn push_count(&mut self, digit: usize) {
        let slot = if self.operator.is_some() {
            &mut self.count_after
        } else {
            &mut self.count_before
        };
        let value = slot
            .unwrap_or(0)
            .saturating_mul(10)
            .saturating_add(digit)
            .min(MAX_MOTION_COUNT);
        *slot = Some(value);
    }

    fn emit(&mut self, command: VimCommand) -> VimOutcome {
        self.reset();
        if command.is_change() {
            self.last_change = Some(command.clone());
        }
        VimOutcome::Command(command)
    }

    fn emit_simple(&mut self, simple: SimpleCommand) -> VimOutcome {
        let command = VimCommand {
            register: self.register,
            count: self.total_count(),
            action: VimAction::Simple(simple),
        };
        self.emit(command)
    }

    fn finish_motion(&mut self, motion: Motion) -> VimOutcome {
        let command = VimCommand {
            register: self.register,
            count: self.total_count(),
            action: match self.operator {
                Some(operator) => VimAction::Operate {
                    operator,
                    target: OperatorTarget::Motion(motion),
                },
                None => VimAction::Motion(motion),
            },
        };
        self.emit(command)
    }

    fn finish_operator_line(&mut self) -> VimOutcome {
        let Some(operator) = self.operator else {
            return self.reject();
        };
        let command = VimCommand {
            register: self.register,
            count: self.total_count(),
            action: VimAction::Operate {
                operator,
                target: OperatorTarget::Line,
            },
        };
        self.emit(command)
    }

    fn feed_normal(&mut self, key: VimKey) -> VimOutcome {
        let after_operator = self.stage == Stage::AfterOperator;

        let Some(c) = key.as_char() else {
            if key == VimKey::Ctrl('r') && !after_operator {
                return self.emit_simple(SimpleCommand::Redo);
            }
            return self.reject();
        };

        if let Some(digit) = key.digit() {
            if digit == 0 && self.active_count().is_none() {
                return self.finish_motion(Motion::LineStart);
            }
            self.push_count(digit);
            return VimOutcome::Pending;
        }

        if let Some(operator) = Operator::from_key(c) {
            if after_operator {
                if self.operator == Some(operator) {
                    return self.finish_operator_line();
                }
                return self.reject();
            }
            self.operator = Some(operator);
            self.stage = Stage::AfterOperator;
            return VimOutcome::Pending;
        }

        if after_operator
            && let Some(operator) = self.operator
            && case_double_key(operator) == Some(c)
        {
            return self.finish_operator_line();
        }

        if let Some(motion) = motion_for_key(c) {
            return self.finish_motion(motion);
        }

        match c {
            'g' => {
                self.stage = Stage::AwaitingG;
                VimOutcome::Pending
            }
            'G' => {
                let motion = self
                    .explicit_count()
                    .map_or(Motion::FileEnd, Motion::GotoLine);
                self.finish_motion(motion)
            }
            'f' | 'F' | 't' | 'T' => {
                let kind = match c {
                    'f' => FindKind::Forward,
                    'F' => FindKind::Backward,
                    't' => FindKind::TillForward,
                    _ => FindKind::TillBackward,
                };
                self.stage = Stage::AwaitingFind(kind);
                VimOutcome::Pending
            }
            ';' | ',' => {
                let Some((kind, target)) = self.last_find else {
                    return self.reject();
                };
                let kind = if c == ';' { kind } else { kind.reversed() };
                self.finish_motion(kind.motion(target))
            }
            'i' | 'a' if after_operator => {
                self.stage = Stage::AwaitingTextObject { around: c == 'a' };
                VimOutcome::Pending
            }
            _ if after_operator => self.reject(),
            '"' => {
                self.stage = Stage::AwaitingRegister;
                VimOutcome::Pending
            }
            'r' => {
                self.stage = Stage::AwaitingReplaceChar;
                VimOutcome::Pending
            }
            'm' => {
                self.stage = Stage::AwaitingMark {
                    goto: false,
                    linewise: false,
                };
                VimOutcome::Pending
            }
            '`' | '\'' => {
                self.stage = Stage::AwaitingMark {
                    goto: true,
                    linewise: c == '\'',
                };
                VimOutcome::Pending
            }
            ':' => {
                self.stage = Stage::CommandLine;
                self.command_line.clear();
                VimOutcome::Pending
            }
            '.' => {
                let Some(mut last) = self.last_change.clone() else {
                    return self.reject();
                };
                if let Some(count) = self.explicit_count() {
                    last.count = count;
                }
                if let Some(register) = self.register {
                    last.register = Some(register);
                }
                self.emit(last)
            }
            _ => match simple_for_key(c) {
                Some(simple) => self.emit_simple(simple),
                None => self.reject(),
            },
        }
    }

    fn feed_register(&mut self, key: VimKey) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        if c.is_alphanumeric() || matches!(c, '"' | '_' | '+' | '*' | '-') {
            self.register = Some(c);
            self.stage = Stage::Start;
            VimOutcome::Pending
        } else {
            self.reject()
        }
    }

    fn feed_text_object(&mut self, key: VimKey, around: bool) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        let (Some(kind), Some(operator)) = (TextObjectKind::from_key(c), self.operator) else {
            return self.reject();
        };
        let command = VimCommand {
            register: self.register,
            count: self.total_count(),
            action: VimAction::Operate {
                operator,
                target: OperatorTarget::TextObject(TextObject { kind, around }),
            },
        };
        self.emit(command)
    }

    fn feed_find(&mut self, key: VimKey, kind: FindKind) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        self.last_find = Some((kind, c));
        self.finish_motion(kind.motion(c))
    }

    fn feed_mark(&mut self, key: VimKey, goto: bool, linewise: bool) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        // An operator plus a mark is not supported; rejecting is clearer than
        // silently dropping the operator.
        if !c.is_alphanumeric() || self.operator.is_some() {
            return self.reject();
        }
        let action = if goto {
            VimAction::GotoMark { mark: c, linewise }
        } else {
            VimAction::SetMark(c)
        };
        self.emit(VimCommand {
            register: None,
            count: 1,
            action,
        })
    }

    fn feed_replace(&mut self, key: VimKey) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        self.emit_simple(SimpleCommand::ReplaceChar(c))
    }

    fn feed_g(&mut self, key: VimKey) -> VimOutcome {
        let Some(c) = key.as_char() else {
            return self.reject();
        };
        match c {
            'g' => {
                let motion = self
                    .explicit_count()
                    .map_or(Motion::FileStart, Motion::GotoLine);
                self.finish_motion(motion)
            }
            'u' | 'U' | '~' if self.operator.is_none() => {
                self.operator = Some(match c {
                    'u' => Operator::Lowercase,
                    'U' => Operator::Uppercase,
                    _ => Operator::ToggleCase,
                });
                self.stage = Stage::AfterOperator;
                VimOutcome::Pending
            }
            _ => self.reject(),
        }
    }

    fn feed_command_line(&mut self, key: VimKey) -> VimOutcome {
        match key {
            VimKey::Escape => {
                self.reset();
                VimOutcome::Pending
            }
            VimKey::Enter => {
                let text = std::mem::take(&mut self.command_line);
                if text.is_empty() {
                    return self.reject();
                }
                let action = parse_substitute(&text)
                    .map_or_else(|| VimAction::Ex(text.clone()), VimAction::Substitute);
                self.emit(VimCommand {
                    register: None,
                    count: 1,
                    action,
                })
            }
            VimKey::Backspace => {
                if self.command_line.pop().is_none() {
                    self.reset();
                }
                VimOutcome::Pending
            }
            VimKey::Char(c) => {
                self.command_line.push(c);
                VimOutcome::Pending
            }
            VimKey::Tab | VimKey::Ctrl(_) => VimOutcome::Pending,
        }
    }
}
