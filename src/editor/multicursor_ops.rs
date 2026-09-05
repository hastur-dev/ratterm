//! Editing through several cursors at once.
//!
//! The arithmetic is in [`multicursor`](super::multicursor); this file applies
//! it to the buffer. Every fan-out is a single undo group, so one `Ctrl+Z`
//! takes back what one keystroke did at every cursor.

use super::multicursor::{MultiCursor, indices_after_backspace, positions_after_insert};
use super::{Editor, Position};

impl Editor {
    /// Returns the secondary cursors.
    #[must_use]
    pub const fn extra_cursors(&self) -> &MultiCursor {
        &self.cursors
    }

    /// Returns every cursor, primary included, in document order.
    #[must_use]
    pub fn all_cursors(&self) -> Vec<Position> {
        self.cursors.all_with(self.cursor.position())
    }

    /// Drops back to a single cursor. This is what `Escape` does.
    pub fn clear_extra_cursors(&mut self) {
        self.cursors.clear();
    }

    /// Adds a cursor on the line below the lowest current one.
    ///
    /// Returns false at the bottom of the document.
    pub fn add_cursor_below(&mut self) -> bool {
        let all = self.all_cursors();
        let Some(lowest) = all.last().copied() else {
            return false;
        };
        let line = lowest.line + 1;
        if line >= self.buffer.len_lines() {
            return false;
        }
        let col = self
            .cursor
            .position()
            .col
            .min(self.buffer.line_len_chars(line));
        self.add_cursor_at(Position::new(line, col))
    }

    /// Adds a cursor on the line above the highest current one.
    pub fn add_cursor_above(&mut self) -> bool {
        let all = self.all_cursors();
        let Some(highest) = all.first().copied() else {
            return false;
        };
        if highest.line == 0 {
            return false;
        }
        let line = highest.line - 1;
        let col = self
            .cursor
            .position()
            .col
            .min(self.buffer.line_len_chars(line));
        self.add_cursor_at(Position::new(line, col))
    }

    /// Adds a cursor at `pos`, keeping the primary where it is.
    pub fn add_cursor_at(&mut self, pos: Position) -> bool {
        let pos = self.buffer.clamp_position(pos);
        if pos == self.cursor.position() {
            return false;
        }
        self.cursors.add(pos)
    }

    /// Adds a cursor at the next occurrence of the word under the primary one.
    ///
    /// Searching wraps, so repeated use walks every occurrence in the file and
    /// then stops rather than adding the same position twice. Returns false
    /// when there is no word under the cursor or no other occurrence.
    pub fn add_cursor_at_next_occurrence(&mut self) -> bool {
        let Some(word) = self.word_at_cursor().filter(|w| !w.trim().is_empty()) else {
            return false;
        };
        let from = self
            .all_cursors()
            .last()
            .copied()
            .unwrap_or_else(|| self.cursor.position());
        let taken = self.all_cursors();

        let hits: Vec<Position> = self.buffer.find(&word).collect();
        let next = hits
            .iter()
            .find(|p| (p.line, p.col) > (from.line, from.col) && !taken.contains(p))
            .or_else(|| hits.iter().find(|p| !taken.contains(p)));

        match next.copied() {
            Some(pos) => self.add_cursor_at(pos),
            None => false,
        }
    }

    /// Adds a cursor at every occurrence of the word under the primary cursor.
    ///
    /// Returns how many were added.
    pub fn add_cursors_at_all_occurrences(&mut self) -> usize {
        let Some(word) = self.word_at_cursor().filter(|w| !w.trim().is_empty()) else {
            return 0;
        };
        let hits: Vec<Position> = self.buffer.find(&word).collect();
        let mut added = 0usize;
        for pos in hits {
            if self.add_cursor_at(pos) {
                added += 1;
            }
        }
        added
    }

    /// Inserts `text` at every cursor as one undo step.
    pub(crate) fn insert_at_every_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let primary = self.cursor.position();
        let all = self.cursors.all_with(primary);
        let after = positions_after_insert(&all, text);

        self.buffer.begin_undo_group();
        for pos in all.iter().rev() {
            self.apply_insert(*pos, text);
        }
        self.buffer.end_undo_group();

        let primary_index = all.iter().position(|p| *p == primary).unwrap_or(0);
        let new_primary = after
            .get(primary_index)
            .copied()
            .unwrap_or_else(|| self.buffer.clamp_position(primary));
        self.cursor.move_to(self.buffer.clamp_position(new_primary));
        let others = after
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != primary_index)
            .map(|(_, p)| self.buffer.clamp_position(p))
            .collect();
        self.cursors.set(others);
    }

    /// Deletes one character before every cursor as one undo step.
    pub(crate) fn backspace_at_every_cursor(&mut self) {
        let primary = self.cursor.position();
        let all = self.cursors.all_with(primary);
        let indices: Vec<usize> = all
            .iter()
            .map(|p| self.buffer.position_to_index(*p))
            .collect();
        let after = indices_after_backspace(&indices);

        self.buffer.begin_undo_group();
        for index in indices.iter().rev() {
            if *index == 0 {
                continue;
            }
            let start = self.buffer.index_to_position(index - 1);
            let end = self.buffer.index_to_position(*index);
            self.apply_delete(start, end);
        }
        self.buffer.end_undo_group();

        let primary_index = all.iter().position(|p| *p == primary).unwrap_or(0);
        let positions: Vec<Position> = after
            .iter()
            .map(|i| self.buffer.index_to_position(*i))
            .collect();
        let new_primary = positions
            .get(primary_index)
            .copied()
            .unwrap_or(Position::new(0, 0));
        self.cursor.move_to(self.buffer.clamp_position(new_primary));
        let others = positions
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != primary_index)
            .map(|(_, p)| self.buffer.clamp_position(p))
            .collect();
        self.cursors.set(others);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn editor_with(text: &str) -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(text);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    #[test]
    fn adding_a_cursor_below_and_typing_edits_both_lines() {
        let mut editor = editor_with("aa\nbb\ncc\n");
        assert!(editor.add_cursor_below());
        assert_eq!(editor.extra_cursors().len(), 1);
        editor.insert_char('X');
        assert_eq!(editor.buffer().text(), "Xaa\nXbb\ncc\n");
        assert_eq!(editor.cursor_position(), Position::new(0, 1));
    }

    #[test]
    fn adding_cursors_above_and_below_covers_three_lines() {
        let mut editor = editor_with("aa\nbb\ncc\n");
        editor.set_cursor_position(Position::new(1, 0));
        assert!(editor.add_cursor_above());
        assert!(editor.add_cursor_below());
        editor.insert_str("- ");
        assert_eq!(editor.buffer().text(), "- aa\n- bb\n- cc\n");
        assert_eq!(editor.all_cursors().len(), 3);
    }

    #[test]
    fn cursors_cannot_be_added_past_the_ends_of_the_document() {
        let mut editor = editor_with("only\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert!(!editor.add_cursor_above());
        // Line 1 is the empty line after the trailing newline, so one below
        // exists; the one after that does not.
        assert!(editor.add_cursor_below());
        assert!(!editor.add_cursor_below());
    }

    #[test]
    fn escape_drops_back_to_one_cursor() {
        let mut editor = editor_with("aa\nbb\n");
        editor.add_cursor_below();
        editor.clear_extra_cursors();
        editor.insert_char('X');
        assert_eq!(editor.buffer().text(), "Xaa\nbb\n");
    }

    #[test]
    fn adding_a_cursor_at_the_next_occurrence_walks_the_file() {
        let mut editor = editor_with("foo bar\nfoo baz\nfoo\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert!(editor.add_cursor_at_next_occurrence());
        assert_eq!(editor.extra_cursors().positions(), &[Position::new(1, 0)]);
        assert!(editor.add_cursor_at_next_occurrence());
        assert_eq!(
            editor.extra_cursors().positions(),
            &[Position::new(1, 0), Position::new(2, 0)]
        );
        // Every occurrence is taken, so there is nothing left to add.
        assert!(!editor.add_cursor_at_next_occurrence());
    }

    #[test]
    fn typing_at_every_occurrence_edits_them_all() {
        let mut editor = editor_with("foo\nfoo\nfoo\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert_eq!(editor.add_cursors_at_all_occurrences(), 2);
        editor.insert_str("my_");
        assert_eq!(editor.buffer().text(), "my_foo\nmy_foo\nmy_foo\n");
    }

    #[test]
    fn an_occurrence_search_with_no_word_under_the_cursor_does_nothing() {
        let mut editor = editor_with("   \n");
        editor.set_cursor_position(Position::new(0, 1));
        assert!(!editor.add_cursor_at_next_occurrence());
        assert_eq!(editor.add_cursors_at_all_occurrences(), 0);
    }

    #[test]
    fn backspacing_at_every_cursor_removes_one_character_each() {
        let mut editor = editor_with("abc\nabc\n");
        editor.set_cursor_position(Position::new(0, 2));
        assert!(editor.add_cursor_below());
        editor.backspace();
        assert_eq!(editor.buffer().text(), "ac\nac\n");
        assert_eq!(editor.cursor_position(), Position::new(0, 1));
    }

    #[test]
    fn backspacing_at_the_very_start_leaves_that_cursor_alone() {
        let mut editor = editor_with("ab\nab\n");
        editor.set_cursor_position(Position::new(0, 0));
        assert!(editor.add_cursor_below());
        editor.backspace();
        // The second cursor joined its line to the first; the first had nothing
        // before it to delete.
        assert_eq!(editor.buffer().text(), "abab\n");
    }

    #[test]
    fn a_multi_cursor_edit_is_a_single_undo_step() {
        let mut editor = editor_with("aa\nbb\ncc\n");
        editor.add_cursor_below();
        editor.add_cursor_below();
        editor.insert_char('X');
        assert_eq!(editor.buffer().text(), "Xaa\nXbb\nXcc\n");
        editor.undo();
        assert_eq!(editor.buffer().text(), "aa\nbb\ncc\n");
        assert!(
            editor.extra_cursors().is_empty(),
            "undo returns to a single cursor"
        );
    }

    #[test]
    fn a_read_only_document_refuses_multi_cursor_typing() {
        let mut editor = editor_with("aa\nbb\n");
        editor.add_cursor_below();
        editor.set_read_only(true);
        editor.insert_char('X');
        editor.backspace();
        assert_eq!(editor.buffer().text(), "aa\nbb\n");
    }

    #[test]
    fn a_cursor_cannot_be_added_on_top_of_the_primary_one() {
        let mut editor = editor_with("abc\n");
        editor.set_cursor_position(Position::new(0, 1));
        assert!(!editor.add_cursor_at(Position::new(0, 1)));
        assert!(editor.add_cursor_at(Position::new(0, 2)));
        assert!(!editor.add_cursor_at(Position::new(0, 2)));
    }

    #[test]
    fn typing_a_newline_at_two_cursors_splits_both_lines() {
        let mut editor = editor_with("ab\nab\n");
        editor.set_cursor_position(Position::new(0, 1));
        assert!(editor.add_cursor_below());
        editor.insert_char('\n');
        assert_eq!(editor.buffer().text(), "a\nb\na\nb\n");
    }
}
