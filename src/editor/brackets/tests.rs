//! Unit tests for the parent module.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;

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
fn opening_for_inverts_auto_pair_for_brackets() {
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        assert_eq!(auto_pair_for(open), Some(close));
        assert_eq!(opening_for(close), Some(open));
    }
    assert_eq!(opening_for('"'), None);
    assert_eq!(opening_for('x'), None);
}

#[test]
fn typing_an_opener_on_empty_space_inserts_the_pair() {
    let buffer = Buffer::from_str("let a = \n");
    assert_eq!(
        type_action(&buffer, Position::new(0, 8), '(', AutoPair::default()),
        TypeAction::InsertPair('(', ')')
    );
}

#[test]
fn typing_a_closer_over_its_partner_steps_over_it() {
    let buffer = Buffer::from_str("f()\n");
    assert_eq!(
        type_action(&buffer, Position::new(0, 2), ')', AutoPair::default()),
        TypeAction::StepOver
    );
    // A quote behaves the same way.
    let buffer = Buffer::from_str("\"\"\n");
    assert_eq!(
        type_action(&buffer, Position::new(0, 1), '"', AutoPair::default()),
        TypeAction::StepOver
    );
}

#[test]
fn typing_before_a_word_inserts_only_the_character() {
    let buffer = Buffer::from_str("abc\n");
    assert_eq!(
        type_action(&buffer, Position::new(0, 0), '(', AutoPair::default()),
        TypeAction::Insert('(')
    );
    // A plain letter is never paired.
    assert_eq!(
        type_action(&buffer, Position::new(0, 3), 'z', AutoPair::default()),
        TypeAction::Insert('z')
    );
}

#[test]
fn auto_pairing_switched_off_always_inserts_plainly() {
    let buffer = Buffer::from_str("f()\n");
    let off = AutoPair {
        enabled: false,
        ..AutoPair::default()
    };
    assert_eq!(
        type_action(&buffer, Position::new(0, 1), '(', off),
        TypeAction::Insert('(')
    );
    assert_eq!(
        type_action(&buffer, Position::new(0, 2), ')', off),
        TypeAction::Insert(')')
    );
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
    assert!(surround_selection(&buffer, Position::new(0, 0), Position::new(0, 3), 'x').is_none());
    assert!(surround_selection(&buffer, Position::new(0, 3), Position::new(0, 0), '(').is_none());
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
