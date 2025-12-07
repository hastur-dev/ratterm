//! Terminal text selection support.
//!
//! Provides mouse and keyboard-based text selection in the terminal grid.

/// Selection mode determines how text is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// Character-by-character selection (default).
    #[default]
    Normal,
    /// Select entire lines.
    Line,
    /// Select rectangular block.
    Block,
}

/// Terminal text selection state.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Start position (col, row) in grid coordinates.
    start: (u16, u16),
    /// End position (col, row) in grid coordinates.
    end: (u16, u16),
    /// Whether selection is actively being made (mouse held).
    active: bool,
    /// Selection mode.
    mode: SelectionMode,
}

impl Selection {
    /// Creates a new selection starting at the given position.
    #[must_use]
    pub fn new(col: u16, row: u16) -> Self {
        Self {
            start: (col, row),
            end: (col, row),
            active: true,
            mode: SelectionMode::Normal,
        }
    }

    /// Creates a new selection with a specific mode.
    #[must_use]
    pub fn with_mode(col: u16, row: u16, mode: SelectionMode) -> Self {
        Self {
            start: (col, row),
            end: (col, row),
            active: true,
            mode,
        }
    }

    /// Returns the start position.
    #[must_use]
    pub const fn start(&self) -> (u16, u16) {
        self.start
    }

    /// Returns the end position.
    #[must_use]
    pub const fn end(&self) -> (u16, u16) {
        self.end
    }

    /// Returns the selection mode.
    #[must_use]
    pub const fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Returns whether the selection is actively being made.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Updates the end position of the selection.
    pub fn update(&mut self, col: u16, row: u16) {
        self.end = (col, row);
    }

    /// Finalizes the selection (mouse released).
    pub fn finalize(&mut self) {
        self.active = false;
    }

    /// Returns the normalized range (start before end).
    /// Returns ((start_col, start_row), (end_col, end_row)).
    #[must_use]
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        let (start, end) = (self.start, self.end);

        // Determine which position comes first
        if start.1 < end.1 || (start.1 == end.1 && start.0 <= end.0) {
            (start, end)
        } else {
            (end, start)
        }
    }

    /// Checks if the given cell position is within the selection.
    #[must_use]
    pub fn contains(&self, col: u16, row: u16) -> bool {
        let ((start_col, start_row), (end_col, end_row)) = self.normalized();

        match self.mode {
            SelectionMode::Normal => {
                // Normal selection: flows like text
                if row < start_row || row > end_row {
                    return false;
                }
                if row == start_row && row == end_row {
                    // Single line selection
                    col >= start_col && col <= end_col
                } else if row == start_row {
                    // First line of multi-line selection
                    col >= start_col
                } else if row == end_row {
                    // Last line of multi-line selection
                    col <= end_col
                } else {
                    // Middle lines are fully selected
                    true
                }
            }
            SelectionMode::Line => {
                // Line selection: entire lines
                row >= start_row && row <= end_row
            }
            SelectionMode::Block => {
                // Block selection: rectangular region
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);
                col >= min_col && col <= max_col && row >= start_row && row <= end_row
            }
        }
    }

    /// Returns whether the selection is empty (start == end).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extends the selection left by one character.
    pub fn extend_left(&mut self) {
        if self.end.0 > 0 {
            self.end.0 -= 1;
        }
    }

    /// Extends the selection right by one character.
    pub fn extend_right(&mut self, max_col: u16) {
        if self.end.0 < max_col.saturating_sub(1) {
            self.end.0 += 1;
        }
    }

    /// Extends the selection up by one row.
    pub fn extend_up(&mut self) {
        if self.end.1 > 0 {
            self.end.1 -= 1;
        }
    }

    /// Extends the selection down by one row.
    pub fn extend_down(&mut self, max_row: u16) {
        if self.end.1 < max_row.saturating_sub(1) {
            self.end.1 += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_new() {
        let sel = Selection::new(5, 10);
        assert_eq!(sel.start(), (5, 10));
        assert_eq!(sel.end(), (5, 10));
        assert!(sel.is_active());
        assert!(sel.is_empty());
    }

    #[test]
    fn test_selection_update() {
        let mut sel = Selection::new(0, 0);
        sel.update(10, 5);
        assert_eq!(sel.end(), (10, 5));
        assert!(!sel.is_empty());
    }

    #[test]
    fn test_selection_contains_single_line() {
        let mut sel = Selection::new(5, 2);
        sel.update(15, 2);

        // Within selection
        assert!(sel.contains(5, 2));
        assert!(sel.contains(10, 2));
        assert!(sel.contains(15, 2));

        // Outside selection
        assert!(!sel.contains(4, 2));
        assert!(!sel.contains(16, 2));
        assert!(!sel.contains(10, 1));
        assert!(!sel.contains(10, 3));
    }

    #[test]
    fn test_selection_contains_multi_line() {
        let mut sel = Selection::new(10, 1);
        sel.update(5, 3);

        // First line: from col 10 onwards
        assert!(!sel.contains(9, 1));
        assert!(sel.contains(10, 1));
        assert!(sel.contains(50, 1));

        // Middle line: all columns
        assert!(sel.contains(0, 2));
        assert!(sel.contains(50, 2));

        // Last line: up to col 5
        assert!(sel.contains(0, 3));
        assert!(sel.contains(5, 3));
        assert!(!sel.contains(6, 3));
    }

    #[test]
    fn test_selection_normalized() {
        // Forward selection
        let mut sel = Selection::new(0, 0);
        sel.update(10, 5);
        let (start, end) = sel.normalized();
        assert_eq!(start, (0, 0));
        assert_eq!(end, (10, 5));

        // Backward selection
        let mut sel = Selection::new(10, 5);
        sel.update(0, 0);
        let (start, end) = sel.normalized();
        assert_eq!(start, (0, 0));
        assert_eq!(end, (10, 5));
    }

    #[test]
    fn test_selection_extend() {
        let mut sel = Selection::new(5, 5);

        sel.extend_left();
        assert_eq!(sel.end(), (4, 5));

        sel.extend_right(80);
        assert_eq!(sel.end(), (5, 5));

        sel.extend_up();
        assert_eq!(sel.end(), (5, 4));

        sel.extend_down(24);
        assert_eq!(sel.end(), (5, 5));
    }

    #[test]
    fn test_selection_line_mode() {
        let mut sel = Selection::with_mode(5, 2, SelectionMode::Line);
        sel.update(10, 4);

        // Line mode selects entire lines
        assert!(sel.contains(0, 2));
        assert!(sel.contains(100, 2));
        assert!(sel.contains(0, 3));
        assert!(sel.contains(0, 4));
        assert!(!sel.contains(0, 1));
        assert!(!sel.contains(0, 5));
    }

    #[test]
    fn test_selection_block_mode() {
        let mut sel = Selection::with_mode(5, 2, SelectionMode::Block);
        sel.update(10, 4);

        // Block mode selects a rectangle
        assert!(sel.contains(5, 2));
        assert!(sel.contains(10, 4));
        assert!(sel.contains(7, 3));
        assert!(!sel.contains(4, 3));
        assert!(!sel.contains(11, 3));
    }
}
