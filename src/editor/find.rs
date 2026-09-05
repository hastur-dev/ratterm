//! Search iterators for the text buffer.
//!
//! Provides iterators for finding pattern matches in text.
//!
//! Both iterators work in **character** space, not byte space. Buffer
//! positions and rope indices are character offsets, so a byte-offset search
//! would report the wrong position (and could panic when slicing inside a
//! multi-byte character) as soon as the buffer contains non-ASCII text.
//!
//! Matches are non-overlapping: after a hit the scan resumes at the character
//! following the match. That is what a text editor means by "find next", and
//! it is what makes [`Buffer::replace_all`] correct for patterns that can
//! overlap themselves (`aa` in `aaa`).

use super::buffer::Buffer;
use super::edit::Position;

/// Maximum iterations for search operations.
pub const MAX_SEARCH_ITERATIONS: usize = 100_000;

/// Folds one character for case-insensitive comparison.
///
/// Uses the first character of the Unicode lowercase mapping so the fold stays
/// 1:1 with the source text. A full lowercase mapping can expand one character
/// into several, which would desynchronise match offsets from buffer
/// positions.
fn fold_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Finds the first occurrence of `needle` in `haystack` at or after `from`.
///
/// Both slices are character slices; the returned index is a character index.
fn find_from(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let last_start = haystack.len() - needle.len();
    if from > last_start {
        return None;
    }
    (from..=last_start).find(|&start| haystack[start..start + needle.len()] == *needle)
}

/// Shared state for the character-space scanners.
struct Scanner<'a> {
    buffer: &'a Buffer,
    haystack: Vec<char>,
    needle: Vec<char>,
    offset: usize,
    iterations: usize,
}

impl Scanner<'_> {
    fn next_match(&mut self) -> Option<Position> {
        if self.needle.is_empty() || self.iterations >= MAX_SEARCH_ITERATIONS {
            return None;
        }

        let start = find_from(&self.haystack, &self.needle, self.offset)?;
        self.offset = start + self.needle.len();
        self.iterations += 1;
        Some(self.buffer.index_to_position(start))
    }
}

/// Iterator for finding pattern matches.
pub struct FindIterator<'a> {
    scanner: Scanner<'a>,
}

impl<'a> FindIterator<'a> {
    /// Creates a new find iterator.
    pub fn new(buffer: &'a Buffer, pattern: &'a str) -> Self {
        Self {
            scanner: Scanner {
                haystack: buffer.text().chars().collect(),
                needle: pattern.chars().collect(),
                buffer,
                offset: 0,
                iterations: 0,
            },
        }
    }
}

impl Iterator for FindIterator<'_> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        self.scanner.next_match()
    }
}

/// Iterator for case-insensitive pattern matches.
pub struct FindCaseInsensitiveIterator<'a> {
    scanner: Scanner<'a>,
}

impl<'a> FindCaseInsensitiveIterator<'a> {
    /// Creates a new case-insensitive find iterator.
    pub fn new(buffer: &'a Buffer, pattern: &str) -> Self {
        Self {
            scanner: Scanner {
                haystack: buffer.text().chars().map(fold_char).collect(),
                needle: pattern.chars().map(fold_char).collect(),
                buffer,
                offset: 0,
                iterations: 0,
            },
        }
    }
}

impl Iterator for FindCaseInsensitiveIterator<'_> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        self.scanner.next_match()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_from_locates_first_match() {
        let hay: Vec<char> = "abcabc".chars().collect();
        let needle: Vec<char> = "bc".chars().collect();
        assert_eq!(find_from(&hay, &needle, 0), Some(1));
        assert_eq!(find_from(&hay, &needle, 2), Some(4));
        assert_eq!(find_from(&hay, &needle, 5), None);
    }

    #[test]
    fn find_from_rejects_empty_and_oversized_needles() {
        let hay: Vec<char> = "ab".chars().collect();
        assert_eq!(find_from(&hay, &[], 0), None);
        let needle: Vec<char> = "abc".chars().collect();
        assert_eq!(find_from(&hay, &needle, 0), None);
    }

    #[test]
    fn matches_are_non_overlapping() {
        let buffer = Buffer::from_str("aaaa");
        let hits: Vec<_> = buffer.find("aa").collect();
        assert_eq!(hits, vec![Position::new(0, 0), Position::new(0, 2)]);
    }

    #[test]
    fn positions_are_character_offsets_not_byte_offsets() {
        // "é" is two bytes but one character; a byte-offset search would report
        // column 3 for "x" instead of column 2.
        let buffer = Buffer::from_str("aéx");
        let hits: Vec<_> = buffer.find("x").collect();
        assert_eq!(hits, vec![Position::new(0, 2)]);
    }

    #[test]
    fn search_does_not_panic_inside_multibyte_characters() {
        let buffer = Buffer::from_str("ααα");
        let hits: Vec<_> = buffer.find("α").collect();
        assert_eq!(
            hits,
            vec![
                Position::new(0, 0),
                Position::new(0, 1),
                Position::new(0, 2)
            ]
        );
    }

    #[test]
    fn case_insensitive_matches_across_scripts() {
        let buffer = Buffer::from_str("Grüße GRÜSSE");
        let hits: Vec<_> = buffer.find_case_insensitive("grü").collect();
        assert_eq!(hits, vec![Position::new(0, 0), Position::new(0, 6)]);
    }

    #[test]
    fn empty_pattern_yields_nothing() {
        let buffer = Buffer::from_str("hello");
        assert_eq!(buffer.find("").count(), 0);
        assert_eq!(buffer.find_case_insensitive("").count(), 0);
    }

    #[test]
    fn no_match_yields_nothing() {
        let buffer = Buffer::from_str("hello");
        assert_eq!(buffer.find("zzz").count(), 0);
    }

    #[test]
    fn matches_span_lines_by_position() {
        let buffer = Buffer::from_str("one\ntwo\nthree");
        let hits: Vec<_> = buffer.find("t").collect();
        assert_eq!(hits, vec![Position::new(1, 0), Position::new(2, 0)]);
    }
}
