//! Driving search and replace from the editor.
//!
//! The find bar's own rules live in [`search`](super::search); this file wires
//! them to the buffer and the cursor, and turns the keys the bar accepts into
//! those calls so `input_editor.rs` only has to translate key events.

use super::search::{SearchDirection, SearchState};
use super::{Editor, Position};

/// A key the find bar understands.
///
/// Deliberately not crossterm's `KeyEvent`: the bar has to be drivable from a
/// test without a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKey {
    /// A printable character typed into the focused field.
    Char(char),
    /// Delete the character before the caret.
    Backspace,
    /// Move to the next match in the bar's direction.
    Enter,
    /// Move to the previous match.
    ShiftEnter,
    /// Switch between the find and replace fields.
    Tab,
    /// Replace the current match.
    Replace,
    /// Replace every match.
    ReplaceAll,
    /// Turn case sensitivity on or off.
    ToggleCase,
    /// Close the bar.
    Escape,
}

/// What the editor did with a find-bar key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOutcome {
    /// The bar consumed the key and stays open.
    Handled,
    /// The bar closed.
    Closed,
    /// The bar is not open, so the key was not consumed.
    Ignored,
}

impl Editor {
    /// Returns the find bar's state.
    #[must_use]
    pub const fn search(&self) -> &SearchState {
        &self.search
    }

    /// Returns the find bar's state for mutation.
    pub fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    /// Opens the find bar, seeding it from the selection when there is one.
    pub fn open_search(&mut self, replacing: bool) {
        if let Some(selected) = self.selected_text()
            && !selected.contains('\n')
            && !selected.is_empty()
        {
            self.search.set_query(selected);
        }
        self.search.open(replacing);
        self.refresh_search();
        self.search.select_from(self.cursor.position());
        self.focus_current_match();
    }

    /// Closes the find bar.
    pub fn close_search(&mut self) {
        self.search.close();
    }

    /// Recomputes the match list if the buffer or the query changed.
    pub fn refresh_search(&mut self) {
        if self.search.is_stale() {
            let mut search = std::mem::take(&mut self.search);
            search.refresh(&self.buffer);
            self.search = search;
        }
    }

    /// Moves the cursor onto the current match, if there is one.
    fn focus_current_match(&mut self) {
        if let Some(pos) = self.search.current_match() {
            self.cursor.move_to(self.buffer.clamp_position(pos));
            self.clamp_cursor_out_of_folds();
            self.ensure_cursor_visible();
        }
    }

    /// Moves to the next match, wrapping at the end of the document.
    pub fn search_next(&mut self) -> Option<Position> {
        self.refresh_search();
        let found = self.search.next_match();
        self.focus_current_match();
        found
    }

    /// Moves to the previous match, wrapping at the start of the document.
    pub fn search_prev(&mut self) -> Option<Position> {
        self.refresh_search();
        let found = self.search.prev_match();
        self.focus_current_match();
        found
    }

    /// Replaces the current match and moves to the next one.
    ///
    /// Returns false when there is nothing selected to replace, or the document
    /// is read-only.
    pub fn replace_current(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        self.refresh_search();
        let Some(start) = self.search.current_match() else {
            return false;
        };
        let width = self.search.query_len();
        if width == 0 {
            return false;
        }
        let end_index = self.buffer.position_to_index(start) + width;
        let end = self.buffer.index_to_position(end_index);
        let replacement = self.search.replacement().to_string();

        self.buffer.begin_undo_group();
        self.apply_delete(start, end);
        let after = self.apply_insert(start, &replacement);
        self.buffer.end_undo_group();
        self.cursor.move_to(after);

        self.search.mark_stale();
        self.after_edit();
        self.refresh_search();
        self.search.select_from(after);
        true
    }

    /// Replaces every match in the document as one undo step.
    ///
    /// Returns how many were replaced.
    pub fn replace_all(&mut self) -> usize {
        if self.read_only {
            return 0;
        }
        self.refresh_search();
        let width = self.search.query_len();
        if width == 0 || self.search.match_count() == 0 {
            return 0;
        }
        let replacement = self.search.replacement().to_string();
        let mut positions: Vec<Position> = self.search.matches().to_vec();
        // Applying from the end keeps the earlier positions valid.
        positions.sort_by_key(|p| std::cmp::Reverse((p.line, p.col)));

        self.buffer.begin_undo_group();
        for start in &positions {
            let end_index = self.buffer.position_to_index(*start) + width;
            let end = self.buffer.index_to_position(end_index);
            self.apply_delete(*start, end);
            self.apply_insert(*start, &replacement);
        }
        self.buffer.end_undo_group();

        let count = positions.len();
        self.cursor.clamp(&self.buffer);
        self.search.mark_stale();
        self.after_edit();
        self.refresh_search();
        count
    }

    /// Feeds one key to the find bar.
    pub fn feed_search_key(&mut self, key: SearchKey) -> SearchOutcome {
        if !self.search.is_active() {
            return SearchOutcome::Ignored;
        }
        match key {
            SearchKey::Escape => {
                self.close_search();
                return SearchOutcome::Closed;
            }
            SearchKey::Char(c) => {
                self.search.push_char(c);
                self.refresh_search();
                self.search.select_from(self.cursor.position());
                self.focus_current_match();
            }
            SearchKey::Backspace => {
                self.search.pop_char();
                self.refresh_search();
                self.focus_current_match();
            }
            SearchKey::Tab => self.search.toggle_field(),
            SearchKey::Enter => {
                self.search.set_direction(SearchDirection::Forward);
                self.search_next();
            }
            SearchKey::ShiftEnter => {
                self.search.set_direction(SearchDirection::Backward);
                self.search_prev();
            }
            SearchKey::Replace => {
                self.replace_current();
            }
            SearchKey::ReplaceAll => {
                let count = self.replace_all();
                self.set_status(format!("Replaced {count}"));
            }
            SearchKey::ToggleCase => {
                let on = !self.search.is_case_sensitive();
                self.search.set_case_sensitive(on);
                self.refresh_search();
                self.focus_current_match();
            }
        }
        SearchOutcome::Handled
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

    fn type_query(editor: &mut Editor, query: &str) {
        for c in query.chars() {
            editor.feed_search_key(SearchKey::Char(c));
        }
    }

    #[test]
    fn opening_the_bar_counts_the_matches_and_moves_to_the_first() {
        let mut editor = editor_with("cat dog cat\n");
        editor.open_search(false);
        type_query(&mut editor, "cat");
        assert_eq!(editor.search().match_count(), 2);
        assert_eq!(editor.search().count_label(), "1/2");
        assert_eq!(editor.cursor_position(), Position::new(0, 0));
    }

    #[test]
    fn enter_moves_forward_and_wraps_at_the_end() {
        let mut editor = editor_with("a\na\na\n");
        editor.open_search(false);
        type_query(&mut editor, "a");
        assert_eq!(editor.cursor_position(), Position::new(0, 0));
        editor.feed_search_key(SearchKey::Enter);
        assert_eq!(editor.cursor_position(), Position::new(1, 0));
        editor.feed_search_key(SearchKey::Enter);
        assert_eq!(editor.cursor_position(), Position::new(2, 0));
        editor.feed_search_key(SearchKey::Enter);
        assert_eq!(editor.cursor_position(), Position::new(0, 0));
        assert!(editor.search().wrapped());
    }

    #[test]
    fn shift_enter_moves_back_and_wraps_at_the_start() {
        let mut editor = editor_with("a\na\na\n");
        editor.open_search(false);
        type_query(&mut editor, "a");
        editor.feed_search_key(SearchKey::ShiftEnter);
        assert_eq!(editor.cursor_position(), Position::new(2, 0));
        assert!(editor.search().wrapped());
        editor.feed_search_key(SearchKey::ShiftEnter);
        assert_eq!(editor.cursor_position(), Position::new(1, 0));
        assert!(!editor.search().wrapped());
    }

    #[test]
    fn replacing_one_match_leaves_the_rest_alone() {
        let mut editor = editor_with("cat cat cat\n");
        editor.open_search(true);
        type_query(&mut editor, "cat");
        editor.feed_search_key(SearchKey::Tab);
        type_query(&mut editor, "dog");
        assert!(editor.replace_current());
        assert_eq!(editor.buffer().text(), "dog cat cat\n");
        assert_eq!(editor.search().match_count(), 2);
    }

    #[test]
    fn replacing_all_matches_is_one_undo_step() {
        let mut editor = editor_with("cat cat\ncat\n");
        editor.open_search(true);
        type_query(&mut editor, "cat");
        editor.feed_search_key(SearchKey::Tab);
        type_query(&mut editor, "bird");
        assert_eq!(editor.replace_all(), 3);
        assert_eq!(editor.buffer().text(), "bird bird\nbird\n");
        editor.undo();
        assert_eq!(editor.buffer().text(), "cat cat\ncat\n");
    }

    #[test]
    fn replacing_with_nothing_deletes_the_matches() {
        let mut editor = editor_with("a-b-c\n");
        editor.open_search(true);
        type_query(&mut editor, "-");
        assert_eq!(editor.replace_all(), 2);
        assert_eq!(editor.buffer().text(), "abc\n");
    }

    #[test]
    fn replacing_with_no_match_reports_failure() {
        let mut editor = editor_with("hello\n");
        editor.open_search(true);
        type_query(&mut editor, "zzz");
        assert!(!editor.replace_current());
        assert_eq!(editor.replace_all(), 0);
        assert_eq!(editor.buffer().text(), "hello\n");
    }

    #[test]
    fn a_read_only_document_refuses_to_replace() {
        let mut editor = editor_with("cat\n");
        editor.set_read_only(true);
        editor.open_search(true);
        type_query(&mut editor, "cat");
        assert!(!editor.replace_current());
        assert_eq!(editor.replace_all(), 0);
        assert_eq!(editor.buffer().text(), "cat\n");
    }

    #[test]
    fn case_sensitivity_can_be_toggled_from_the_bar() {
        let mut editor = editor_with("Cat cat\n");
        editor.open_search(false);
        type_query(&mut editor, "cat");
        assert_eq!(editor.search().match_count(), 2);
        editor.feed_search_key(SearchKey::ToggleCase);
        assert_eq!(editor.search().match_count(), 1);
    }

    #[test]
    fn escape_closes_the_bar_and_later_keys_are_ignored() {
        let mut editor = editor_with("cat\n");
        editor.open_search(false);
        assert_eq!(
            editor.feed_search_key(SearchKey::Escape),
            SearchOutcome::Closed
        );
        assert!(!editor.search().is_active());
        assert_eq!(
            editor.feed_search_key(SearchKey::Char('x')),
            SearchOutcome::Ignored
        );
    }

    #[test]
    fn backspacing_the_query_recounts() {
        let mut editor = editor_with("cat car\n");
        editor.open_search(false);
        type_query(&mut editor, "cat");
        assert_eq!(editor.search().match_count(), 1);
        editor.feed_search_key(SearchKey::Backspace);
        assert_eq!(editor.search().match_count(), 2);
    }

    #[test]
    fn the_bar_seeds_itself_from_the_selection() {
        let mut editor = editor_with("alpha beta\n");
        editor.set_cursor_position(Position::new(0, 0));
        editor.cursor_mut().start_selection();
        editor.cursor_mut().extend_to(Position::new(0, 5));
        editor.open_search(false);
        assert_eq!(editor.search().query(), "alpha");
        assert_eq!(editor.search().match_count(), 1);
    }

    #[test]
    fn editing_after_a_search_recounts_on_the_next_move() {
        let mut editor = editor_with("cat cat\n");
        editor.open_search(false);
        type_query(&mut editor, "cat");
        assert_eq!(editor.search().match_count(), 2);

        editor.set_cursor_position(Position::new(0, 7));
        editor.insert_str(" cat");
        editor.search_next();
        assert_eq!(editor.search().match_count(), 3);
    }

    #[test]
    fn replace_all_from_the_bar_reports_the_count_in_the_status_line() {
        let mut editor = editor_with("x x x\n");
        editor.open_search(true);
        type_query(&mut editor, "x");
        editor.feed_search_key(SearchKey::Tab);
        type_query(&mut editor, "y");
        editor.feed_search_key(SearchKey::ReplaceAll);
        assert_eq!(editor.buffer().text(), "y y y\n");
        assert_eq!(editor.status(), "Replaced 3");
    }
}
