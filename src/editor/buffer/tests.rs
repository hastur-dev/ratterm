//! Unit tests for [`Buffer`](super::Buffer).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn test_buffer_new() {
    let buffer = Buffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len_lines(), 1);
}

#[test]
fn test_buffer_from_str() {
    let buffer = Buffer::from_str("Hello\nWorld");
    assert_eq!(buffer.len_lines(), 2);
    assert_eq!(buffer.line(0), Some("Hello\n".to_string()));
    assert_eq!(buffer.line(1), Some("World".to_string()));
}

#[test]
fn test_buffer_insert() {
    let mut buffer = Buffer::from_str("Hello");
    buffer.insert_char(Position::new(0, 5), '!');
    assert_eq!(buffer.text(), "Hello!");
}

#[test]
fn test_buffer_undo() {
    let mut buffer = Buffer::from_str("Hello");
    buffer.insert_char(Position::new(0, 5), '!');
    buffer.undo();
    assert_eq!(buffer.text(), "Hello");
}

#[test]
fn test_buffer_redo() {
    let mut buffer = Buffer::from_str("Hello");
    buffer.insert_char(Position::new(0, 5), '!');
    buffer.undo();
    buffer.redo();
    assert_eq!(buffer.text(), "Hello!");
}

#[test]
fn undo_of_multibyte_insert_restores_exactly() {
    // Undo replays the inverse edit against character indices. Using the
    // byte length of the recorded text removed too many characters.
    let mut buffer = Buffer::from_str("aé");
    buffer.insert_str(Position::new(0, 2), "ü");
    assert_eq!(buffer.text(), "aéü");
    buffer.undo();
    assert_eq!(buffer.text(), "aé");
}

#[test]
fn undo_of_multibyte_delete_restores_exactly() {
    let mut buffer = Buffer::from_str("αβγ");
    buffer.delete_range(Position::new(0, 0), Position::new(0, 2));
    assert_eq!(buffer.text(), "γ");
    buffer.undo();
    assert_eq!(buffer.text(), "αβγ");
    buffer.redo();
    assert_eq!(buffer.text(), "γ");
}

#[test]
fn replace_all_handles_multibyte_patterns() {
    let mut buffer = Buffer::from_str("naïve naïve");
    let count = buffer.replace_all("naïve", "plain");
    assert_eq!(count, 2);
    assert_eq!(buffer.text(), "plain plain");
}

#[test]
fn replace_all_does_not_double_replace_overlapping_patterns() {
    let mut buffer = Buffer::from_str("aaaa");
    let count = buffer.replace_all("aa", "b");
    assert_eq!(count, 2);
    assert_eq!(buffer.text(), "bb");
}

#[test]
fn replace_all_returns_zero_for_empty_or_absent_pattern() {
    let mut buffer = Buffer::from_str("hello");
    assert_eq!(buffer.replace_all("", "x"), 0);
    assert_eq!(buffer.replace_all("zzz", "x"), 0);
    assert_eq!(buffer.text(), "hello");
}

#[test]
fn replace_all_is_a_single_undo_step() {
    let mut buffer = Buffer::from_str("a a a");
    buffer.replace_all("a", "b");
    assert_eq!(buffer.text(), "b b b");
    buffer.undo();
    assert_eq!(buffer.text(), "a a a");
}

#[test]
fn clone_preserves_text_and_modified_flag() {
    let mut buffer = Buffer::from_str("hello");
    buffer.insert_char(Position::new(0, 5), '!');
    let copy = buffer.clone();
    assert_eq!(copy.text(), "hello!");
    assert!(copy.is_modified());
    // The clone owns its own undo history.
    let mut copy = copy;
    copy.undo();
    assert_eq!(copy.text(), "hello");
    assert_eq!(buffer.text(), "hello!");
}

#[test]
fn the_revision_advances_on_every_mutation_including_undo() {
    let mut buffer = Buffer::from_str("a");
    let start = buffer.revision();
    buffer.insert_char(Position::new(0, 1), 'b');
    let after_insert = buffer.revision();
    assert_ne!(after_insert, start);

    buffer.delete_char(Position::new(0, 0));
    let after_delete = buffer.revision();
    assert_ne!(after_delete, after_insert);

    buffer.undo();
    assert_ne!(buffer.revision(), after_delete);
    buffer.redo();
    assert_ne!(buffer.revision(), after_delete);
}

#[test]
fn a_no_op_mutation_does_not_advance_the_revision() {
    let mut buffer = Buffer::from_str("a");
    let start = buffer.revision();
    buffer.insert_str(Position::new(0, 0), "");
    buffer.delete_range(Position::new(0, 1), Position::new(0, 0));
    assert_eq!(buffer.revision(), start);
}

#[test]
fn chunks_reassemble_the_whole_document() {
    let text = "fn main() {\n    println!(\"héllo\");\n}\n".repeat(400);
    let buffer = Buffer::from_str(&text);
    let mut rebuilt = String::new();
    let mut at = 0usize;
    while at < buffer.len_bytes() {
        let chunk = buffer.chunk_at_byte(at);
        assert!(
            !chunk.is_empty(),
            "a chunk inside the buffer must not be empty"
        );
        rebuilt.push_str(chunk);
        at += chunk.len();
    }
    assert_eq!(rebuilt, text);
    assert_eq!(buffer.chunk_at_byte(buffer.len_bytes()), "");
    assert_eq!(buffer.chunk_at_byte(buffer.len_bytes() + 100), "");
}

#[test]
// The reversed range is the point of the last assertion: a caller that hands
// one over must get an empty result rather than a panic.
#[allow(clippy::reversed_empty_ranges)]
fn byte_range_clamps_and_handles_multibyte_text() {
    let buffer = Buffer::from_str("aéb");
    assert_eq!(buffer.byte_range(0..1), b"a".to_vec());
    assert_eq!(buffer.byte_range(1..3), "é".as_bytes().to_vec());
    assert_eq!(buffer.byte_range(0..999), "aéb".as_bytes().to_vec());
    assert!(buffer.byte_range(99..100).is_empty());
    assert!(buffer.byte_range(5..2).is_empty());
}
