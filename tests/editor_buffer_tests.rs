//! Tests for editor text buffer operations.
//!
//! Tests cover: text insertion, deletion, cursor movement, undo/redo.

use ratterm::editor::buffer::{Buffer, Position};

/// Test buffer creation with empty content.
#[test]
fn test_buffer_new_empty() {
    let buffer = Buffer::new();

    assert_eq!(buffer.len_lines(), 1, "Empty buffer has one line");
    assert_eq!(buffer.len_chars(), 0, "Empty buffer has zero chars");
    assert!(buffer.is_empty(), "Buffer should be empty");
}

/// Test buffer creation from string.
#[test]
fn test_buffer_from_string() {
    let buffer = Buffer::from_str("Hello\nWorld");

    assert_eq!(buffer.len_lines(), 2, "Should have two lines");
    assert_eq!(
        buffer.line(0),
        Some("Hello\n".into()),
        "First line mismatch"
    );
    assert_eq!(buffer.line(1), Some("World".into()), "Second line mismatch");
}

/// Test buffer line retrieval.
#[test]
fn test_buffer_get_line() {
    let buffer = Buffer::from_str("Line 1\nLine 2\nLine 3");

    assert_eq!(buffer.line(0), Some("Line 1\n".into()));
    assert_eq!(buffer.line(1), Some("Line 2\n".into()));
    assert_eq!(buffer.line(2), Some("Line 3".into()));
    assert_eq!(buffer.line(3), None, "Out of bounds should return None");
}

/// Test inserting character at position.
#[test]
fn test_buffer_insert_char() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.insert_char(Position::new(0, 5), '!');

    assert_eq!(buffer.text(), "Hello!", "Character should be inserted");
}

/// Test inserting character at beginning.
#[test]
fn test_buffer_insert_char_beginning() {
    let mut buffer = Buffer::from_str("ello");

    buffer.insert_char(Position::new(0, 0), 'H');

    assert_eq!(buffer.text(), "Hello", "Character at beginning");
}

/// Test inserting character in middle.
#[test]
fn test_buffer_insert_char_middle() {
    let mut buffer = Buffer::from_str("Hllo");

    buffer.insert_char(Position::new(0, 1), 'e');

    assert_eq!(buffer.text(), "Hello", "Character in middle");
}

/// Test inserting newline.
#[test]
fn test_buffer_insert_newline() {
    let mut buffer = Buffer::from_str("HelloWorld");

    buffer.insert_char(Position::new(0, 5), '\n');

    assert_eq!(buffer.len_lines(), 2, "Should split into two lines");
    assert_eq!(buffer.line(0), Some("Hello\n".into()));
    assert_eq!(buffer.line(1), Some("World".into()));
}

/// Test inserting string.
#[test]
fn test_buffer_insert_string() {
    let mut buffer = Buffer::from_str("Hello!");

    buffer.insert_str(Position::new(0, 5), " World");

    assert_eq!(buffer.text(), "Hello World!", "String inserted");
}

/// Test inserting multiline string.
#[test]
fn test_buffer_insert_multiline_string() {
    let mut buffer = Buffer::from_str("AB");

    buffer.insert_str(Position::new(0, 1), "X\nY\nZ");

    assert_eq!(buffer.text(), "AX\nY\nZB", "Multiline insert");
    assert_eq!(buffer.len_lines(), 3, "Should have 3 lines");
}

/// Test deleting character forward.
#[test]
fn test_buffer_delete_char_forward() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.delete_char(Position::new(0, 0));

    assert_eq!(buffer.text(), "ello", "First char deleted");
}

/// Test deleting character backward (backspace).
#[test]
fn test_buffer_delete_char_backward() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.delete_char_backward(Position::new(0, 5));

    assert_eq!(buffer.text(), "Hell", "Last char deleted");
}

/// Test deleting newline (join lines).
#[test]
fn test_buffer_delete_newline() {
    let mut buffer = Buffer::from_str("Hello\nWorld");

    buffer.delete_char(Position::new(0, 5)); // Delete the newline

    assert_eq!(buffer.text(), "HelloWorld", "Lines joined");
    assert_eq!(buffer.len_lines(), 1, "Should be one line");
}

/// Test deleting range.
#[test]
fn test_buffer_delete_range() {
    let mut buffer = Buffer::from_str("Hello World");

    buffer.delete_range(Position::new(0, 5), Position::new(0, 11));

    assert_eq!(buffer.text(), "Hello", "Range deleted");
}

/// Test deleting multiline range.
#[test]
fn test_buffer_delete_multiline_range() {
    let mut buffer = Buffer::from_str("Line 1\nLine 2\nLine 3");

    buffer.delete_range(Position::new(0, 4), Position::new(2, 4));

    assert_eq!(buffer.text(), "Line 3", "Multiline range deleted");
}

/// Test position to char index conversion.
#[test]
fn test_buffer_position_to_index() {
    let buffer = Buffer::from_str("Hello\nWorld");

    assert_eq!(buffer.position_to_index(Position::new(0, 0)), 0);
    assert_eq!(buffer.position_to_index(Position::new(0, 5)), 5);
    assert_eq!(buffer.position_to_index(Position::new(1, 0)), 6); // After newline
    assert_eq!(buffer.position_to_index(Position::new(1, 5)), 11);
}

/// Test char index to position conversion.
#[test]
fn test_buffer_index_to_position() {
    let buffer = Buffer::from_str("Hello\nWorld");

    assert_eq!(buffer.index_to_position(0), Position::new(0, 0));
    assert_eq!(buffer.index_to_position(5), Position::new(0, 5));
    assert_eq!(buffer.index_to_position(6), Position::new(1, 0));
    assert_eq!(buffer.index_to_position(11), Position::new(1, 5));
}

/// Test line length.
#[test]
fn test_buffer_line_len() {
    let buffer = Buffer::from_str("Hello\nHi\nWorld");

    assert_eq!(buffer.line_len(0), 6, "First line including newline");
    assert_eq!(buffer.line_len(1), 3, "Second line including newline");
    assert_eq!(buffer.line_len(2), 5, "Third line no newline");
    assert_eq!(buffer.line_len(3), 0, "Out of bounds");
}

/// Test line length without newline.
#[test]
fn test_buffer_line_len_chars() {
    let buffer = Buffer::from_str("Hello\nHi\nWorld");

    assert_eq!(buffer.line_len_chars(0), 5, "First line without newline");
    assert_eq!(buffer.line_len_chars(1), 2, "Second line without newline");
    assert_eq!(buffer.line_len_chars(2), 5, "Third line");
}

/// Test undo single operation.
#[test]
fn test_buffer_undo_single() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.insert_char(Position::new(0, 5), '!');
    assert_eq!(buffer.text(), "Hello!");

    buffer.undo();
    assert_eq!(buffer.text(), "Hello", "Undo should restore original");
}

/// Test redo single operation.
#[test]
fn test_buffer_redo_single() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.insert_char(Position::new(0, 5), '!');
    buffer.undo();
    buffer.redo();

    assert_eq!(buffer.text(), "Hello!", "Redo should restore change");
}

/// Test multiple undo operations.
#[test]
fn test_buffer_undo_multiple() {
    let mut buffer = Buffer::from_str("");

    buffer.insert_char(Position::new(0, 0), 'A');
    buffer.insert_char(Position::new(0, 1), 'B');
    buffer.insert_char(Position::new(0, 2), 'C');

    assert_eq!(buffer.text(), "ABC");

    buffer.undo();
    assert_eq!(buffer.text(), "AB");

    buffer.undo();
    assert_eq!(buffer.text(), "A");

    buffer.undo();
    assert_eq!(buffer.text(), "");
}

/// Test undo clears redo stack on new edit.
#[test]
fn test_buffer_undo_clears_redo() {
    let mut buffer = Buffer::from_str("A");

    buffer.insert_char(Position::new(0, 1), 'B');
    buffer.undo();

    // New edit should clear redo
    buffer.insert_char(Position::new(0, 1), 'X');

    // Redo should do nothing now
    buffer.redo();
    assert_eq!(buffer.text(), "AX", "Redo stack should be cleared");
}

/// Test position validation.
#[test]
fn test_buffer_clamp_position() {
    let buffer = Buffer::from_str("Hello\nWorld");

    // Position beyond line end
    let clamped = buffer.clamp_position(Position::new(0, 100));
    assert_eq!(clamped, Position::new(0, 5), "Should clamp to line end");

    // Position beyond last line
    let clamped = buffer.clamp_position(Position::new(100, 0));
    assert_eq!(clamped.line, 1, "Should clamp to last line");
}

/// Test word boundaries.
#[test]
fn test_buffer_word_start() {
    let buffer = Buffer::from_str("hello world test");

    assert_eq!(buffer.word_start(Position::new(0, 8)), Position::new(0, 6));
    assert_eq!(buffer.word_start(Position::new(0, 4)), Position::new(0, 0));
    assert_eq!(buffer.word_start(Position::new(0, 6)), Position::new(0, 6));
}

/// Test word end.
#[test]
fn test_buffer_word_end() {
    let buffer = Buffer::from_str("hello world test");

    assert_eq!(buffer.word_end(Position::new(0, 0)), Position::new(0, 5));
    assert_eq!(buffer.word_end(Position::new(0, 6)), Position::new(0, 11));
}

/// Test find text.
#[test]
fn test_buffer_find() {
    let buffer = Buffer::from_str("Hello World Hello");

    let matches: Vec<_> = buffer.find("Hello").collect();
    assert_eq!(matches.len(), 2, "Should find two matches");
    assert_eq!(matches[0], Position::new(0, 0));
    assert_eq!(matches[1], Position::new(0, 12));
}

/// Test find case insensitive.
#[test]
fn test_buffer_find_case_insensitive() {
    let buffer = Buffer::from_str("Hello HELLO hello");

    let matches: Vec<_> = buffer.find_case_insensitive("hello").collect();
    assert_eq!(matches.len(), 3, "Should find three matches");
}

/// Test replace.
#[test]
fn test_buffer_replace() {
    let mut buffer = Buffer::from_str("Hello World");

    buffer.replace(Position::new(0, 6), Position::new(0, 11), "Rust");

    assert_eq!(buffer.text(), "Hello Rust", "Replace text");
}

/// Test replace all.
#[test]
fn test_buffer_replace_all() {
    let mut buffer = Buffer::from_str("foo bar foo baz foo");

    let count = buffer.replace_all("foo", "qux");

    assert_eq!(count, 3, "Should replace 3 occurrences");
    assert_eq!(buffer.text(), "qux bar qux baz qux");
}

/// Test get text range.
#[test]
fn test_buffer_get_range() {
    let buffer = Buffer::from_str("Hello World");

    let text = buffer.get_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(text, Some("Hello".to_string()));

    let text = buffer.get_range(Position::new(0, 6), Position::new(0, 11));
    assert_eq!(text, Some("World".to_string()));
}

/// Test get multiline range.
#[test]
fn test_buffer_get_multiline_range() {
    let buffer = Buffer::from_str("Line 1\nLine 2\nLine 3");

    let text = buffer.get_range(Position::new(0, 5), Position::new(2, 4));
    assert_eq!(text, Some("1\nLine 2\nLine".to_string()));
}

/// Test modified flag.
#[test]
fn test_buffer_modified() {
    let mut buffer = Buffer::from_str("Hello");

    assert!(!buffer.is_modified(), "New buffer is not modified");

    buffer.insert_char(Position::new(0, 5), '!');
    assert!(buffer.is_modified(), "After edit, buffer is modified");

    buffer.mark_saved();
    assert!(!buffer.is_modified(), "After save, buffer is not modified");
}

/// Test empty line insertion.
#[test]
fn test_buffer_insert_empty_line() {
    let mut buffer = Buffer::from_str("Line1\nLine2");

    buffer.insert_char(Position::new(0, 5), '\n');
    buffer.insert_char(Position::new(1, 0), '\n');

    assert_eq!(buffer.len_lines(), 4, "Should have 4 lines");
    assert_eq!(buffer.line(1), Some("\n".into()), "Empty line");
}

/// Test UTF-8 handling.
#[test]
fn test_buffer_utf8() {
    let mut buffer = Buffer::from_str("Hello 世界");

    // "Hello " = 6 chars + "世界" = 2 chars = 8 total
    assert_eq!(
        buffer.len_chars(),
        8,
        "8 characters including space and CJK"
    );

    buffer.insert_char(Position::new(0, 8), '!');
    assert_eq!(buffer.text(), "Hello 世界!", "UTF-8 preserved");
}

/// Test position at line start.
#[test]
fn test_buffer_line_start() {
    let buffer = Buffer::from_str("  Hello\n    World");

    assert_eq!(buffer.line_start(0), Position::new(0, 0));
    assert_eq!(buffer.line_start(1), Position::new(1, 0));
}

/// Test first non-whitespace position.
#[test]
fn test_buffer_first_non_whitespace() {
    let buffer = Buffer::from_str("  Hello\n    World");

    assert_eq!(buffer.first_non_whitespace(0), Some(Position::new(0, 2)));
    assert_eq!(buffer.first_non_whitespace(1), Some(Position::new(1, 4)));
}

/// Test line end position.
#[test]
fn test_buffer_line_end() {
    let buffer = Buffer::from_str("Hello\nWorld");

    assert_eq!(buffer.line_end(0), Position::new(0, 5));
    assert_eq!(buffer.line_end(1), Position::new(1, 5));
}

/// Test insert at end of buffer.
#[test]
fn test_buffer_insert_at_end() {
    let mut buffer = Buffer::from_str("Hello");

    buffer.insert_str(Position::new(0, 5), "\nWorld");

    assert_eq!(buffer.text(), "Hello\nWorld");
    assert_eq!(buffer.len_lines(), 2);
}

/// Test delete to line start.
#[test]
fn test_buffer_delete_to_line_start() {
    let mut buffer = Buffer::from_str("Hello World");

    buffer.delete_range(Position::new(0, 0), Position::new(0, 6));

    assert_eq!(buffer.text(), "World");
}

/// Test grouping edits for undo.
#[test]
fn test_buffer_undo_group() {
    let mut buffer = Buffer::from_str("");

    buffer.begin_undo_group();
    buffer.insert_char(Position::new(0, 0), 'A');
    buffer.insert_char(Position::new(0, 1), 'B');
    buffer.insert_char(Position::new(0, 2), 'C');
    buffer.end_undo_group();

    assert_eq!(buffer.text(), "ABC");

    // Single undo should revert all grouped changes
    buffer.undo();
    assert_eq!(buffer.text(), "", "Grouped undo");
}
