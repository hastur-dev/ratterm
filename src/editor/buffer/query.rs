//! Read-only queries over a [`Buffer`]: search, word and line boundaries.
//!
//! Split out of `buffer.rs` so neither file grows past the project's size
//! limit. Nothing in here mutates the buffer.

use super::{Buffer, Position};
use crate::editor::find::{FindCaseInsensitiveIterator, FindIterator};

impl Buffer {
    /// Finds all occurrences of a pattern.
    pub fn find<'a>(&'a self, pattern: &'a str) -> impl Iterator<Item = Position> + 'a {
        FindIterator::new(self, pattern)
    }

    /// Finds all occurrences of a pattern (case insensitive).
    pub fn find_case_insensitive<'a>(
        &'a self,
        pattern: &'a str,
    ) -> impl Iterator<Item = Position> + 'a {
        FindCaseInsensitiveIterator::new(self, pattern)
    }

    /// Gets text in a range.
    #[must_use]
    pub fn get_range(&self, start: Position, end: Position) -> Option<String> {
        let start_idx = self.position_to_index(start);
        let end_idx = self.position_to_index(end);

        if start_idx >= end_idx {
            return None;
        }

        Some(self.rope.slice(start_idx..end_idx).to_string())
    }

    /// Returns the start of the word at position.
    ///
    /// A word here is a run of non-whitespace, which is what the completion and
    /// selection callers want.
    #[must_use]
    pub fn word_start(&self, pos: Position) -> Position {
        let chars = self.line_chars(pos.line);
        let mut start = pos.col.min(chars.len());
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        Position::new(pos.line, start)
    }

    /// Returns the end of the word at position.
    #[must_use]
    pub fn word_end(&self, pos: Position) -> Position {
        let chars = self.line_chars(pos.line);
        let mut end = pos.col.min(chars.len());
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        Position::new(pos.line, end)
    }

    /// Returns a line's characters without its trailing newline.
    #[must_use]
    pub fn line_chars(&self, line: usize) -> Vec<char> {
        self.line(line).map_or_else(Vec::new, |raw| {
            raw.strip_suffix('\n').unwrap_or(&raw).chars().collect()
        })
    }

    /// Returns a line's text without its trailing newline.
    #[must_use]
    pub fn line_text(&self, line: usize) -> String {
        self.line(line).map_or_else(String::new, |raw| {
            raw.strip_suffix('\n').unwrap_or(&raw).to_string()
        })
    }

    /// Returns the position at the start of a line.
    #[must_use]
    pub fn line_start(&self, line: usize) -> Position {
        Position::new(line, 0)
    }

    /// Returns the position at the end of a line.
    #[must_use]
    pub fn line_end(&self, line: usize) -> Position {
        Position::new(line, self.line_len_chars(line))
    }

    /// Returns the first non-whitespace position on a line.
    #[must_use]
    pub fn first_non_whitespace(&self, line: usize) -> Option<Position> {
        if line >= self.len_lines() {
            return None;
        }
        self.line_chars(line)
            .iter()
            .position(|c| !c.is_whitespace())
            .map(|col| Position::new(line, col))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn word_boundaries_stay_on_the_cursor_line() {
        let buffer = Buffer::from_str("alpha beta\ngamma\n");
        assert_eq!(buffer.word_start(Position::new(0, 8)), Position::new(0, 6));
        assert_eq!(buffer.word_end(Position::new(0, 8)), Position::new(0, 10));
        // A previous implementation walked the whole rope and could cross the
        // newline into the line above.
        assert_eq!(buffer.word_start(Position::new(1, 3)), Position::new(1, 0));
    }

    #[test]
    fn word_boundaries_on_whitespace_collapse_to_the_cursor() {
        let buffer = Buffer::from_str("a  b\n");
        assert_eq!(buffer.word_start(Position::new(0, 2)), Position::new(0, 2));
        assert_eq!(buffer.word_end(Position::new(0, 2)), Position::new(0, 2));
    }

    #[test]
    fn word_boundaries_past_the_end_of_a_line_are_clamped() {
        let buffer = Buffer::from_str("ab\n");
        assert_eq!(buffer.word_end(Position::new(0, 99)), Position::new(0, 2));
        assert_eq!(buffer.word_start(Position::new(9, 9)), Position::new(9, 0));
    }

    #[test]
    fn get_range_spans_lines_in_characters() {
        let buffer = Buffer::from_str("aé\nbc\n");
        assert_eq!(
            buffer.get_range(Position::new(0, 1), Position::new(1, 1)),
            Some("é\nb".to_string())
        );
        assert_eq!(
            buffer.get_range(Position::new(1, 1), Position::new(0, 1)),
            None
        );
    }

    #[test]
    fn line_helpers_report_text_without_the_newline() {
        let buffer = Buffer::from_str("  hi\n\n");
        assert_eq!(buffer.line_text(0), "  hi");
        assert_eq!(buffer.line_chars(0).len(), 4);
        assert_eq!(buffer.line_end(0), Position::new(0, 4));
        assert_eq!(buffer.line_start(3), Position::new(3, 0));
        assert_eq!(buffer.first_non_whitespace(0), Some(Position::new(0, 2)));
        assert_eq!(buffer.first_non_whitespace(1), None);
        assert_eq!(buffer.first_non_whitespace(99), None);
    }

    #[test]
    fn find_still_reaches_through_the_query_module() {
        let buffer = Buffer::from_str("ab ab");
        assert_eq!(buffer.find("ab").count(), 2);
        assert_eq!(buffer.find_case_insensitive("AB").count(), 2);
    }
}
