//! Vim text objects (`iw`, `a"`, `i(`, `ap`, and friends).
//!
//! [`resolve_text_object`] returns a half-open range: the start position is
//! included and the end position is not, which is the same convention
//! [`Buffer::delete_range`] uses.
//!
//! Word and quote objects are looked for on the cursor's line. Bracket and
//! paragraph objects may span lines.

use crate::editor::brackets::{BracketConfig, enclosing_pair};
use crate::editor::buffer::{Buffer, Position};

use super::motion::{Class, class, last_line};

/// What kind of region a text object selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    /// `w` — a run of one character class.
    Word,
    /// `W` — a run of non-whitespace.
    BigWord,
    /// `"`, `'`, or `` ` `` — a quoted span on the current line.
    Quote(char),
    /// `(` or `b`
    Paren,
    /// `[`
    Bracket,
    /// `{` or `B`
    Brace,
    /// `<`
    AngleBracket,
    /// `p` — a run of non-blank lines.
    Paragraph,
}

impl TextObjectKind {
    /// Returns the bracket pair this kind delimits, if it is a bracket kind.
    #[must_use]
    pub const fn bracket_pair(self) -> Option<(char, char)> {
        match self {
            Self::Paren => Some(('(', ')')),
            Self::Bracket => Some(('[', ']')),
            Self::Brace => Some(('{', '}')),
            Self::AngleBracket => Some(('<', '>')),
            _ => None,
        }
    }

    /// Maps the key that names this object, as typed after `i` or `a`.
    #[must_use]
    pub fn from_key(key: char) -> Option<Self> {
        match key {
            'w' => Some(Self::Word),
            'W' => Some(Self::BigWord),
            '"' | '\'' | '`' => Some(Self::Quote(key)),
            '(' | ')' | 'b' => Some(Self::Paren),
            '[' | ']' => Some(Self::Bracket),
            '{' | '}' | 'B' => Some(Self::Brace),
            '<' | '>' => Some(Self::AngleBracket),
            'p' => Some(Self::Paragraph),
            _ => None,
        }
    }
}

/// A text object: a kind plus whether the delimiters or trailing space count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObject {
    /// What is being selected.
    pub kind: TextObjectKind,
    /// `true` for `a...`, `false` for `i...`.
    pub around: bool,
}

impl TextObject {
    /// Builds an `i` object.
    #[must_use]
    pub const fn inner(kind: TextObjectKind) -> Self {
        Self {
            kind,
            around: false,
        }
    }

    /// Builds an `a` object.
    #[must_use]
    pub const fn around(kind: TextObjectKind) -> Self {
        Self { kind, around: true }
    }
}

fn line_chars(buffer: &Buffer, line: usize) -> Vec<char> {
    buffer.line(line).map_or_else(Vec::new, |raw| {
        raw.strip_suffix('\n').unwrap_or(&raw).chars().collect()
    })
}

/// Returns the half-open range a text object covers.
///
/// Returns `None` when the object does not exist at `pos` — no quoted span on
/// the line, no enclosing bracket, or an empty line for a word object.
#[must_use]
pub fn resolve_text_object(
    buffer: &Buffer,
    pos: Position,
    object: &TextObject,
) -> Option<(Position, Position)> {
    match object.kind {
        TextObjectKind::Word => word_object(buffer, pos, object.around, false),
        TextObjectKind::BigWord => word_object(buffer, pos, object.around, true),
        TextObjectKind::Quote(q) => quote_object(buffer, pos, q, object.around),
        TextObjectKind::Paragraph => paragraph_object(buffer, pos, object.around),
        kind => {
            let (open, close) = kind.bracket_pair()?;
            bracket_object(buffer, pos, open, close, object.around)
        }
    }
}

fn word_object(
    buffer: &Buffer,
    pos: Position,
    around: bool,
    big: bool,
) -> Option<(Position, Position)> {
    let chars = line_chars(buffer, pos.line);
    if chars.is_empty() {
        return None;
    }
    let col = pos.col.min(chars.len() - 1);
    let same = |a: char, b: char| {
        if big {
            (class(a) == Class::Blank) == (class(b) == Class::Blank)
        } else {
            class(a) == class(b)
        }
    };
    let anchor = chars[col];

    let mut start = col;
    while start > 0 && same(chars[start - 1], anchor) {
        start -= 1;
    }
    let mut end = col + 1;
    while end < chars.len() && same(chars[end], anchor) {
        end += 1;
    }

    if around {
        let before = end;
        while end < chars.len() && class(chars[end]) == Class::Blank {
            end += 1;
        }
        if end == before {
            while start > 0 && class(chars[start - 1]) == Class::Blank {
                start -= 1;
            }
        }
    }

    Some((Position::new(pos.line, start), Position::new(pos.line, end)))
}

/// Returns the columns of unescaped `quote` characters on a line.
fn quote_columns(chars: &[char], quote: char) -> Vec<usize> {
    let mut columns = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            columns.push(i);
        }
        i += 1;
    }
    columns
}

fn quote_object(
    buffer: &Buffer,
    pos: Position,
    quote: char,
    around: bool,
) -> Option<(Position, Position)> {
    let chars = line_chars(buffer, pos.line);
    let columns = quote_columns(&chars, quote);
    // Quotes pair up from the start of the line; take the first pair whose
    // closing quote is at or after the cursor.
    let mut pair = None;
    let mut i = 0usize;
    while i + 1 < columns.len() {
        if columns[i + 1] >= pos.col {
            pair = Some((columns[i], columns[i + 1]));
            break;
        }
        i += 2;
    }
    let (open, close) = pair?;
    let (start, end) = if around {
        (open, close + 1)
    } else {
        (open + 1, close)
    };
    Some((Position::new(pos.line, start), Position::new(pos.line, end)))
}

fn bracket_object(
    buffer: &Buffer,
    pos: Position,
    open: char,
    close: char,
    around: bool,
) -> Option<(Position, Position)> {
    let config = BracketConfig {
        angle_brackets: open == '<',
        ..BracketConfig::default()
    };
    let (start, end) = enclosing_pair(buffer, pos, open, close, &config)?;
    if around {
        Some((start, Position::new(end.line, end.col + 1)))
    } else {
        Some((Position::new(start.line, start.col + 1), end))
    }
}

fn is_blank_line(buffer: &Buffer, line: usize) -> bool {
    buffer.line(line).is_none_or(|raw| raw.trim().is_empty())
}

/// Returns the half-open range covering whole lines `first` through `last`.
fn line_span(buffer: &Buffer, first: usize, last: usize) -> (Position, Position) {
    let end_of_buffer = last_line(buffer);
    let end = if last < end_of_buffer {
        Position::new(last + 1, 0)
    } else {
        Position::new(last, buffer.line_len_chars(last))
    };
    (Position::new(first, 0), end)
}

fn paragraph_object(buffer: &Buffer, pos: Position, around: bool) -> Option<(Position, Position)> {
    let end_of_buffer = last_line(buffer);
    let line = pos.line.min(end_of_buffer);
    let blank = is_blank_line(buffer, line);

    let mut first = line;
    while first > 0 && is_blank_line(buffer, first - 1) == blank {
        first -= 1;
    }
    let mut last = line;
    while last < end_of_buffer && is_blank_line(buffer, last + 1) == blank {
        last += 1;
    }

    if around {
        let before = last;
        while last < end_of_buffer && is_blank_line(buffer, last + 1) != blank {
            last += 1;
        }
        if last == before {
            while first > 0 && is_blank_line(buffer, first - 1) != blank {
                first -= 1;
            }
        }
    }

    Some(line_span(buffer, first, last))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn span(text: &str, pos: (usize, usize), object: TextObject) -> Option<String> {
        let buffer = Buffer::from_str(text);
        let (start, end) = resolve_text_object(&buffer, Position::new(pos.0, pos.1), &object)?;
        Some(buffer.get_range(start, end).unwrap_or_default())
    }

    #[test]
    fn inner_word_selects_the_run_under_the_cursor() {
        let text = "one two three\n";
        assert_eq!(
            span(text, (0, 5), TextObject::inner(TextObjectKind::Word)),
            Some("two".to_string())
        );
        assert_eq!(
            span(text, (0, 3), TextObject::inner(TextObjectKind::Word)),
            Some(" ".to_string())
        );
    }

    #[test]
    fn around_word_takes_the_trailing_space() {
        let text = "one two three\n";
        assert_eq!(
            span(text, (0, 4), TextObject::around(TextObjectKind::Word)),
            Some("two ".to_string())
        );
        // With no trailing space it takes the leading one instead.
        assert_eq!(
            span(text, (0, 10), TextObject::around(TextObjectKind::Word)),
            Some(" three".to_string())
        );
    }

    #[test]
    fn word_objects_split_on_punctuation_and_big_words_do_not() {
        let text = "foo.bar baz\n";
        assert_eq!(
            span(text, (0, 1), TextObject::inner(TextObjectKind::Word)),
            Some("foo".to_string())
        );
        assert_eq!(
            span(text, (0, 1), TextObject::inner(TextObjectKind::BigWord)),
            Some("foo.bar".to_string())
        );
    }

    #[test]
    fn a_word_object_on_an_empty_line_fails() {
        assert_eq!(
            span("\n\n", (0, 0), TextObject::inner(TextObjectKind::Word)),
            None
        );
    }

    #[test]
    fn inner_and_around_quotes() {
        let text = "let s = \"hello\";\n";
        assert_eq!(
            span(text, (0, 10), TextObject::inner(TextObjectKind::Quote('"'))),
            Some("hello".to_string())
        );
        assert_eq!(
            span(
                text,
                (0, 10),
                TextObject::around(TextObjectKind::Quote('"'))
            ),
            Some("\"hello\"".to_string())
        );
        // The cursor before the span still finds the following pair.
        assert_eq!(
            span(text, (0, 0), TextObject::inner(TextObjectKind::Quote('"'))),
            Some("hello".to_string())
        );
    }

    #[test]
    fn a_quote_object_with_no_pair_fails() {
        assert_eq!(
            span(
                "no quotes here\n",
                (0, 3),
                TextObject::inner(TextObjectKind::Quote('"'))
            ),
            None
        );
        assert_eq!(
            span(
                "one \" only\n",
                (0, 0),
                TextObject::inner(TextObjectKind::Quote('"'))
            ),
            None
        );
    }

    #[test]
    fn escaped_quotes_do_not_split_the_pair() {
        let text = r#"s = "a\"b";"#;
        assert_eq!(
            span(text, (0, 6), TextObject::inner(TextObjectKind::Quote('"'))),
            Some(r#"a\"b"#.to_string())
        );
    }

    #[test]
    fn inner_and_around_parentheses() {
        let text = "call(a, b)\n";
        assert_eq!(
            span(text, (0, 6), TextObject::inner(TextObjectKind::Paren)),
            Some("a, b".to_string())
        );
        assert_eq!(
            span(text, (0, 6), TextObject::around(TextObjectKind::Paren)),
            Some("(a, b)".to_string())
        );
    }

    #[test]
    fn bracket_objects_span_lines() {
        let text = "fn f() {\n    body();\n}\n";
        assert_eq!(
            span(text, (1, 4), TextObject::inner(TextObjectKind::Brace)),
            Some("\n    body();\n".to_string())
        );
        assert_eq!(
            span(text, (1, 4), TextObject::around(TextObjectKind::Brace)),
            Some("{\n    body();\n}".to_string())
        );
    }

    #[test]
    fn angle_brackets_resolve_when_asked_for() {
        assert_eq!(
            span(
                "Vec<u8>\n",
                (0, 5),
                TextObject::inner(TextObjectKind::AngleBracket)
            ),
            Some("u8".to_string())
        );
    }

    #[test]
    fn a_bracket_object_with_no_enclosing_pair_fails() {
        assert_eq!(
            span(
                "plain text\n",
                (0, 3),
                TextObject::inner(TextObjectKind::Paren)
            ),
            None
        );
    }

    #[test]
    fn inner_paragraph_covers_the_block_of_non_blank_lines() {
        let text = "one\ntwo\n\nthree\n";
        assert_eq!(
            span(text, (0, 0), TextObject::inner(TextObjectKind::Paragraph)),
            Some("one\ntwo\n".to_string())
        );
        assert_eq!(
            span(text, (1, 0), TextObject::around(TextObjectKind::Paragraph)),
            Some("one\ntwo\n\n".to_string())
        );
    }

    #[test]
    fn object_keys_map_to_kinds() {
        assert_eq!(TextObjectKind::from_key('w'), Some(TextObjectKind::Word));
        assert_eq!(TextObjectKind::from_key('B'), Some(TextObjectKind::Brace));
        assert_eq!(TextObjectKind::from_key(')'), Some(TextObjectKind::Paren));
        assert_eq!(
            TextObjectKind::from_key('"'),
            Some(TextObjectKind::Quote('"'))
        );
        assert_eq!(TextObjectKind::from_key('z'), None);
        assert_eq!(TextObjectKind::Word.bracket_pair(), None);
        assert_eq!(TextObjectKind::Brace.bracket_pair(), Some(('{', '}')));
    }
}
