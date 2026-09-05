//! Turning buffer edits into the [`InputEdit`] values tree-sitter needs.
//!
//! Split out of `highlight.rs` so neither file grows past the project's size
//! limit. `Point::column` is a **byte** offset within its line, which is why
//! nothing here can reuse the character columns carried by [`Position`].

use tree_sitter::{InputEdit, Point};

use super::buffer::{Buffer, Position};

/// Returns the byte offset and tree-sitter point for a buffer position.
///
/// The scan is per line, so it costs the length of the line the position is on
/// plus the walk down to that line, not a copy of the document.
#[must_use]
pub fn locate(buffer: &Buffer, pos: Position) -> (usize, Point) {
    let last = buffer.len_lines().saturating_sub(1);
    let row = pos.line.min(last);
    let col = pos.col.min(buffer.line_len_chars(row));
    let line_start_byte = buffer.line_to_byte(row);
    let byte = buffer.char_to_byte(buffer.position_to_index(Position::new(row, col)));
    (byte, Point::new(row, byte.saturating_sub(line_start_byte)))
}

/// Builds the tree-sitter edit describing an insertion into `before`.
///
/// `before` is the buffer as it was prior to the insertion.
#[must_use]
pub fn insertion_edit(before: &Buffer, at: Position, inserted: &str) -> InputEdit {
    let (start_byte, start_position) = locate(before, at);
    let added_lines = inserted.matches('\n').count();
    let new_end_position = if added_lines == 0 {
        Point::new(start_position.row, start_position.column + inserted.len())
    } else {
        let last = inserted.rsplit('\n').next().unwrap_or("");
        Point::new(start_position.row + added_lines, last.len())
    };
    InputEdit {
        start_byte,
        old_end_byte: start_byte,
        new_end_byte: start_byte + inserted.len(),
        start_position,
        old_end_position: start_position,
        new_end_position,
    }
}

/// Builds the tree-sitter edit describing a deletion from `before`.
///
/// `before` is the buffer as it was prior to the deletion. The positions may be
/// supplied in either order.
#[must_use]
pub fn deletion_edit(before: &Buffer, start: Position, end: Position) -> InputEdit {
    let a = locate(before, start);
    let b = locate(before, end);
    let ((start_byte, start_position), (old_end_byte, old_end_position)) =
        if a.0 <= b.0 { (a, b) } else { (b, a) };
    InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte: start_byte,
        start_position,
        old_end_position,
        new_end_position: start_position,
    }
}

/// Builds the edit describing a replacement of `start..end` with `text`.
#[must_use]
pub fn replacement_edit(before: &Buffer, start: Position, end: Position, text: &str) -> InputEdit {
    let mut edit = deletion_edit(before, start, end);
    let added_lines = text.matches('\n').count();
    edit.new_end_byte = edit.start_byte + text.len();
    edit.new_end_position = if added_lines == 0 {
        Point::new(
            edit.start_position.row,
            edit.start_position.column + text.len(),
        )
    } else {
        let last = text.rsplit('\n').next().unwrap_or("");
        Point::new(edit.start_position.row + added_lines, last.len())
    };
    edit
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn deletion_edit_covers_the_removed_range_in_bytes() {
        let buffer = Buffer::from_str("aé\nbc\n");
        let edit = deletion_edit(&buffer, Position::new(0, 1), Position::new(1, 1));
        assert_eq!(edit.start_byte, 1);
        // 'é' is two bytes, then the newline, then one byte of line 1.
        assert_eq!(edit.old_end_byte, 5);
        assert_eq!(edit.new_end_byte, 1);
        assert_eq!(edit.start_position, Point::new(0, 1));
        assert_eq!(edit.old_end_position, Point::new(1, 1));
    }

    #[test]
    fn deletion_edit_accepts_reversed_positions() {
        let buffer = Buffer::from_str("abcdef\n");
        let forward = deletion_edit(&buffer, Position::new(0, 1), Position::new(0, 4));
        let reversed = deletion_edit(&buffer, Position::new(0, 4), Position::new(0, 1));
        assert_eq!(forward.start_byte, reversed.start_byte);
        assert_eq!(forward.old_end_byte, reversed.old_end_byte);
    }

    #[test]
    fn insertion_edit_tracks_added_lines() {
        let buffer = Buffer::from_str("abc\n");
        let edit = insertion_edit(&buffer, Position::new(0, 3), "\nxy");
        assert_eq!(edit.start_byte, 3);
        assert_eq!(edit.old_end_byte, 3);
        assert_eq!(edit.new_end_byte, 6);
        assert_eq!(edit.new_end_position, Point::new(1, 2));
    }

    #[test]
    fn locate_matches_a_byte_scan_of_the_same_text() {
        let text = "let é = 1;\nlet ü = 2;\nplain\n";
        let buffer = Buffer::from_str(text);
        for (line, raw) in text.lines().enumerate() {
            for col in 0..=raw.chars().count() {
                let (byte, point) = locate(&buffer, Position::new(line, col));
                let expected_line_start: usize = text.lines().take(line).map(|l| l.len() + 1).sum();
                let expected_col: usize = raw.chars().take(col).map(char::len_utf8).sum();
                assert_eq!(byte, expected_line_start + expected_col, "{line}:{col}");
                assert_eq!(point, Point::new(line, expected_col), "{line}:{col}");
            }
        }
    }

    #[test]
    fn locate_clamps_a_position_past_the_end() {
        let buffer = Buffer::from_str("ab\n");
        let (byte, point) = locate(&buffer, Position::new(99, 99));
        assert!(byte <= buffer.len_bytes());
        assert!(point.row < buffer.len_lines());
    }

    #[test]
    fn replacement_edit_reports_the_new_text_length() {
        let buffer = Buffer::from_str("one two\n");
        let edit = replacement_edit(&buffer, Position::new(0, 0), Position::new(0, 3), "seven");
        assert_eq!(edit.start_byte, 0);
        assert_eq!(edit.old_end_byte, 3);
        assert_eq!(edit.new_end_byte, 5);
        assert_eq!(edit.new_end_position, Point::new(0, 5));
    }

    #[test]
    fn replacement_edit_tracks_multi_line_replacements() {
        let buffer = Buffer::from_str("abc\n");
        let edit = replacement_edit(&buffer, Position::new(0, 1), Position::new(0, 2), "x\ny");
        assert_eq!(edit.new_end_position, Point::new(1, 1));
    }
}
