//! Vim motions and how they resolve against a buffer.
//!
//! [`resolve_motion`] is pure: it takes a buffer and a starting position and
//! returns where the motion lands plus whether the span it covers includes its
//! endpoint. Applying that span is the caller's job.
//!
//! All columns are character offsets. Walking is bounded by
//! [`MAX_MOTION_STEPS`] characters and by the buffer's line count.

use crate::editor::brackets::{BracketConfig, matching_bracket_with};
use crate::editor::buffer::{Buffer, Position};

use super::walker::{Walker, step_word_back, step_word_end, step_word_forward};

/// Characters a single motion will step over before giving up.
pub const MAX_MOTION_STEPS: usize = 200_000;

/// Largest count a motion honours.
pub const MAX_MOTION_COUNT: usize = 100_000;

/// How the span between the start and the motion's target is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    /// The target character is not part of the span.
    Exclusive,
    /// The target character is part of the span.
    Inclusive,
    /// The span covers whole lines.
    Linewise,
}

/// A cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// `h`
    Left,
    /// `l`
    Right,
    /// `k`
    Up,
    /// `j`
    Down,
    /// `w`
    WordForward,
    /// `W`
    WordForwardBig,
    /// `b`
    WordBack,
    /// `B`
    WordBackBig,
    /// `e`
    WordEnd,
    /// `E`
    WordEndBig,
    /// `0`
    LineStart,
    /// `^`
    FirstNonBlank,
    /// `$`
    LineEnd,
    /// `gg` with no count
    FileStart,
    /// `G` with no count
    FileEnd,
    /// `{n}G` or `{n}gg`
    GotoLine(usize),
    /// `f{char}`
    FindForward(char),
    /// `F{char}`
    FindBackward(char),
    /// `t{char}`
    TillForward(char),
    /// `T{char}`
    TillBackward(char),
    /// `}`
    ParagraphForward,
    /// `{`
    ParagraphBack,
    /// `%`
    MatchPair,
}

/// Character categories used by word motions and word text objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Whitespace, including the line break.
    Blank,
    /// Letters, digits, and `_`.
    Word,
    /// Everything else.
    Punct,
}

/// Returns the category a character belongs to.
pub(crate) fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Blank
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// Returns the last line that holds text.
///
/// A file ending in a newline gives ropey an extra empty line; Vim does not
/// count it, and neither does this.
#[must_use]
pub fn last_line(buffer: &Buffer) -> usize {
    let last = buffer.len_lines().saturating_sub(1);
    if last > 0 && buffer.line_len_chars(last) == 0 {
        last - 1
    } else {
        last
    }
}

pub(super) fn line_chars(buffer: &Buffer, line: usize) -> Vec<char> {
    buffer.line(line).map_or_else(Vec::new, |raw| {
        raw.strip_suffix('\n').unwrap_or(&raw).chars().collect()
    })
}

fn is_blank_line(buffer: &Buffer, line: usize) -> bool {
    buffer.line(line).is_none_or(|raw| raw.trim().is_empty())
}

/// Finds the `count`-th occurrence of `target` on one line.
fn find_on_line(
    buffer: &Buffer,
    pos: Position,
    target: char,
    count: usize,
    forward: bool,
) -> Option<usize> {
    let chars = line_chars(buffer, pos.line);
    let mut remaining = count.max(1);
    if forward {
        for (col, c) in chars.iter().enumerate().skip(pos.col + 1) {
            if *c == target {
                remaining -= 1;
                if remaining == 0 {
                    return Some(col);
                }
            }
        }
    } else {
        for col in (0..pos.col.min(chars.len())).rev() {
            if chars[col] == target {
                remaining -= 1;
                if remaining == 0 {
                    return Some(col);
                }
            }
        }
    }
    None
}

fn paragraph_forward(buffer: &Buffer, line: usize) -> usize {
    let last = last_line(buffer);
    let mut l = line;
    if l >= last {
        return last;
    }
    l += 1;
    while l < last && is_blank_line(buffer, l) {
        l += 1;
    }
    while l < last && !is_blank_line(buffer, l) {
        l += 1;
    }
    l
}

fn paragraph_back(buffer: &Buffer, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut l = line - 1;
    while l > 0 && is_blank_line(buffer, l) {
        l -= 1;
    }
    while l > 0 && !is_blank_line(buffer, l) {
        l -= 1;
    }
    l
}

fn first_non_blank(buffer: &Buffer, line: usize) -> Position {
    buffer
        .first_non_whitespace(line)
        .unwrap_or_else(|| Position::new(line, 0))
}

/// Resolves a motion to a target position and the span kind it implies.
///
/// Returns `None` when the motion cannot move — a find that matches nothing, a
/// `%` with no bracket to match, or a step off the end of the buffer.
#[must_use]
pub fn resolve_motion(
    buffer: &Buffer,
    pos: Position,
    motion: &Motion,
    count: usize,
) -> Option<(Position, MotionKind)> {
    let count = count.clamp(1, MAX_MOTION_COUNT);
    let last = last_line(buffer);
    let pos = Position::new(
        pos.line.min(last),
        pos.col.min(buffer.line_len_chars(pos.line.min(last))),
    );
    let mut budget = MAX_MOTION_STEPS;

    match motion {
        Motion::Left => Some((
            Position::new(pos.line, pos.col.saturating_sub(count)),
            MotionKind::Exclusive,
        )),
        Motion::Right => {
            let len = buffer.line_len_chars(pos.line);
            Some((
                Position::new(pos.line, (pos.col + count).min(len)),
                MotionKind::Exclusive,
            ))
        }
        Motion::Up => {
            let line = pos.line.saturating_sub(count);
            Some((
                Position::new(line, pos.col.min(buffer.line_len_chars(line))),
                MotionKind::Linewise,
            ))
        }
        Motion::Down => {
            let line = (pos.line + count).min(last);
            Some((
                Position::new(line, pos.col.min(buffer.line_len_chars(line))),
                MotionKind::Linewise,
            ))
        }
        Motion::WordForward | Motion::WordForwardBig => {
            let big = *motion == Motion::WordForwardBig;
            let mut walker = Walker::new(buffer, pos);
            for _ in 0..count {
                if !step_word_forward(&mut walker, big, &mut budget) {
                    break;
                }
            }
            Some((walker.pos(), MotionKind::Exclusive))
        }
        Motion::WordBack | Motion::WordBackBig => {
            let big = *motion == Motion::WordBackBig;
            let mut walker = Walker::new(buffer, pos);
            for _ in 0..count {
                if !step_word_back(&mut walker, big, &mut budget) {
                    break;
                }
            }
            Some((walker.pos(), MotionKind::Exclusive))
        }
        Motion::WordEnd | Motion::WordEndBig => {
            let big = *motion == Motion::WordEndBig;
            let mut walker = Walker::new(buffer, pos);
            for _ in 0..count {
                if !step_word_end(&mut walker, big, &mut budget) {
                    break;
                }
            }
            Some((walker.pos(), MotionKind::Inclusive))
        }
        Motion::LineStart => Some((Position::new(pos.line, 0), MotionKind::Exclusive)),
        Motion::FirstNonBlank => Some((first_non_blank(buffer, pos.line), MotionKind::Exclusive)),
        Motion::LineEnd => {
            let line = (pos.line + count - 1).min(last);
            let len = buffer.line_len_chars(line);
            Some((
                Position::new(line, len.saturating_sub(1)),
                MotionKind::Inclusive,
            ))
        }
        Motion::FileStart => Some((first_non_blank(buffer, 0), MotionKind::Linewise)),
        Motion::FileEnd => Some((first_non_blank(buffer, last), MotionKind::Linewise)),
        Motion::GotoLine(n) => {
            let line = n.saturating_sub(1).min(last);
            Some((first_non_blank(buffer, line), MotionKind::Linewise))
        }
        Motion::FindForward(c) => find_on_line(buffer, pos, *c, count, true)
            .map(|col| (Position::new(pos.line, col), MotionKind::Inclusive)),
        Motion::TillForward(c) => find_on_line(buffer, pos, *c, count, true)
            .filter(|col| *col > pos.col)
            .map(|col| (Position::new(pos.line, col - 1), MotionKind::Inclusive)),
        Motion::FindBackward(c) => find_on_line(buffer, pos, *c, count, false)
            .map(|col| (Position::new(pos.line, col), MotionKind::Exclusive)),
        Motion::TillBackward(c) => find_on_line(buffer, pos, *c, count, false)
            .map(|col| (Position::new(pos.line, col + 1), MotionKind::Exclusive)),
        Motion::ParagraphForward => {
            let mut line = pos.line;
            for _ in 0..count {
                line = paragraph_forward(buffer, line);
            }
            Some((Position::new(line, 0), MotionKind::Exclusive))
        }
        Motion::ParagraphBack => {
            let mut line = pos.line;
            for _ in 0..count {
                line = paragraph_back(buffer, line);
            }
            Some((Position::new(line, 0), MotionKind::Exclusive))
        }
        Motion::MatchPair => {
            let chars = line_chars(buffer, pos.line);
            let config = BracketConfig::default();
            let start = chars
                .iter()
                .enumerate()
                .skip(pos.col)
                .find_map(|(col, c)| {
                    if "()[]{}".contains(*c) {
                        Some(col)
                    } else {
                        None
                    }
                })?;
            let target = matching_bracket_with(buffer, Position::new(pos.line, start), &config)?;
            Some((target, MotionKind::Inclusive))
        }
    }
}
