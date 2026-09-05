//! Secondary cursors.
//!
//! The editor keeps one primary [`Cursor`](super::cursor::Cursor), which owns
//! selection and preferred column, plus a set of secondary positions held here.
//! Typing fans out to all of them; `Escape` drops back to one.
//!
//! The fan-out rules are the interesting part and they live in this file as
//! pure functions over positions, so they can be checked without an editor.

use super::buffer::{Buffer, Position};

/// Secondary cursors beyond this count are refused.
///
/// "Add a cursor at every occurrence" in a large file is a mistake far more
/// often than it is a plan.
pub const MAX_EXTRA_CURSORS: usize = 512;

/// The secondary cursors, in document order.
#[derive(Debug, Clone, Default)]
pub struct MultiCursor {
    extra: Vec<Position>,
}

impl MultiCursor {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self { extra: Vec::new() }
    }

    /// Returns the secondary cursors, in document order.
    #[must_use]
    pub fn positions(&self) -> &[Position] {
        &self.extra
    }

    /// Returns how many secondary cursors there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.extra.len()
    }

    /// Returns true when only the primary cursor is active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extra.is_empty()
    }

    /// Drops every secondary cursor.
    pub fn clear(&mut self) {
        self.extra.clear();
    }

    /// Adds a cursor at `pos` unless one is already there.
    ///
    /// Returns false when the position is a duplicate or the cap is reached.
    pub fn add(&mut self, pos: Position) -> bool {
        if self.extra.len() >= MAX_EXTRA_CURSORS || self.extra.contains(&pos) {
            return false;
        }
        self.extra.push(pos);
        self.sort();
        true
    }

    /// Removes a cursor at `pos`, if there is one.
    pub fn remove(&mut self, pos: Position) -> bool {
        let before = self.extra.len();
        self.extra.retain(|p| *p != pos);
        before != self.extra.len()
    }

    /// Replaces the whole set.
    pub fn set(&mut self, positions: Vec<Position>) {
        self.extra = positions;
        self.sort();
        self.extra.dedup();
        self.extra.truncate(MAX_EXTRA_CURSORS);
    }

    fn sort(&mut self) {
        self.extra.sort_by_key(|p| (p.line, p.col));
        self.extra.dedup();
    }

    /// Clamps every cursor into `buffer`, dropping duplicates.
    pub fn clamp(&mut self, buffer: &Buffer) {
        let clamped: Vec<Position> = self
            .extra
            .iter()
            .map(|p| buffer.clamp_position(*p))
            .collect();
        self.set(clamped);
    }

    /// Returns every cursor, primary included, in document order.
    #[must_use]
    pub fn all_with(&self, primary: Position) -> Vec<Position> {
        let mut all = self.extra.clone();
        all.push(primary);
        all.sort_by_key(|p| (p.line, p.col));
        all.dedup();
        all
    }

    /// Returns the secondary cursors on one line, as columns.
    #[must_use]
    pub fn columns_on_line(&self, line: usize) -> Vec<usize> {
        self.extra
            .iter()
            .filter(|p| p.line == line)
            .map(|p| p.col)
            .collect()
    }
}

/// Returns the position each cursor ends at after `text` is inserted at all of
/// them, in the same order as `sorted`.
///
/// Insertions are applied last-first so earlier positions stay valid; each
/// cursor then sits just past its own insertion, which means the *n*-th cursor
/// shifts by *n* insertions' worth of text.
#[must_use]
pub fn positions_after_insert(sorted: &[Position], text: &str) -> Vec<Position> {
    let added_lines = text.matches('\n').count();
    let tail_len = text.rsplit('\n').next().unwrap_or("").chars().count();
    let width = text.chars().count();

    let mut out = Vec::with_capacity(sorted.len());
    let mut line_shift = 0usize;
    let mut col_shift = 0usize;
    let mut shifted_line: Option<usize> = None;

    for pos in sorted {
        let line = pos.line + line_shift;
        // The column shift only applies while we stay on the line the previous
        // insertion ended on.
        let col = if shifted_line == Some(line) {
            pos.col + col_shift
        } else {
            pos.col
        };

        if added_lines == 0 {
            out.push(Position::new(line, col + width));
            col_shift = col + width - pos.col;
            shifted_line = Some(line);
        } else {
            let end_line = line + added_lines;
            out.push(Position::new(end_line, tail_len));
            line_shift += added_lines;
            col_shift = tail_len;
            shifted_line = Some(end_line);
        }
    }
    out
}

/// Returns where each cursor ends after one backspace at every one of them.
///
/// `sorted` holds character indices in ascending order. A cursor at index zero
/// deletes nothing; every other one loses its own preceding character plus the
/// characters the cursors before it deleted.
#[must_use]
pub fn indices_after_backspace(sorted: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(sorted.len());
    let mut removed = 0usize;
    for index in sorted {
        if *index == 0 {
            out.push(0);
            continue;
        }
        out.push(index - 1 - removed);
        removed += 1;
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_new_set_is_empty() {
        let cursors = MultiCursor::new();
        assert!(cursors.is_empty());
        assert_eq!(cursors.len(), 0);
        assert_eq!(cursors.positions(), &[]);
    }

    #[test]
    fn cursors_are_kept_in_document_order_without_duplicates() {
        let mut cursors = MultiCursor::new();
        assert!(cursors.add(Position::new(2, 0)));
        assert!(cursors.add(Position::new(0, 5)));
        assert!(cursors.add(Position::new(0, 1)));
        assert!(!cursors.add(Position::new(0, 1)), "duplicates are refused");
        assert_eq!(
            cursors.positions(),
            &[
                Position::new(0, 1),
                Position::new(0, 5),
                Position::new(2, 0)
            ]
        );
    }

    #[test]
    fn removing_and_clearing_work() {
        let mut cursors = MultiCursor::new();
        cursors.add(Position::new(1, 1));
        assert!(cursors.remove(Position::new(1, 1)));
        assert!(!cursors.remove(Position::new(1, 1)));
        cursors.add(Position::new(2, 2));
        cursors.clear();
        assert!(cursors.is_empty());
    }

    #[test]
    fn the_cap_stops_runaway_cursor_counts() {
        let mut cursors = MultiCursor::new();
        for line in 0..MAX_EXTRA_CURSORS {
            assert!(cursors.add(Position::new(line, 0)));
        }
        assert!(!cursors.add(Position::new(MAX_EXTRA_CURSORS, 0)));
        assert_eq!(cursors.len(), MAX_EXTRA_CURSORS);
    }

    #[test]
    fn setting_the_whole_list_sorts_dedups_and_truncates() {
        let mut cursors = MultiCursor::new();
        cursors.set(vec![
            Position::new(3, 0),
            Position::new(1, 0),
            Position::new(3, 0),
        ]);
        assert_eq!(
            cursors.positions(),
            &[Position::new(1, 0), Position::new(3, 0)]
        );
    }

    #[test]
    fn clamping_drops_positions_past_the_end() {
        let buffer = Buffer::from_str("ab\ncd\n");
        let mut cursors = MultiCursor::new();
        cursors.set(vec![Position::new(0, 99), Position::new(99, 99)]);
        cursors.clamp(&buffer);
        for pos in cursors.positions() {
            assert!(pos.line < buffer.len_lines());
            assert!(pos.col <= buffer.line_len_chars(pos.line));
        }
    }

    #[test]
    fn all_with_merges_the_primary_cursor_in_order() {
        let mut cursors = MultiCursor::new();
        cursors.add(Position::new(2, 0));
        cursors.add(Position::new(0, 0));
        assert_eq!(
            cursors.all_with(Position::new(1, 0)),
            vec![
                Position::new(0, 0),
                Position::new(1, 0),
                Position::new(2, 0)
            ]
        );
        // A primary that coincides with a secondary is not counted twice.
        assert_eq!(cursors.all_with(Position::new(0, 0)).len(), 2);
    }

    #[test]
    fn columns_on_a_line_are_reported_for_the_renderer() {
        let mut cursors = MultiCursor::new();
        cursors.add(Position::new(1, 3));
        cursors.add(Position::new(1, 7));
        cursors.add(Position::new(2, 0));
        assert_eq!(cursors.columns_on_line(1), vec![3, 7]);
        assert!(cursors.columns_on_line(9).is_empty());
    }

    #[test]
    fn inserting_one_character_shifts_later_cursors_on_the_same_line() {
        let sorted = vec![Position::new(0, 0), Position::new(0, 4)];
        assert_eq!(
            positions_after_insert(&sorted, "x"),
            vec![Position::new(0, 1), Position::new(0, 6)]
        );
    }

    #[test]
    fn inserting_on_different_lines_does_not_cross_shift_columns() {
        let sorted = vec![Position::new(0, 2), Position::new(1, 2)];
        assert_eq!(
            positions_after_insert(&sorted, "ab"),
            vec![Position::new(0, 4), Position::new(1, 4)]
        );
    }

    #[test]
    fn inserting_a_newline_pushes_later_cursors_down() {
        let sorted = vec![Position::new(0, 1), Position::new(2, 3)];
        // Each cursor lands just after its own newline, and the second one is
        // a line lower than it was because the first insertion added a line.
        assert_eq!(
            positions_after_insert(&sorted, "\n"),
            vec![Position::new(1, 0), Position::new(4, 0)]
        );
    }

    #[test]
    fn two_cursors_on_one_line_both_track_a_multi_line_insert() {
        let sorted = vec![Position::new(0, 0), Position::new(0, 4)];
        assert_eq!(
            positions_after_insert(&sorted, "a\nb"),
            vec![Position::new(1, 1), Position::new(2, 1)]
        );
    }

    #[test]
    fn inserting_into_an_empty_cursor_set_returns_nothing() {
        assert!(positions_after_insert(&[], "x").is_empty());
    }

    #[test]
    fn backspacing_shifts_every_later_cursor_by_the_deletions_before_it() {
        // Two cursors, both able to delete: the second loses its own character
        // and the one the first deleted.
        assert_eq!(indices_after_backspace(&[3, 7]), vec![2, 5]);
        // A cursor at the very start deletes nothing, so nothing shifts for it.
        assert_eq!(indices_after_backspace(&[0, 4]), vec![0, 3]);
        assert!(indices_after_backspace(&[]).is_empty());
    }
}
