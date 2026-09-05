//! Bracket matching, auto-pairing, and unmatched-bracket diagnostics.
//!
//! This file owns the classification of a line into code, string and comment
//! plus the typing-time rules; the scanning that pairs brackets up lives in
//! [`bracket_scan`](super::bracket_scan) and is re-exported here, so callers
//! keep using `crate::editor::brackets::*`.
//!
//! String and comment detection is per line. Strings that span lines — Rust raw
//! strings, Python triple quotes, JavaScript template literals broken over lines
//! — are not tracked, and brackets inside them are treated as code. That keeps
//! the classifier O(line) instead of O(file).
//!
//! All columns are character offsets.

use super::buffer::{Buffer, Position};
use super::language::Language;

pub use super::bracket_scan::{
    enclosing_pair, highlight_pair, matching_bracket, matching_bracket_with, unmatched_brackets,
    unmatched_brackets_with,
};

/// Default cap on how far a bracket scan will travel.
pub const MAX_BRACKET_SCAN_CHARS: usize = 20_000;

/// What a character on a line is part of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharContext {
    /// Ordinary code.
    Code,
    /// Inside a string or character literal, including its delimiters.
    StringLike,
    /// Inside a line comment, including its marker.
    Comment,
}

/// Options for bracket matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketConfig {
    /// Treat `<` and `>` as a pair. Off by default because they are far more
    /// often comparison operators than brackets.
    pub angle_brackets: bool,
    /// Maximum characters, and maximum lines, a single scan may cover.
    pub max_scan_chars: usize,
    /// Language used to classify strings and comments.
    pub language: Language,
}

impl Default for BracketConfig {
    fn default() -> Self {
        Self {
            angle_brackets: false,
            max_scan_chars: MAX_BRACKET_SCAN_CHARS,
            language: Language::PlainText,
        }
    }
}

impl BracketConfig {
    /// Returns a config for `language` with the defaults otherwise.
    #[must_use]
    pub fn for_language(language: Language) -> Self {
        Self {
            language,
            ..Self::default()
        }
    }

    /// Returns the bracket pairs this config matches.
    #[must_use]
    pub const fn pairs(&self) -> &'static [(char, char)] {
        if self.angle_brackets {
            &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')]
        } else {
            &[('(', ')'), ('[', ']'), ('{', '}')]
        }
    }
}

/// Which characters to pair automatically when typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPair {
    /// Master switch.
    pub enabled: bool,
    /// Pair quote characters as well as brackets.
    pub quotes: bool,
    /// Pair `<` with `>`.
    pub angle_brackets: bool,
}

impl Default for AutoPair {
    fn default() -> Self {
        Self {
            enabled: true,
            quotes: true,
            angle_brackets: false,
        }
    }
}

impl AutoPair {
    /// Returns the closing character for `opening`, honouring the flags.
    #[must_use]
    pub fn close_for(&self, opening: char) -> Option<char> {
        if !self.enabled {
            return None;
        }
        match opening {
            '<' if self.angle_brackets => Some('>'),
            '"' | '\'' | '`' if self.quotes => auto_pair_for(opening),
            _ if is_quote(opening) => None,
            _ => auto_pair_for(opening),
        }
    }
}

/// Returns the closing character that pairs with `opening`.
///
/// `<` is deliberately absent: it needs the surrounding context to tell a
/// generic parameter from a comparison. Use [`AutoPair::close_for`] with
/// `angle_brackets` set when the caller has that context.
#[must_use]
pub const fn auto_pair_for(opening: char) -> Option<char> {
    match opening {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Returns the opening character that pairs with `closing`.
#[must_use]
pub const fn opening_for(closing: char) -> Option<char> {
    match closing {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

const fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Returns the characters of a line without its trailing newline.
pub(super) fn line_chars(buffer: &Buffer, line: usize) -> Option<Vec<char>> {
    let raw = buffer.line(line)?;
    let text = raw.strip_suffix('\n').unwrap_or(&raw);
    Some(text.chars().collect())
}

fn matches_at(chars: &[char], at: usize, pattern: &[char]) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(k, p)| chars.get(at + k) == Some(p))
}

/// Returns the index of the quote closing the one at `start`.
///
/// An unterminated quote reports the last character on the line, so the rest of
/// the line counts as string content.
fn scan_quoted(chars: &[char], start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            return i;
        }
        i += 1;
    }
    chars.len().saturating_sub(1).max(start)
}

/// Decides whether `'` at `at` opens a string rather than being an apostrophe
/// or a Rust lifetime.
fn single_quote_opens_string(language: Language, chars: &[char], at: usize) -> bool {
    if language.single_quote_is_string() {
        return true;
    }
    if language != Language::Rust {
        return false;
    }
    // `'a'` and `'\n'` are char literals; `'a` on its own is a lifetime.
    if chars.get(at + 1) == Some(&'\\') {
        return chars[at + 2..].iter().take(4).any(|c| *c == '\'');
    }
    chars.get(at + 2) == Some(&'\'')
}

/// Classifies every character on a line as code, string, or comment.
#[must_use]
pub fn line_contexts(line: &str, language: Language) -> Vec<CharContext> {
    let chars: Vec<char> = line.chars().collect();
    contexts_of(&chars, language)
}

/// [`line_contexts`] over a line already split into characters.
pub(super) fn contexts_of(chars: &[char], language: Language) -> Vec<CharContext> {
    let mut out = vec![CharContext::Code; chars.len()];
    let comment: Vec<char> = language.line_comment().unwrap_or("").chars().collect();

    let mut i = 0usize;
    while i < chars.len() {
        if !comment.is_empty() && matches_at(chars, i, &comment) {
            for slot in &mut out[i..] {
                *slot = CharContext::Comment;
            }
            break;
        }
        let c = chars[i];
        let opens_string = c == '"'
            || (c == '`' && language == Language::JavaScript)
            || (c == '\'' && single_quote_opens_string(language, chars, i));
        if opens_string {
            let end = scan_quoted(chars, i, c);
            for slot in out.iter_mut().take(end + 1).skip(i) {
                *slot = CharContext::StringLike;
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// Returns true when typing `opening` at `pos` should also insert its partner.
///
/// Auto-closing is suppressed directly before a word character, and quotes are
/// also suppressed directly after one so that an apostrophe in prose does not
/// grow a partner.
#[must_use]
pub fn should_auto_close(buffer: &Buffer, pos: Position, opening: char) -> bool {
    if auto_pair_for(opening).is_none() {
        return false;
    }
    let Some(chars) = line_chars(buffer, pos.line) else {
        return false;
    };
    if let Some(next) = chars.get(pos.col)
        && (is_word_char(*next) || *next == opening)
    {
        return false;
    }
    if is_quote(opening)
        && pos.col > 0
        && let Some(prev) = chars.get(pos.col - 1)
        && (is_word_char(*prev) || *prev == opening)
    {
        return false;
    }
    true
}

/// Returns true when typing `closing` at `pos` should step over the existing
/// character instead of inserting another one.
#[must_use]
pub fn should_skip_close(buffer: &Buffer, pos: Position, closing: char) -> bool {
    let Some(chars) = line_chars(buffer, pos.line) else {
        return false;
    };
    chars.get(pos.col) == Some(&closing)
}

/// What typing one character should do, once auto-pairing has had its say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeAction {
    /// Insert the character on its own.
    Insert(char),
    /// Insert the character and its partner, leaving the cursor between them.
    InsertPair(char, char),
    /// Step over the character already there instead of inserting another.
    StepOver,
}

/// Decides what typing `c` at `pos` should do.
///
/// This is the whole auto-pair policy in one place: the input layer calls it
/// and applies the answer, so the rules can be tested without an editor.
#[must_use]
pub fn type_action(buffer: &Buffer, pos: Position, c: char, pairs: AutoPair) -> TypeAction {
    if !pairs.enabled {
        return TypeAction::Insert(c);
    }
    // A closing character typed onto its own auto-inserted partner steps over.
    if opening_for(c).is_some() && should_skip_close(buffer, pos, c) {
        return TypeAction::StepOver;
    }
    if let Some(close) = pairs.close_for(c) {
        // A quote already sitting under the cursor closes the pair instead of
        // opening a new one.
        if is_quote(c) && should_skip_close(buffer, pos, c) {
            return TypeAction::StepOver;
        }
        if should_auto_close(buffer, pos, c) {
            return TypeAction::InsertPair(c, close);
        }
    }
    TypeAction::Insert(c)
}

/// The two insertions that wrap a range in a bracket or quote pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surround {
    /// Where the opening character goes.
    pub open_at: Position,
    /// The opening character.
    pub open: char,
    /// Where the closing character goes, in coordinates of the unmodified buffer.
    pub close_at: Position,
    /// The closing character.
    pub close: char,
}

impl Surround {
    /// Applies both insertions as one undo step.
    ///
    /// The closing character is inserted first so the opening insertion cannot
    /// shift it.
    pub fn apply(&self, buffer: &mut Buffer) {
        buffer.begin_undo_group();
        buffer.insert_char(self.close_at, self.close);
        buffer.insert_char(self.open_at, self.open);
        buffer.end_undo_group();
    }
}

/// Describes wrapping the range `start..end` in the pair opened by `opening`.
///
/// Returns `None` if `opening` has no partner or the range is reversed.
#[must_use]
pub fn surround_selection(
    buffer: &Buffer,
    start: Position,
    end: Position,
    opening: char,
) -> Option<Surround> {
    let close = auto_pair_for(opening)?;
    if buffer.position_to_index(start) > buffer.position_to_index(end) {
        return None;
    }
    Some(Surround {
        open_at: start,
        open: opening,
        close_at: end,
        close,
    })
}

#[cfg(test)]
mod tests;
