//! Tests for Vim motion resolution.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::editor::buffer::{Buffer, Position};

use super::motion::{Motion, MotionKind, resolve_motion};

fn go(
    text: &str,
    pos: (usize, usize),
    motion: Motion,
    count: usize,
) -> Option<(Position, MotionKind)> {
    let buffer = Buffer::from_str(text);
    resolve_motion(&buffer, Position::new(pos.0, pos.1), &motion, count)
}

fn at(text: &str, pos: (usize, usize), motion: Motion, count: usize) -> Position {
    go(text, pos, motion, count).expect("motion resolves").0
}

#[test]
fn character_motions_clamp_to_the_line() {
    assert_eq!(at("abcdef\n", (0, 3), Motion::Left, 2), Position::new(0, 1));
    assert_eq!(
        at("abcdef\n", (0, 3), Motion::Left, 99),
        Position::new(0, 0)
    );
    assert_eq!(
        at("abcdef\n", (0, 3), Motion::Right, 2),
        Position::new(0, 5)
    );
    assert_eq!(
        at("abcdef\n", (0, 3), Motion::Right, 99),
        Position::new(0, 6)
    );
}

#[test]
fn vertical_motions_are_linewise_and_clamp() {
    let (pos, kind) = go("a\nbb\nccc\n", (0, 0), Motion::Down, 2).unwrap();
    assert_eq!(pos, Position::new(2, 0));
    assert_eq!(kind, MotionKind::Linewise);
    // The column is clamped to the shorter line's length.
    assert_eq!(
        at("a\nbb\nccc\n", (2, 2), Motion::Up, 99),
        Position::new(0, 1)
    );
    assert_eq!(
        at("a\nbb\nccc\n", (0, 0), Motion::Down, 99),
        Position::new(2, 0)
    );
}

#[test]
fn word_forward_counts_multiply_through() {
    let text = "one two three four\n";
    assert_eq!(
        at(text, (0, 0), Motion::WordForward, 1),
        Position::new(0, 4)
    );
    assert_eq!(
        at(text, (0, 0), Motion::WordForward, 3),
        Position::new(0, 14)
    );
}

#[test]
fn word_forward_treats_punctuation_as_its_own_word() {
    let text = "foo.bar baz\n";
    assert_eq!(
        at(text, (0, 0), Motion::WordForward, 1),
        Position::new(0, 3)
    );
    assert_eq!(
        at(text, (0, 3), Motion::WordForward, 1),
        Position::new(0, 4)
    );
    // The big variant steps over the whole token.
    assert_eq!(
        at(text, (0, 0), Motion::WordForwardBig, 1),
        Position::new(0, 8)
    );
}

#[test]
fn word_motions_cross_line_boundaries() {
    let text = "one\ntwo\n";
    assert_eq!(
        at(text, (0, 0), Motion::WordForward, 1),
        Position::new(1, 0)
    );
    assert_eq!(at(text, (1, 0), Motion::WordBack, 1), Position::new(0, 0));
}

#[test]
fn word_back_lands_on_the_word_start() {
    let text = "one two three\n";
    assert_eq!(at(text, (0, 10), Motion::WordBack, 1), Position::new(0, 8));
    assert_eq!(at(text, (0, 10), Motion::WordBack, 2), Position::new(0, 4));
    assert_eq!(at(text, (0, 0), Motion::WordBack, 1), Position::new(0, 0));
}

#[test]
fn word_end_is_inclusive_and_lands_on_the_last_character() {
    let text = "one two\n";
    let (pos, kind) = go(text, (0, 0), Motion::WordEnd, 1).unwrap();
    assert_eq!(pos, Position::new(0, 2));
    assert_eq!(kind, MotionKind::Inclusive);
    assert_eq!(at(text, (0, 0), Motion::WordEnd, 2), Position::new(0, 6));
    assert_eq!(
        at("a.b c\n", (0, 0), Motion::WordEndBig, 1),
        Position::new(0, 2)
    );
}

#[test]
fn line_anchors_resolve() {
    let text = "    hello world\n";
    assert_eq!(at(text, (0, 7), Motion::LineStart, 1), Position::new(0, 0));
    assert_eq!(
        at(text, (0, 7), Motion::FirstNonBlank, 1),
        Position::new(0, 4)
    );
    let (pos, kind) = go(text, (0, 0), Motion::LineEnd, 1).unwrap();
    assert_eq!(pos, Position::new(0, 14));
    assert_eq!(kind, MotionKind::Inclusive);
}

#[test]
fn line_end_with_a_count_moves_down_first() {
    assert_eq!(
        at("ab\ncdef\n", (0, 0), Motion::LineEnd, 2),
        Position::new(1, 3)
    );
}

#[test]
fn file_anchors_ignore_the_trailing_empty_line() {
    let text = "  first\nsecond\nthird\n";
    assert_eq!(at(text, (2, 0), Motion::FileStart, 1), Position::new(0, 2));
    assert_eq!(at(text, (0, 0), Motion::FileEnd, 1), Position::new(2, 0));
    assert_eq!(
        at(text, (0, 0), Motion::GotoLine(2), 1),
        Position::new(1, 0)
    );
    assert_eq!(
        at(text, (0, 0), Motion::GotoLine(999), 1),
        Position::new(2, 0)
    );
}

#[test]
fn find_and_till_search_the_current_line() {
    let text = "a-b-c-d\n";
    assert_eq!(
        at(text, (0, 0), Motion::FindForward('-'), 1),
        Position::new(0, 1)
    );
    assert_eq!(
        at(text, (0, 0), Motion::FindForward('-'), 2),
        Position::new(0, 3)
    );
    assert_eq!(
        at(text, (0, 0), Motion::TillForward('-'), 2),
        Position::new(0, 2)
    );
    assert_eq!(
        at(text, (0, 6), Motion::FindBackward('-'), 1),
        Position::new(0, 5)
    );
    assert_eq!(
        at(text, (0, 6), Motion::TillBackward('-'), 1),
        Position::new(0, 6)
    );
}

#[test]
fn a_find_with_no_match_fails() {
    assert!(go("abc\n", (0, 0), Motion::FindForward('z'), 1).is_none());
    assert!(go("abc\n", (0, 0), Motion::FindForward('a'), 1).is_none());
    assert!(go("a-b\n", (0, 0), Motion::FindForward('-'), 5).is_none());
}

#[test]
fn paragraph_motions_stop_at_blank_lines() {
    let text = "one\ntwo\n\nthree\nfour\n\nfive\n";
    assert_eq!(
        at(text, (0, 0), Motion::ParagraphForward, 1),
        Position::new(2, 0)
    );
    assert_eq!(
        at(text, (0, 0), Motion::ParagraphForward, 2),
        Position::new(5, 0)
    );
    assert_eq!(
        at(text, (4, 0), Motion::ParagraphBack, 1),
        Position::new(2, 0)
    );
    assert_eq!(
        at(text, (0, 0), Motion::ParagraphBack, 1),
        Position::new(0, 0)
    );
}

#[test]
fn match_pair_jumps_to_the_partner() {
    let text = "if (a[0]) {\n}\n";
    assert_eq!(at(text, (0, 3), Motion::MatchPair, 1), Position::new(0, 8));
    // From a non-bracket column it finds the next bracket on the line first.
    assert_eq!(at(text, (0, 0), Motion::MatchPair, 1), Position::new(0, 8));
    assert!(go("no brackets\n", (0, 0), Motion::MatchPair, 1).is_none());
}

#[test]
fn motions_on_an_empty_buffer_do_not_panic() {
    let buffer = Buffer::new();
    for motion in [
        Motion::WordForward,
        Motion::WordBack,
        Motion::WordEnd,
        Motion::LineEnd,
        Motion::FileEnd,
        Motion::ParagraphForward,
        Motion::ParagraphBack,
    ] {
        let got = resolve_motion(&buffer, Position::new(0, 0), &motion, 1);
        assert!(got.is_none() || got == Some((Position::new(0, 0), kind_of(&motion))));
    }
}

fn kind_of(motion: &Motion) -> MotionKind {
    match motion {
        Motion::WordEnd | Motion::LineEnd => MotionKind::Inclusive,
        Motion::FileEnd => MotionKind::Linewise,
        _ => MotionKind::Exclusive,
    }
}

#[test]
fn word_motions_use_character_columns_on_non_ascii_lines() {
    let text = "héllo wörld\n";
    assert_eq!(
        at(text, (0, 0), Motion::WordForward, 1),
        Position::new(0, 6)
    );
    assert_eq!(at(text, (0, 6), Motion::WordBack, 1), Position::new(0, 0));
}
