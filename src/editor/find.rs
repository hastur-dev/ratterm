//! Search iterators for the text buffer.
//!
//! Provides iterators for finding pattern matches in text.

use super::buffer::Buffer;
use super::edit::Position;

/// Maximum iterations for search operations.
pub const MAX_SEARCH_ITERATIONS: usize = 100_000;

/// Iterator for finding pattern matches.
pub struct FindIterator<'a> {
    buffer: &'a Buffer,
    pattern: &'a str,
    offset: usize,
    iterations: usize,
}

impl<'a> FindIterator<'a> {
    /// Creates a new find iterator.
    pub fn new(buffer: &'a Buffer, pattern: &'a str) -> Self {
        Self {
            buffer,
            pattern,
            offset: 0,
            iterations: 0,
        }
    }
}

impl<'a> Iterator for FindIterator<'a> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pattern.is_empty() || self.iterations >= MAX_SEARCH_ITERATIONS {
            return None;
        }

        let text = self.buffer.text();
        let search_text = &text[self.offset..];

        if let Some(rel_pos) = search_text.find(self.pattern) {
            let abs_pos = self.offset + rel_pos;
            self.offset = abs_pos + 1;
            self.iterations += 1;
            Some(self.buffer.index_to_position(abs_pos))
        } else {
            None
        }
    }
}

/// Iterator for case-insensitive pattern matches.
pub struct FindCaseInsensitiveIterator<'a> {
    buffer: &'a Buffer,
    pattern: String,
    offset: usize,
    iterations: usize,
}

impl<'a> FindCaseInsensitiveIterator<'a> {
    /// Creates a new case-insensitive find iterator.
    pub fn new(buffer: &'a Buffer, pattern: &str) -> Self {
        Self {
            buffer,
            pattern: pattern.to_lowercase(),
            offset: 0,
            iterations: 0,
        }
    }
}

impl<'a> Iterator for FindCaseInsensitiveIterator<'a> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pattern.is_empty() || self.iterations >= MAX_SEARCH_ITERATIONS {
            return None;
        }

        let text = self.buffer.text().to_lowercase();
        let search_text = &text[self.offset..];

        if let Some(rel_pos) = search_text.find(&self.pattern) {
            let abs_pos = self.offset + rel_pos;
            self.offset = abs_pos + 1;
            self.iterations += 1;
            Some(self.buffer.index_to_position(abs_pos))
        } else {
            None
        }
    }
}
