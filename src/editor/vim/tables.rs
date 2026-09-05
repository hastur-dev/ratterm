//! Key tables and the parser's pending-stage enum.
//!
//! Separated from `state.rs` so neither file grows past the project's size
//! limit.

use super::command::{Operator, SimpleCommand};
use super::motion::Motion;

/// Which `f`-family search was last used, so `;` and `,` can repeat it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FindKind {
    Forward,
    Backward,
    TillForward,
    TillBackward,
}

impl FindKind {
    pub(super) const fn motion(self, target: char) -> Motion {
        match self {
            Self::Forward => Motion::FindForward(target),
            Self::Backward => Motion::FindBackward(target),
            Self::TillForward => Motion::TillForward(target),
            Self::TillBackward => Motion::TillBackward(target),
        }
    }

    pub(super) const fn reversed(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
            Self::TillForward => Self::TillBackward,
            Self::TillBackward => Self::TillForward,
        }
    }
}

/// What the state machine is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Stage {
    #[default]
    Start,
    AwaitingRegister,
    AfterOperator,
    AwaitingTextObject {
        around: bool,
    },
    AwaitingFind(FindKind),
    AwaitingMark {
        goto: bool,
        linewise: bool,
    },
    AwaitingReplaceChar,
    AwaitingG,
    CommandLine,
}

/// Maps a key to the motion it names, for the keys that are motions in both
/// normal and operator-pending mode.
pub(super) fn motion_for_key(key: char) -> Option<Motion> {
    Some(match key {
        'h' => Motion::Left,
        'l' => Motion::Right,
        'j' => Motion::Down,
        'k' => Motion::Up,
        'w' => Motion::WordForward,
        'W' => Motion::WordForwardBig,
        'b' => Motion::WordBack,
        'B' => Motion::WordBackBig,
        'e' => Motion::WordEnd,
        'E' => Motion::WordEndBig,
        '^' => Motion::FirstNonBlank,
        '$' => Motion::LineEnd,
        '{' => Motion::ParagraphBack,
        '}' => Motion::ParagraphForward,
        '%' => Motion::MatchPair,
        _ => return None,
    })
}

/// The key that doubles a `g`-prefixed operator into a linewise one.
pub(super) const fn case_double_key(operator: Operator) -> Option<char> {
    match operator {
        Operator::Lowercase => Some('u'),
        Operator::Uppercase => Some('U'),
        Operator::ToggleCase => Some('~'),
        _ => None,
    }
}

/// A command that needs neither an operator nor a motion.
pub(super) const fn simple_for_key(key: char) -> Option<SimpleCommand> {
    Some(match key {
        'i' => SimpleCommand::InsertBefore,
        'I' => SimpleCommand::InsertLineStart,
        'a' => SimpleCommand::InsertAfter,
        'A' => SimpleCommand::AppendLineEnd,
        'o' => SimpleCommand::OpenBelow,
        'O' => SimpleCommand::OpenAbove,
        'x' => SimpleCommand::DeleteChar,
        'X' => SimpleCommand::DeleteCharBefore,
        'D' => SimpleCommand::DeleteToLineEnd,
        'C' => SimpleCommand::ChangeToLineEnd,
        's' => SimpleCommand::SubstituteChar,
        'S' => SimpleCommand::SubstituteLine,
        'p' => SimpleCommand::PasteAfter,
        'P' => SimpleCommand::PasteBefore,
        'J' => SimpleCommand::JoinLines,
        'u' => SimpleCommand::Undo,
        'v' => SimpleCommand::VisualMode,
        'V' => SimpleCommand::VisualLineMode,
        '~' => SimpleCommand::ToggleCaseChar,
        _ => return None,
    })
}
