//! Bracket matching, auto-pairing, and unmatched-bracket diagnostics.
//!
//! Every scan is bounded: matching walks at most [`BracketConfig::max_scan_chars`]
//! characters (and the same number of lines) before giving up, so a pathological
//! file cannot stall a render.
//!
//! String and comment detection is per line. Strings that span lines — Rust raw
//! strings, Python triple quotes, JavaScript template literals broken over lines
//! — are not tracked, and brackets inside them are treated as code. That keeps
//! the classifier O(line) instead of O(file).
//!
//! All columns are character offsets.

use std::collections::HashMap;

use super::buffer::{Buffer, Position};
use super::highlight::Language;

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

const fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Returns the characters of a line without its trailing newline.
fn line_chars(buffer: &Buffer, line: usize) -> Option<Vec<char>> {
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
fn contexts_of(chars: &[char], language: Language) -> Vec<CharContext> {
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
fn scan_forward(
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

/// Reports every bracket in the buffer that has no partner, using default options.
#[must_use]
pub fn unmatched_brackets(buffer: &Buffer) -> Vec<Position> {
    unmatched_brackets_with(buffer, &BracketConfig::default())
}

/// Reports every bracket in the buffer that has no partner.
///
/// The scan stops after [`BracketConfig::max_scan_chars`] characters; anything
/// beyond that point is not reported.
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn rust() -> BracketConfig {
        BracketConfig::for_language(Language::Rust)
    }

    #[test]
    fn nested_brackets_match_outermost_first() {
        let buffer = Buffer::from_str("a(b[c]d)e");
        assert_eq!(
            matching_bracket(&buffer, Position::new(0, 1)),
            Some(Position::new(0, 7))
        );
        assert_eq!(
            matching_bracket(&buffer, Position::new(0, 3)),
            Some(Position::new(0, 5))
        );
        // And backwards from the closer.
        assert_eq!(
            matching_bracket(&buffer, Position::new(0, 7)),
            Some(Position::new(0, 1))
        );
    }

    #[test]
    fn matching_spans_lines() {
        let buffer = Buffer::from_str("fn f() {\n    if x {\n    }\n}\n");
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(0, 7), &rust()),
            Some(Position::new(3, 0))
        );
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(3, 0), &rust()),
            Some(Position::new(0, 7))
        );
    }

    #[test]
    fn unmatched_bracket_returns_none() {
        let buffer = Buffer::from_str("a(b\nc\n");
        assert_eq!(matching_bracket(&buffer, Position::new(0, 1)), None);
        // Not on a bracket at all.
        assert_eq!(matching_bracket(&buffer, Position::new(0, 0)), None);
    }

    #[test]
    fn brackets_inside_a_string_are_skipped() {
        let buffer = Buffer::from_str("f(\"a)b\")");
        assert_eq!(
            matching_bracket(&buffer, Position::new(0, 1)),
            Some(Position::new(0, 7))
        );
    }

    #[test]
    fn brackets_inside_a_line_comment_are_skipped() {
        let buffer = Buffer::from_str("f( // )\n)\n");
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(0, 1), &rust()),
            Some(Position::new(1, 0))
        );
    }

    #[test]
    fn a_bracket_inside_a_string_has_no_match() {
        let buffer = Buffer::from_str("x = \"(\"");
        assert_eq!(matching_bracket(&buffer, Position::new(0, 5)), None);
    }

    #[test]
    fn the_scan_cap_is_respected() {
        let mut text = String::from("(");
        text.push_str(&"a".repeat(50));
        text.push(')');
        let buffer = Buffer::from_str(&text);
        let tight = BracketConfig {
            max_scan_chars: 10,
            ..BracketConfig::default()
        };
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(0, 0), &tight),
            None
        );
        let roomy = BracketConfig {
            max_scan_chars: 1000,
            ..BracketConfig::default()
        };
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(0, 0), &roomy),
            Some(Position::new(0, 51))
        );
    }

    #[test]
    fn angle_brackets_only_match_when_enabled() {
        let buffer = Buffer::from_str("Vec<u8>");
        assert_eq!(matching_bracket(&buffer, Position::new(0, 3)), None);
        let cfg = BracketConfig {
            angle_brackets: true,
            ..BracketConfig::default()
        };
        assert_eq!(
            matching_bracket_with(&buffer, Position::new(0, 3), &cfg),
            Some(Position::new(0, 6))
        );
    }

    #[test]
    fn matching_uses_character_columns_on_non_ascii_lines() {
        let buffer = Buffer::from_str("é(ü)");
        assert_eq!(
            matching_bracket(&buffer, Position::new(0, 1)),
            Some(Position::new(0, 3))
        );
    }

    #[test]
    fn auto_close_is_suppressed_before_a_word_character() {
        let buffer = Buffer::from_str("abc");
        assert!(!should_auto_close(&buffer, Position::new(0, 0), '('));
        assert!(should_auto_close(&buffer, Position::new(0, 3), '('));
    }

    #[test]
    fn quote_auto_close_is_suppressed_after_a_word_character() {
        let buffer = Buffer::from_str("dont");
        assert!(!should_auto_close(&buffer, Position::new(0, 4), '\''));
        let buffer = Buffer::from_str("x = ");
        assert!(should_auto_close(&buffer, Position::new(0, 4), '\''));
    }

    #[test]
    fn unpaired_characters_never_auto_close() {
        let buffer = Buffer::from_str("");
        assert!(!should_auto_close(&buffer, Position::new(0, 0), 'x'));
        assert!(!should_auto_close(&buffer, Position::new(0, 0), '<'));
    }

    #[test]
    fn skip_over_an_existing_closer() {
        let buffer = Buffer::from_str("f()");
        assert!(should_skip_close(&buffer, Position::new(0, 2), ')'));
        assert!(!should_skip_close(&buffer, Position::new(0, 1), ')'));
        assert!(!should_skip_close(&buffer, Position::new(0, 3), ')'));
    }

    #[test]
    fn auto_pair_config_honours_its_flags() {
        let all = AutoPair::default();
        assert_eq!(all.close_for('('), Some(')'));
        assert_eq!(all.close_for('"'), Some('"'));
        assert_eq!(all.close_for('<'), None);

        let no_quotes = AutoPair {
            quotes: false,
            angle_brackets: true,
            ..AutoPair::default()
        };
        assert_eq!(no_quotes.close_for('"'), None);
        assert_eq!(no_quotes.close_for('<'), Some('>'));

        let off = AutoPair {
            enabled: false,
            ..AutoPair::default()
        };
        assert_eq!(off.close_for('('), None);
    }

    #[test]
    fn surround_wraps_a_selection() {
        let mut buffer = Buffer::from_str("hello world");
        let s = surround_selection(&buffer, Position::new(0, 6), Position::new(0, 11), '"')
            .expect("pairable");
        s.apply(&mut buffer);
        assert_eq!(buffer.text(), "hello \"world\"");
        buffer.undo();
        assert_eq!(buffer.text(), "hello world");
    }

    #[test]
    fn surround_rejects_unpairable_and_reversed_input() {
        let buffer = Buffer::from_str("hello");
        assert!(
            surround_selection(&buffer, Position::new(0, 0), Position::new(0, 3), 'x').is_none()
        );
        assert!(
            surround_selection(&buffer, Position::new(0, 3), Position::new(0, 0), '(').is_none()
        );
    }

    #[test]
    fn unmatched_brackets_reports_both_directions() {
        let buffer = Buffer::from_str("fn f() {\n  )\n  (\n");
        let bad = unmatched_brackets(&buffer);
        assert_eq!(
            bad,
            vec![
                Position::new(0, 7),
                Position::new(1, 2),
                Position::new(2, 2)
            ]
        );
    }

    #[test]
    fn balanced_text_reports_nothing() {
        let buffer = Buffer::from_str("fn f() { g([1, 2]); }\n");
        assert!(unmatched_brackets(&buffer).is_empty());
    }

    #[test]
    fn enclosing_pair_finds_the_innermost_span() {
        let buffer = Buffer::from_str("f(a, g(b), c)");
        assert_eq!(
            enclosing_pair(
                &buffer,
                Position::new(0, 7),
                '(',
                ')',
                &BracketConfig::default()
            ),
            Some((Position::new(0, 6), Position::new(0, 8)))
        );
        assert_eq!(
            enclosing_pair(
                &buffer,
                Position::new(0, 3),
                '(',
                ')',
                &BracketConfig::default()
            ),
            Some((Position::new(0, 1), Position::new(0, 12)))
        );
        assert_eq!(
            enclosing_pair(
                &buffer,
                Position::new(0, 0),
                '(',
                ')',
                &BracketConfig::default()
            ),
            None
        );
    }

    #[test]
    fn line_contexts_classifies_strings_and_comments() {
        let ctx = line_contexts("a \"b\" // c", Language::Rust);
        assert_eq!(ctx[0], CharContext::Code);
        assert_eq!(ctx[2], CharContext::StringLike);
        assert_eq!(ctx[4], CharContext::StringLike);
        assert_eq!(ctx[6], CharContext::Comment);
        assert_eq!(ctx[9], CharContext::Comment);
    }

    #[test]
    fn rust_lifetimes_are_not_strings_but_char_literals_are() {
        let lifetime = line_contexts("fn f<'a>(x: &'a str) {}", Language::Rust);
        assert!(lifetime.iter().all(|c| *c == CharContext::Code));
        let literal = line_contexts("let c = 'x';", Language::Rust);
        assert_eq!(literal[8], CharContext::StringLike);
        assert_eq!(literal[10], CharContext::StringLike);
        assert_eq!(literal[11], CharContext::Code);
    }

    #[test]
    fn python_single_quotes_are_strings() {
        let ctx = line_contexts("s = 'a)b'", Language::Python);
        assert_eq!(ctx[5], CharContext::StringLike);
        assert_eq!(ctx[6], CharContext::StringLike);
    }

    #[test]
    fn escaped_quotes_do_not_end_a_string() {
        let ctx = line_contexts(r#"a = "x\"y" + b"#, Language::Rust);
        assert_eq!(ctx[7], CharContext::StringLike);
        assert_eq!(ctx[9], CharContext::StringLike);
        assert_eq!(ctx[10], CharContext::Code);
    }
}
