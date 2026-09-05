//! Unit tests for the parent module.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::editor::language::Language;

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
fn highlight_pair_prefers_the_bracket_under_the_cursor() {
    let buffer = Buffer::from_str("f(a)b\n");
    let cfg = BracketConfig::default();
    assert_eq!(
        highlight_pair(&buffer, Position::new(0, 1), &cfg),
        Some((Position::new(0, 1), Position::new(0, 3)))
    );
    assert_eq!(
        highlight_pair(&buffer, Position::new(0, 3), &cfg),
        Some((Position::new(0, 1), Position::new(0, 3)))
    );
}

#[test]
fn highlight_pair_falls_back_to_the_bracket_just_before_the_cursor() {
    let buffer = Buffer::from_str("f(a)b\n");
    let cfg = BracketConfig::default();
    // The cursor sits after the ')', which is where insert mode leaves it.
    assert_eq!(
        highlight_pair(&buffer, Position::new(0, 4), &cfg),
        Some((Position::new(0, 1), Position::new(0, 3)))
    );
}

#[test]
fn highlight_pair_reports_nothing_away_from_brackets() {
    let buffer = Buffer::from_str("plain text\n");
    assert_eq!(
        highlight_pair(&buffer, Position::new(0, 4), &BracketConfig::default()),
        None
    );
    // An unmatched bracket is not a pair either.
    let buffer = Buffer::from_str("f(\n");
    assert_eq!(
        highlight_pair(&buffer, Position::new(0, 1), &BracketConfig::default()),
        None
    );
}
