//! Folding as the editor and the renderer see it.
//!
//! [`fold`](super::fold) decides where the regions are and which are collapsed.
//! This file answers the two questions the rest of the program asks: which
//! buffer lines does a frame draw, and where may the cursor stand.

use super::fold::{FoldRange, FoldState};
use super::{Editor, Position};

impl FoldState {
    /// Returns the outermost collapsed region hiding `line`, if any.
    #[must_use]
    pub fn hiding_region(&self, line: usize) -> Option<&FoldRange> {
        self.ranges()
            .iter()
            .filter(|r| self.is_collapsed(r.start_line) && r.hides(line))
            .min_by_key(|r| r.start_line)
    }

    /// Returns the line that stands in for `line` on screen.
    ///
    /// A visible line is its own anchor; a hidden one is represented by the
    /// first line of the outermost collapsed region covering it.
    #[must_use]
    pub fn visible_anchor(&self, line: usize) -> Option<usize> {
        Some(self.hiding_region(line).map_or(line, |r| r.start_line))
    }

    /// Returns the region a toggle at `line` would act on.
    #[must_use]
    pub fn target_start(&self, line: usize) -> Option<usize> {
        self.region_at(line).map(|r| r.start_line)
    }

    /// Returns how many lines are hidden by the region collapsed at `line`.
    ///
    /// Zero when nothing is collapsed there.
    #[must_use]
    pub fn hidden_below(&self, line: usize) -> usize {
        if !self.is_collapsed(line) {
            return 0;
        }
        self.ranges()
            .iter()
            .filter(|r| r.start_line == line)
            .map(|r| r.end_line - r.start_line)
            .min()
            .unwrap_or(0)
    }

    /// Collapses the region at `line`. Returns false when there is none, or it
    /// was already collapsed.
    pub fn fold(&mut self, line: usize) -> bool {
        match self.target_start(line) {
            Some(start) if !self.is_collapsed(start) => self.toggle(line),
            _ => false,
        }
    }

    /// Expands the region at `line`. Returns false when there is none, or it
    /// was already expanded.
    pub fn unfold(&mut self, line: usize) -> bool {
        match self.target_start(line) {
            Some(start) if self.is_collapsed(start) => self.toggle(line),
            _ => false,
        }
    }

    /// Returns the first line at or after `from` that is not hidden.
    #[must_use]
    pub fn first_visible_at_or_after(&self, from: usize, total: usize) -> usize {
        let mut line = from;
        while line < total && self.is_hidden(line) {
            line += 1;
        }
        line.min(total.saturating_sub(1))
    }

    /// Returns up to `count` visible lines starting at or after `from`.
    #[must_use]
    pub fn visible_window(&self, from: usize, count: usize, total: usize) -> Vec<usize> {
        let mut out = Vec::with_capacity(count);
        let mut line = from;
        while line < total && out.len() < count {
            if !self.is_hidden(line) {
                out.push(line);
            }
            line += 1;
        }
        out
    }
}

impl Editor {
    /// Collapses or expands the region under the cursor.
    ///
    /// Returns false when the cursor is not inside any foldable region.
    pub fn toggle_fold(&mut self) -> bool {
        self.ensure_folds();
        let line = self.cursor.position().line;
        let toggled = self.folds.toggle(line);
        if toggled {
            self.clamp_cursor_out_of_folds();
        }
        toggled
    }

    /// Collapses the region under the cursor.
    pub fn fold_current(&mut self) -> bool {
        self.ensure_folds();
        let line = self.cursor.position().line;
        let folded = self.folds.fold(line);
        if folded {
            self.clamp_cursor_out_of_folds();
        }
        folded
    }

    /// Expands the region under the cursor.
    pub fn unfold_current(&mut self) -> bool {
        self.ensure_folds();
        let line = self.cursor.position().line;
        self.folds.unfold(line)
    }

    /// Collapses every region in the document.
    pub fn fold_all(&mut self) {
        self.ensure_folds();
        self.folds.fold_all();
        self.clamp_cursor_out_of_folds();
    }

    /// Expands every region in the document.
    pub fn unfold_all(&mut self) {
        self.folds.unfold_all();
    }

    /// Returns true when `line` is hidden inside a collapsed region.
    #[must_use]
    pub fn is_line_hidden(&self, line: usize) -> bool {
        self.folds.is_hidden(line)
    }

    /// Returns how many lines the fold starting at `line` hides.
    #[must_use]
    pub fn folded_line_count(&self, line: usize) -> usize {
        self.folds.hidden_below(line)
    }

    /// Returns the buffer lines a frame should draw, top to bottom.
    ///
    /// With nothing collapsed this is the viewport's line range. With folds it
    /// skips the hidden lines, so a collapsed region occupies exactly one row.
    #[must_use]
    pub fn screen_lines(&self) -> Vec<usize> {
        let total = self.buffer.len_lines();
        let top = self
            .folds
            .first_visible_at_or_after(self.view.scroll_top(), total);
        self.folds.visible_window(top, self.view.height(), total)
    }

    /// Returns the screen row `line` is drawn on, if it is on screen.
    #[must_use]
    pub fn screen_row_of(&self, line: usize) -> Option<u16> {
        self.screen_lines()
            .iter()
            .position(|l| *l == line)
            .and_then(|row| u16::try_from(row).ok())
    }

    /// Moves the cursor down one *visible* line, stepping over folds.
    pub fn move_down_visible(&mut self) {
        let pos = self.cursor.position();
        let total = self.buffer.len_lines();
        let mut line = pos.line + 1;
        while line < total && self.folds.is_hidden(line) {
            line += 1;
        }
        if line < total {
            let col = pos.col.min(self.buffer.line_len_chars(line));
            self.cursor.set_position(Position::new(line, col));
            self.ensure_cursor_visible();
        }
    }

    /// Moves the cursor up one *visible* line, stepping over folds.
    pub fn move_up_visible(&mut self) {
        let pos = self.cursor.position();
        let mut line = pos.line;
        while line > 0 {
            line -= 1;
            if !self.folds.is_hidden(line) {
                let col = pos.col.min(self.buffer.line_len_chars(line));
                self.cursor.set_position(Position::new(line, col));
                self.ensure_cursor_visible();
                return;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::highlight::Language;

    const SRC: &str = "fn f() {\n    if a {\n        g();\n    }\n}\nlet z = 1;\n";

    fn folded_editor() -> Editor {
        let mut editor = Editor::new(80, 24);
        editor.insert_str(SRC);
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    #[test]
    fn a_collapsed_region_draws_as_one_line() {
        let mut editor = folded_editor();
        let before = editor.screen_lines();
        assert!(before.contains(&2));

        assert!(editor.toggle_fold());
        let after = editor.screen_lines();
        assert!(after.contains(&0), "the fold's first line stays visible");
        assert!(!after.contains(&1), "the body is hidden");
        assert!(!after.contains(&4));
        assert!(after.contains(&5), "text after the region still draws");
        assert_eq!(editor.folded_line_count(0), 4);
    }

    #[test]
    fn unfolding_restores_every_line() {
        let mut editor = folded_editor();
        let before = editor.screen_lines();
        editor.toggle_fold();
        editor.toggle_fold();
        assert_eq!(editor.screen_lines(), before);
        assert_eq!(editor.folded_line_count(0), 0);
    }

    #[test]
    fn fold_all_and_unfold_all_cover_the_document() {
        let mut editor = folded_editor();
        editor.fold_all();
        assert!(editor.is_line_hidden(2));
        editor.unfold_all();
        assert!(!editor.is_line_hidden(2));
    }

    #[test]
    fn the_cursor_cannot_stay_inside_a_region_that_collapses() {
        let mut editor = folded_editor();
        editor.set_cursor_position(Position::new(2, 4));
        // Folding the outer region from the inside collapses the inner one, so
        // fold the outer one explicitly.
        assert!(editor.folds_mut().fold(0));
        editor.clamp_cursor_out_of_folds();
        assert_eq!(editor.cursor_position().line, 0);
        assert!(!editor.is_line_hidden(editor.cursor_position().line));
    }

    #[test]
    fn moving_down_steps_over_a_collapsed_region() {
        let mut editor = folded_editor();
        assert!(editor.folds_mut().fold(0));
        editor.set_cursor_position(Position::new(0, 0));
        editor.move_down_visible();
        assert_eq!(editor.cursor_position().line, 5);
        editor.move_up_visible();
        assert_eq!(editor.cursor_position().line, 0);
    }

    #[test]
    fn moving_past_the_ends_of_the_buffer_stays_put() {
        let mut editor = folded_editor();
        editor.set_cursor_position(Position::new(0, 0));
        editor.move_up_visible();
        assert_eq!(editor.cursor_position().line, 0);

        let last = editor.buffer().len_lines() - 1;
        editor.set_cursor_position(Position::new(last, 0));
        editor.move_down_visible();
        assert_eq!(editor.cursor_position().line, last);
    }

    #[test]
    fn fold_and_unfold_report_whether_they_changed_anything() {
        let mut editor = folded_editor();
        assert!(editor.fold_current());
        assert!(!editor.fold_current(), "already folded");
        assert!(editor.unfold_current());
        assert!(!editor.unfold_current(), "already unfolded");
    }

    #[test]
    fn a_line_with_no_region_cannot_be_folded() {
        let mut editor = Editor::new(80, 24);
        editor.insert_str("let a = 1;\n");
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        assert!(!editor.toggle_fold());
        assert!(!editor.fold_current());
        assert_eq!(editor.screen_row_of(0), Some(0));
    }

    #[test]
    fn screen_row_of_skips_hidden_lines() {
        let mut editor = folded_editor();
        assert!(editor.folds_mut().fold(0));
        assert_eq!(editor.screen_row_of(0), Some(0));
        assert_eq!(editor.screen_row_of(2), None);
        assert_eq!(editor.screen_row_of(5), Some(1));
    }

    #[test]
    fn a_window_past_the_end_of_the_buffer_is_short_not_wrong() {
        let state = FoldState::default();
        assert_eq!(state.visible_window(0, 10, 3), vec![0, 1, 2]);
        assert!(state.visible_window(9, 10, 3).is_empty());
        assert_eq!(state.first_visible_at_or_after(0, 3), 0);
        assert_eq!(state.visible_anchor(2), Some(2));
        assert_eq!(state.hidden_below(0), 0);
        assert_eq!(state.target_start(0), None);
    }
}
