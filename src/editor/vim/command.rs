//! The typed commands the Vim state machine emits.
//!
//! A [`VimCommand`] describes what to do; it never touches a buffer. The input
//! layer reads one and calls the editor. Keeping the description separate is
//! what makes the state machine testable and dot-repeat a matter of replaying a
//! value.

use super::motion::Motion;
use super::textobject::TextObject;

/// An operator, waiting for something to operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// `d`
    Delete,
    /// `c`
    Change,
    /// `y`
    Yank,
    /// `>`
    Indent,
    /// `<`
    Outdent,
    /// `gu`
    Lowercase,
    /// `gU`
    Uppercase,
    /// `g~`
    ToggleCase,
}

impl Operator {
    /// Returns the single key that starts this operator, if it has one.
    ///
    /// The case operators are entered through `g` and so have no single key.
    #[must_use]
    pub const fn key(self) -> Option<char> {
        match self {
            Self::Delete => Some('d'),
            Self::Change => Some('c'),
            Self::Yank => Some('y'),
            Self::Indent => Some('>'),
            Self::Outdent => Some('<'),
            Self::Lowercase | Self::Uppercase | Self::ToggleCase => None,
        }
    }

    /// Maps a single key to the operator it starts.
    #[must_use]
    pub const fn from_key(key: char) -> Option<Self> {
        match key {
            'd' => Some(Self::Delete),
            'c' => Some(Self::Change),
            'y' => Some(Self::Yank),
            '>' => Some(Self::Indent),
            '<' => Some(Self::Outdent),
            _ => None,
        }
    }

    /// Returns true when the operator modifies the buffer.
    #[must_use]
    pub const fn is_change(self) -> bool {
        !matches!(self, Self::Yank)
    }
}

/// What an operator applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorTarget {
    /// A motion, as in `dw`.
    Motion(Motion),
    /// A text object, as in `ciw`.
    TextObject(TextObject),
    /// Whole lines, as in `dd`.
    Line,
}

/// A command that needs no operator and no motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleCommand {
    /// `i`
    InsertBefore,
    /// `a`
    InsertAfter,
    /// `I`
    InsertLineStart,
    /// `A`
    AppendLineEnd,
    /// `o`
    OpenBelow,
    /// `O`
    OpenAbove,
    /// `x`
    DeleteChar,
    /// `X`
    DeleteCharBefore,
    /// `D`
    DeleteToLineEnd,
    /// `C`
    ChangeToLineEnd,
    /// `s`
    SubstituteChar,
    /// `S`
    SubstituteLine,
    /// `r{char}`
    ReplaceChar(char),
    /// `p`
    PasteAfter,
    /// `P`
    PasteBefore,
    /// `J`
    JoinLines,
    /// `u`
    Undo,
    /// `Ctrl-r`
    Redo,
    /// `v`
    VisualMode,
    /// `V`
    VisualLineMode,
    /// `~`
    ToggleCaseChar,
}

impl SimpleCommand {
    /// Returns true when the command modifies the buffer.
    ///
    /// Mode switches, paste-position choices aside, and undo/redo are excluded:
    /// dot-repeat must not replay them.
    #[must_use]
    pub const fn is_change(self) -> bool {
        matches!(
            self,
            Self::DeleteChar
                | Self::DeleteCharBefore
                | Self::DeleteToLineEnd
                | Self::ChangeToLineEnd
                | Self::SubstituteChar
                | Self::SubstituteLine
                | Self::ReplaceChar(_)
                | Self::PasteAfter
                | Self::PasteBefore
                | Self::JoinLines
                | Self::ToggleCaseChar
        )
    }
}

/// A parsed `:s` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substitute {
    /// Text to look for.
    pub pattern: String,
    /// Text to put in its place.
    pub replacement: String,
    /// `%` was given: apply to the whole file rather than the current line.
    pub whole_file: bool,
    /// The `g` flag: replace every match on a line, not just the first.
    pub all_occurrences: bool,
    /// The `i` flag.
    pub ignore_case: bool,
}

/// Parses `:s/old/new/flags` and `:%s/old/new/flags`.
///
/// Any non-alphanumeric character may be the separator, and `\` escapes it
/// inside the pattern or the replacement. Returns `None` when the input is not
/// a substitute command, when the pattern is empty, or when a flag is unknown.
#[must_use]
pub fn parse_substitute(input: &str) -> Option<Substitute> {
    let mut rest = input.trim();
    if let Some(stripped) = rest.strip_prefix(':') {
        rest = stripped.trim_start();
    }
    let whole_file = rest.starts_with('%');
    if whole_file {
        rest = rest.get(1..)?;
    }
    let rest = rest.strip_prefix('s')?;

    let mut chars = rest.chars();
    let separator = chars.next()?;
    if separator.is_alphanumeric() || separator == '\\' || separator == '"' || separator == '|' {
        return None;
    }

    let body: Vec<char> = chars.collect();
    let mut fields: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut i = 0usize;
    while i < body.len() {
        let c = body[i];
        if c == '\\' && i + 1 < body.len() {
            if body[i + 1] == separator {
                current.push(separator);
            } else {
                current.push('\\');
                current.push(body[i + 1]);
            }
            i += 2;
            continue;
        }
        if c == separator {
            fields.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    fields.push(current);

    let pattern = fields.first()?.clone();
    if pattern.is_empty() {
        return None;
    }
    let replacement = fields.get(1).cloned().unwrap_or_default();

    let mut all_occurrences = false;
    let mut ignore_case = false;
    for flag in fields.get(2).map_or("", String::as_str).chars() {
        match flag {
            'g' => all_occurrences = true,
            'i' => ignore_case = true,
            'I' => ignore_case = false,
            'c' => {}
            _ => return None,
        }
    }

    Some(Substitute {
        pattern,
        replacement,
        whole_file,
        all_occurrences,
        ignore_case,
    })
}

/// What a completed key sequence asks the editor to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimAction {
    /// Move the cursor.
    Motion(Motion),
    /// Apply an operator to a range.
    Operate {
        /// The operator.
        operator: Operator,
        /// What it applies to.
        target: OperatorTarget,
    },
    /// A standalone command.
    Simple(SimpleCommand),
    /// `m{char}`
    SetMark(char),
    /// `` `{char} `` or `'{char}`
    GotoMark {
        /// The mark's name.
        mark: char,
        /// `true` for `'{char}`, which moves to the mark's line.
        linewise: bool,
    },
    /// A parsed `:s` command.
    Substitute(Substitute),
    /// Any other `:` command, without the colon.
    Ex(String),
}

/// A complete command: what to do, how many times, and through which register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VimCommand {
    /// The register named with `"a`, if any.
    pub register: Option<char>,
    /// The resolved count. Counts before and after an operator multiply, so
    /// `2d3w` arrives here as 6. Always at least 1.
    pub count: usize,
    /// What to do.
    pub action: VimAction,
}

impl VimCommand {
    /// Builds a command with no register and a count of one.
    #[must_use]
    pub fn new(action: VimAction) -> Self {
        Self {
            register: None,
            count: 1,
            action,
        }
    }

    /// Returns true when replaying this command with `.` makes sense.
    #[must_use]
    pub fn is_change(&self) -> bool {
        match &self.action {
            VimAction::Operate { operator, .. } => operator.is_change(),
            VimAction::Simple(cmd) => cmd.is_change(),
            VimAction::Substitute(_) => true,
            VimAction::Motion(_)
            | VimAction::SetMark(_)
            | VimAction::GotoMark { .. }
            | VimAction::Ex(_) => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn operator_keys_round_trip() {
        for op in [
            Operator::Delete,
            Operator::Change,
            Operator::Yank,
            Operator::Indent,
            Operator::Outdent,
        ] {
            let key = op.key().expect("has a key");
            assert_eq!(Operator::from_key(key), Some(op));
        }
        assert_eq!(Operator::Lowercase.key(), None);
        assert_eq!(Operator::from_key('z'), None);
    }

    #[test]
    fn only_yank_is_not_a_change() {
        assert!(Operator::Delete.is_change());
        assert!(Operator::ToggleCase.is_change());
        assert!(!Operator::Yank.is_change());
    }

    #[test]
    fn mode_switches_and_undo_are_not_changes() {
        assert!(!SimpleCommand::InsertBefore.is_change());
        assert!(!SimpleCommand::Undo.is_change());
        assert!(!SimpleCommand::Redo.is_change());
        assert!(!SimpleCommand::VisualMode.is_change());
        assert!(SimpleCommand::DeleteChar.is_change());
        assert!(SimpleCommand::ReplaceChar('x').is_change());
    }

    #[test]
    fn parses_a_whole_file_substitute_with_flags() {
        let got = parse_substitute(":%s/a/b/g").expect("parses");
        assert_eq!(
            got,
            Substitute {
                pattern: "a".to_string(),
                replacement: "b".to_string(),
                whole_file: true,
                all_occurrences: true,
                ignore_case: false,
            }
        );
    }

    #[test]
    fn parses_a_line_substitute_without_flags() {
        let got = parse_substitute("s/old/new/").expect("parses");
        assert_eq!(got.pattern, "old");
        assert_eq!(got.replacement, "new");
        assert!(!got.whole_file);
        assert!(!got.all_occurrences);
    }

    #[test]
    fn parses_the_ignore_case_flag_and_its_override() {
        assert!(parse_substitute(":%s/a/b/gi").expect("parses").ignore_case);
        assert!(!parse_substitute(":%s/a/b/iI").expect("parses").ignore_case);
        // The confirm flag is accepted and ignored.
        assert!(parse_substitute(":s/a/b/gc").is_some());
    }

    #[test]
    fn accepts_an_alternative_separator_and_escapes() {
        let got = parse_substitute(":s#a/b#c#").expect("parses");
        assert_eq!(got.pattern, "a/b");
        assert_eq!(got.replacement, "c");

        let got = parse_substitute(r":s/a\/b/c/").expect("parses");
        assert_eq!(got.pattern, "a/b");
    }

    #[test]
    fn a_missing_replacement_deletes_the_match() {
        let got = parse_substitute(":s/gone").expect("parses");
        assert_eq!(got.pattern, "gone");
        assert_eq!(got.replacement, "");
    }

    #[test]
    fn rejects_input_that_is_not_a_substitute() {
        assert!(parse_substitute(":w").is_none());
        assert!(parse_substitute("").is_none());
        assert!(parse_substitute(":s").is_none());
        assert!(parse_substitute(":sabc").is_none());
        assert!(parse_substitute(":s//b/").is_none());
        assert!(parse_substitute(":s/a/b/z").is_none());
    }

    #[test]
    fn only_mutating_actions_repeat() {
        assert!(
            VimCommand::new(VimAction::Operate {
                operator: Operator::Delete,
                target: OperatorTarget::Line,
            })
            .is_change()
        );
        assert!(
            !VimCommand::new(VimAction::Operate {
                operator: Operator::Yank,
                target: OperatorTarget::Line,
            })
            .is_change()
        );
        assert!(!VimCommand::new(VimAction::Motion(Motion::WordForward)).is_change());
        assert!(!VimCommand::new(VimAction::SetMark('a')).is_change());
        assert!(!VimCommand::new(VimAction::Ex("w".to_string())).is_change());
    }
}
