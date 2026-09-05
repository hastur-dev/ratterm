//! Unit tests for the parent module.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::editor::buffer::Position;

fn kinds_on(hl: &Highlighter, buffer: &Buffer, line: usize) -> Vec<HighlightKind> {
    hl.highlight_line(buffer, line)
        .into_iter()
        .map(|s| s.kind)
        .collect()
}

fn highlighted(language: Language, text: &str) -> (Highlighter, Buffer) {
    let mut hl = Highlighter::new(language).expect("grammar loads");
    let buffer = Buffer::from_str(text);
    hl.parse(&buffer);
    (hl, buffer)
}

#[test]
fn rust_line_has_a_keyword_and_a_string() {
    let (hl, buffer) = highlighted(Language::Rust, "fn main() {\n    let s = \"hi\";\n}\n");
    assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
    assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
}

#[test]
fn python_line_has_a_keyword_and_a_string() {
    let (hl, buffer) = highlighted(Language::Python, "def f():\n    return \"hi\"\n");
    assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
    assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
}

#[test]
fn javascript_line_has_a_keyword_and_a_string() {
    let (hl, buffer) = highlighted(
        Language::JavaScript,
        "function f() {\n  return \"hi\";\n}\n",
    );
    assert!(kinds_on(&hl, &buffer, 0).contains(&HighlightKind::Keyword));
    assert!(kinds_on(&hl, &buffer, 1).contains(&HighlightKind::String));
}

#[test]
fn plain_text_produces_no_spans() {
    let (hl, buffer) = highlighted(Language::PlainText, "fn main() {}\n");
    assert!(!hl.has_tree());
    assert!(hl.highlight_line(&buffer, 0).is_empty());
}

#[test]
fn a_syntactically_broken_file_does_not_panic() {
    let (hl, buffer) = highlighted(Language::Rust, "fn ((( {{{ \"unterminated\nlet ] ) }\n");
    // Whatever the parser recovers, asking for spans must not panic and must
    // stay within each line.
    for line in 0..buffer.len_lines() {
        let len = buffer.line_len_chars(line);
        for span in hl.highlight_line(&buffer, line) {
            assert!(span.start_col < span.end_col);
            assert!(span.end_col <= len);
        }
    }
}

#[test]
fn spans_use_character_columns_on_a_non_ascii_line() {
    let text = "let é = \"wörld\";\n";
    let (hl, buffer) = highlighted(Language::Rust, text);
    let spans = hl.highlight_line(&buffer, 0);
    let string_span = spans
        .iter()
        .find(|s| s.kind == HighlightKind::String)
        .expect("string span");
    let chars: Vec<char> = text.trim_end_matches('\n').chars().collect();
    assert_eq!(chars[string_span.start_col], '"');
    // A byte-offset bug would land one column late, on the 'w'.
    assert_eq!(string_span.start_col, 8);
    assert!(string_span.end_col <= chars.len());
}

#[test]
fn out_of_range_lines_are_empty() {
    let (hl, buffer) = highlighted(Language::Rust, "fn main() {}\n");
    assert!(hl.highlight_line(&buffer, 999).is_empty());
}

#[test]
fn a_file_over_the_size_cap_is_not_parsed() {
    let mut hl = Highlighter::new(Language::Rust).expect("grammar loads");
    let huge = "fn f() {}\n".repeat(MAX_HIGHLIGHT_BYTES / 10 + 10);
    let buffer = Buffer::from_str(&huge);
    hl.parse(&buffer);
    assert!(!hl.has_tree());
    assert!(hl.highlight_line(&buffer, 0).is_empty());
}

#[test]
fn cache_returns_the_same_spans_after_an_unrelated_edit() {
    let text = "fn a() {}\n\n\n\n\nfn b() {}\n";
    let mut hl = Highlighter::new(Language::Rust).expect("grammar loads");
    let mut buffer = Buffer::from_str(text);
    hl.parse(&buffer);
    let before = hl.highlight_line(&buffer, 0);
    assert_eq!(hl.cached_line_count(), 1);

    let at = Position::new(5, 9);
    let edit = insertion_edit(&buffer, at, " // tail");
    buffer.insert_str(at, " // tail");
    hl.edit(&edit);
    assert_eq!(hl.cached_line_count(), 0, "edit drops the cache");
    hl.parse(&buffer);

    let after = hl.highlight_line(&buffer, 0);
    assert_eq!(before, after);
    assert!(
        hl.highlight_line(&buffer, 5)
            .iter()
            .any(|s| s.kind == HighlightKind::Comment)
    );
}

#[test]
fn an_incremental_reparse_matches_a_fresh_one() {
    let start = "fn main() {\n    let a = 1;\n}\n";
    let mut incremental = Highlighter::new(Language::Rust).expect("grammar loads");
    let mut buffer = Buffer::from_str(start);
    incremental.parse(&buffer);

    let at = Position::new(1, 12);
    let edit = insertion_edit(&buffer, at, "23 + \"s\"");
    buffer.insert_str(at, "23 + \"s\"");
    incremental.edit(&edit);
    incremental.parse(&buffer);

    let mut fresh = Highlighter::new(Language::Rust).expect("grammar loads");
    fresh.parse(&buffer);

    for line in 0..buffer.len_lines() {
        assert_eq!(
            incremental.highlight_line(&buffer, line),
            fresh.highlight_line(&buffer, line),
            "line {line}"
        );
    }
}

#[test]
fn invalidate_forces_a_parse_from_scratch() {
    let (mut hl, buffer) = highlighted(Language::Rust, "fn main() {}\n");
    assert!(hl.has_tree());
    hl.invalidate();
    assert!(!hl.has_tree());
    hl.parse(&buffer);
    assert!(hl.has_tree());
    assert!(!hl.highlight_line(&buffer, 0).is_empty());
}

#[test]
fn switching_language_drops_the_previous_tree() {
    let (mut hl, buffer) = highlighted(Language::Rust, "fn main() {}\n");
    assert!(hl.has_tree());
    hl.set_language(Language::PlainText).expect("plain text");
    assert!(!hl.has_tree());
    assert!(hl.highlight_line(&buffer, 0).is_empty());
}

#[test]
fn a_plain_highlighter_never_fails_and_never_paints() {
    let hl = Highlighter::plain();
    assert_eq!(hl.language(), Language::PlainText);
    assert!(!hl.has_tree());
    assert!(
        hl.highlight_line(&Buffer::from_str("fn f() {}"), 0)
            .is_empty()
    );
}

#[test]
fn byte_to_char_map_covers_multibyte_text() {
    let map = byte_to_char_map("aé b");
    assert_eq!(map[0], 0);
    assert_eq!(map[1], 1);
    assert_eq!(map[2], 1);
    assert_eq!(map[3], 2);
    assert_eq!(map[map.len() - 1], 4);
}

#[test]
fn for_path_picks_the_language_from_the_extension() {
    let hl = Highlighter::for_path(Path::new("x/y.py")).expect("python loads");
    assert_eq!(hl.language(), Language::Python);
    let hl = Highlighter::for_path(Path::new("x/y.unknown")).expect("plain loads");
    assert_eq!(hl.language(), Language::PlainText);
}
