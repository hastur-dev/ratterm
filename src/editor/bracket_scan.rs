//! Walking the buffer to pair brackets up.
//!
//! Split out of `brackets.rs` so neither file grows past the project's size
//! limit; [`brackets`](super::brackets) re-exports everything here.
//!
//! Every scan is bounded: matching walks at most
//! [`BracketConfig::max_scan_chars`](super::brackets::BracketConfig::max_scan_chars)
//! characters (and the same number of lines) before giving up, so a
//! pathological file cannot stall a render. All columns are character offsets.

use std::collections::HashMap;

use super::brackets::{BracketConfig, CharContext, contexts_of, line_chars};
use super::buffer::{Buffer, Position};

/// Finds the bracket matching the one under `pos`, using default options.
#[must_use]
pub fn matching_bracket(buffer: &Buffer, pos: Position) -> Option<Position> {
    matching_bracket_with(buffer, pos, &BracketConfig::default())
}

/// Finds the bracket matching the one under `pos`.
///
/// Returns `None` when `pos` is not on a bracket, when the bracket is inside a
/// string or comment, when no match exists, or when the scan budget runs out.
#[must_use]
pub fn matching_bracket_with(
    buffer: &Buffer,
    pos: Position,
    config: &BracketConfig,
) -> Option<Position> {
    let chars = line_chars(buffer, pos.line)?;
    let contexts = contexts_of(&chars, config.language);
    let ch = *chars.get(pos.col)?;
    if contexts.get(pos.col).copied()? != CharContext::Code {
        return None;
    }

    for (open, close) in config.pairs() {
        if ch == *open {
            return scan_forward(buffer, pos, *open, *close, config);
        }
        if ch == *close {
            return scan_backward(buffer, pos, *open, *close, config);
        }
    }
    None
}

/// Walks forward from an opening bracket to its partner.
pub(super) fn scan_forward(
    buffer: &Buffer,
    from: Position,
    open: char,
    close: char,
    config: &BracketConfig,
) -> Option<Position> {
    let mut budget = config.max_scan_chars;
    let mut depth = 0i32;
    let total = buffer.len_lines();
    let mut line = from.line;
    let mut col = from.col;

    while line < total {
        let chars = line_chars(buffer, line)?;
        let contexts = contexts_of(&chars, config.language);
        while col < chars.len() {
            if budget == 0 {
                return None;
            }
            budget -= 1;
            if contexts[col] == CharContext::Code {
                if chars[col] == open {
                    depth += 1;
                } else if chars[col] == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Position::new(line, col));
                    }
                }
            }
            col += 1;
        }
        if budget == 0 {
            return None;
        }
        budget -= 1;
        line += 1;
        col = 0;
    }
    None
}

/// Walks backward from a closing bracket to its partner.
fn scan_backward(
    buffer: &Buffer,
    from: Position,
    open: char,
    close: char,
    config: &BracketConfig,
) -> Option<Position> {
    let mut budget = config.max_scan_chars;
    let mut depth = 0i32;
    let mut line = from.line;
    let mut col = Some(from.col);

    loop {
        let chars = line_chars(buffer, line)?;
        let contexts = contexts_of(&chars, config.language);
        let mut c = col.unwrap_or_else(|| chars.len().saturating_sub(1));
        if !chars.is_empty() {
            loop {
                if budget == 0 {
                    return None;
                }
                budget -= 1;
                if c < chars.len() && contexts[c] == CharContext::Code {
                    if chars[c] == close {
                        depth += 1;
                    } else if chars[c] == open {
                        depth -= 1;
                        if depth == 0 {
                            return Some(Position::new(line, c));
                        }
                    }
                }
                if c == 0 {
                    break;
                }
                c -= 1;
            }
        }
        if line == 0 || budget == 0 {
            return None;
        }
        line -= 1;
        col = None;
    }
}

/// Finds the innermost `open`/`close` pair enclosing `pos`.
///
/// A bracket directly under the cursor counts as one end of the pair. The
/// returned positions are the bracket characters themselves.
#[must_use]
pub fn enclosing_pair(
    buffer: &Buffer,
    pos: Position,
    open: char,
    close: char,
    config: &BracketConfig,
) -> Option<(Position, Position)> {
    let chars = line_chars(buffer, pos.line)?;
    let here = chars.get(pos.col).copied();

    if here == Some(open) {
        let end = scan_forward(buffer, pos, open, close, config)?;
        return Some((pos, end));
    }
    if here == Some(close) {
        let start = scan_backward(buffer, pos, open, close, config)?;
        return Some((start, pos));
    }

    let start = unmatched_open_before(buffer, pos, open, close, config)?;
    let end = scan_forward(buffer, start, open, close, config)?;
    Some((start, end))
}

/// Walks backward for the nearest opener that is not already closed.
fn unmatched_open_before(
    buffer: &Buffer,
    pos: Position,
    open: char,
    close: char,
    config: &BracketConfig,
) -> Option<Position> {
    let mut budget = config.max_scan_chars;
    let mut depth = 0i32;
    let mut line = pos.line;
    let mut start_col = Some(pos.col);

    loop {
        let chars = line_chars(buffer, line)?;
        let contexts = contexts_of(&chars, config.language);
        let upper = start_col.unwrap_or(chars.len());
        let mut c = upper.min(chars.len());
        while c > 0 {
            c -= 1;
            if budget == 0 {
                return None;
            }
            budget -= 1;
            if contexts[c] != CharContext::Code {
                continue;
            }
            if chars[c] == close {
                depth += 1;
            } else if chars[c] == open {
                if depth == 0 {
                    return Some(Position::new(line, c));
                }
                depth -= 1;
            }
        }
        if line == 0 || budget == 0 {
            return None;
        }
        line -= 1;
        start_col = None;
    }
}

/// Reports every bracket in the buffer that has no partner, using default options.
#[must_use]
pub fn unmatched_brackets(buffer: &Buffer) -> Vec<Position> {
    unmatched_brackets_with(buffer, &BracketConfig::default())
}

/// Reports every bracket in the buffer that has no partner.
///
/// The scan stops after `max_scan_chars` characters; anything beyond that point
/// is not reported.
#[must_use]
pub fn unmatched_brackets_with(buffer: &Buffer, config: &BracketConfig) -> Vec<Position> {
    let pairs = config.pairs();
    let openers: HashMap<char, char> = pairs.iter().copied().collect();
    let closers: HashMap<char, char> = pairs.iter().map(|(o, c)| (*c, *o)).collect();

    let mut stack: Vec<(Position, char)> = Vec::new();
    let mut bad: Vec<Position> = Vec::new();
    let mut budget = config.max_scan_chars;

    'outer: for line in 0..buffer.len_lines() {
        let Some(chars) = line_chars(buffer, line) else {
            break;
        };
        let contexts = contexts_of(&chars, config.language);
        for (col, ch) in chars.iter().enumerate() {
            if budget == 0 {
                break 'outer;
            }
            budget -= 1;
            if contexts[col] != CharContext::Code {
                continue;
            }
            if openers.contains_key(ch) {
                stack.push((Position::new(line, col), *ch));
            } else if let Some(open) = closers.get(ch) {
                match stack.last() {
                    Some((_, top)) if top == open => {
                        stack.pop();
                    }
                    _ => bad.push(Position::new(line, col)),
                }
            }
        }
        if budget == 0 {
            break;
        }
        budget -= 1;
    }

    bad.extend(stack.into_iter().map(|(p, _)| p));
    bad.sort_by_key(|p| (p.line, p.col));
    bad
}

/// Returns the bracket pair to highlight for a cursor at `pos`.
///
/// The bracket under the cursor wins; failing that, the one immediately before
/// it is used, so a cursor sitting just past a closing brace still highlights
/// the pair. Returns the two positions in buffer order.
#[must_use]
pub fn highlight_pair(
    buffer: &Buffer,
    pos: Position,
    config: &BracketConfig,
) -> Option<(Position, Position)> {
    let mut candidates = vec![pos];
    if pos.col > 0 {
        candidates.push(Position::new(pos.line, pos.col - 1));
    }
    for candidate in candidates {
        if let Some(other) = matching_bracket_with(buffer, candidate, config) {
            let ordered = if (candidate.line, candidate.col) <= (other.line, other.col) {
                (candidate, other)
            } else {
                (other, candidate)
            };
            return Some(ordered);
        }
    }
    None
}

#[cfg(test)]
mod tests;
