//! Terminal grid (cell buffer) for storing terminal content.
//!
//! Provides a 2D grid of cells with character and style information.

use unicode_width::UnicodeWidthChar;

use super::cell::{Cell, CursorShape, Row};
use super::selection::{Selection, SelectionMode};
use super::style::Style;

/// Default tab stop width.
const DEFAULT_TAB_WIDTH: u16 = 8;

/// Terminal grid storing the visible content.
pub struct Grid {
    /// Rows in the grid.
    rows: Vec<Row>,
    /// Number of columns.
    cols: u16,
    /// Number of visible rows.
    visible_rows: u16,
    /// Cursor column position (0-indexed).
    cursor_col: u16,
    /// Cursor row position (0-indexed).
    cursor_row: u16,
    /// Saved cursor position.
    saved_cursor: Option<(u16, u16)>,
    /// Current text style.
    current_style: Style,
    /// Cursor visibility.
    cursor_visible: bool,
    /// Cursor shape.
    cursor_shape: CursorShape,
    /// Alternate screen buffer.
    alternate_screen: Option<Vec<Row>>,
    /// Alternate cursor position.
    alternate_cursor: Option<(u16, u16)>,
    /// Scrollback buffer.
    scrollback: Vec<Row>,
    /// Maximum scrollback lines.
    scrollback_limit: usize,
    /// Current text selection.
    selection: Option<Selection>,
}

impl Grid {
    /// Creates a new grid with the given dimensions.
    ///
    /// # Panics
    /// Panics if cols or rows is zero.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        assert!(cols > 0, "Grid columns must be positive");
        assert!(rows > 0, "Grid rows must be positive");

        let grid_rows: Vec<Row> = (0..rows).map(|_| Row::new(cols)).collect();

        Self {
            rows: grid_rows,
            cols,
            visible_rows: rows,
            cursor_col: 0,
            cursor_row: 0,
            saved_cursor: None,
            current_style: Style::new(),
            cursor_visible: true,
            cursor_shape: CursorShape::Block,
            alternate_screen: None,
            alternate_cursor: None,
            scrollback: Vec::new(),
            scrollback_limit: 10_000,
            selection: None,
        }
    }

    /// Returns the number of columns.
    #[must_use]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    /// Returns the number of visible rows.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.visible_rows
    }

    /// Returns the cursor position as (col, row).
    #[must_use]
    pub const fn cursor_pos(&self) -> (u16, u16) {
        (self.cursor_col, self.cursor_row)
    }

    /// Returns the cell at the given position.
    #[must_use]
    pub fn cell(&self, col: u16, row: u16) -> Option<&Cell> {
        self.rows.get(row as usize).and_then(|r| r.cell(col))
    }

    /// Returns a mutable cell at the given position.
    fn cell_mut(&mut self, col: u16, row: u16) -> Option<&mut Cell> {
        self.rows
            .get_mut(row as usize)
            .and_then(|r| r.cell_mut(col))
    }

    /// Sets the cursor position.
    pub fn set_cursor_pos(&mut self, col: u16, row: u16) {
        self.cursor_col = col.min(self.cols.saturating_sub(1));
        self.cursor_row = row.min(self.visible_rows.saturating_sub(1));
    }

    /// Moves cursor right by n columns.
    pub fn move_cursor_right(&mut self, n: u16) {
        self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
    }

    /// Moves cursor left by n columns.
    pub fn move_cursor_left(&mut self, n: u16) {
        self.cursor_col = self.cursor_col.saturating_sub(n);
    }

    /// Moves cursor down by n rows.
    pub fn move_cursor_down(&mut self, n: u16) {
        self.cursor_row = (self.cursor_row + n).min(self.visible_rows.saturating_sub(1));
    }

    /// Moves cursor up by n rows.
    pub fn move_cursor_up(&mut self, n: u16) {
        self.cursor_row = self.cursor_row.saturating_sub(n);
    }

    /// Sets the current text style.
    pub fn set_style(&mut self, style: Style) {
        self.current_style = style;
    }

    /// Writes a character at the current cursor position.
    pub fn write_char(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        if width == 0 {
            return;
        }

        // Check if we need to wrap
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.visible_rows {
                self.scroll_up(1);
                self.cursor_row = self.visible_rows.saturating_sub(1);
            }
        }

        // Copy style before borrowing
        let style = self.current_style;
        let cursor_col = self.cursor_col;
        let cursor_row = self.cursor_row;

        // Write the character
        if let Some(cell) = self.cell_mut(cursor_col, cursor_row) {
            cell.set_char(c);
            cell.set_style(style);
        }

        self.cursor_col += 1;

        // Handle wide characters
        if width > 1 && self.cursor_col < self.cols {
            let cursor_col = self.cursor_col;
            let cursor_row = self.cursor_row;
            if let Some(cell) = self.cell_mut(cursor_col, cursor_row) {
                cell.set_wide_continuation();
                cell.set_style(style);
            }
            self.cursor_col += 1;
        }
    }

    /// Handles newline (LF).
    pub fn newline(&mut self) {
        self.cursor_row += 1;
        if self.cursor_row >= self.visible_rows {
            self.scroll_up(1);
            self.cursor_row = self.visible_rows.saturating_sub(1);
        }
        self.cursor_col = 0;
    }

    /// Handles carriage return (CR).
    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    /// Handles tab.
    pub fn tab(&mut self) {
        let next_tab = ((self.cursor_col / DEFAULT_TAB_WIDTH) + 1) * DEFAULT_TAB_WIDTH;
        self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
    }

    /// Handles backspace.
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    /// Scrolls the grid up by n lines.
    pub fn scroll_up(&mut self, n: u16) {
        let n = n.min(self.visible_rows) as usize;

        // Move top lines to scrollback
        for row in self.rows.drain(..n) {
            self.scrollback.push(row);
        }

        // Trim scrollback if needed
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.remove(0);
        }

        // Add new empty lines at bottom
        for _ in 0..n {
            self.rows.push(Row::new(self.cols));
        }
    }

    /// Scrolls the grid down by n lines.
    pub fn scroll_down(&mut self, n: u16) {
        let n = n.min(self.visible_rows) as usize;

        // Remove lines from bottom
        for _ in 0..n {
            self.rows.pop();
        }

        // Add new empty lines at top
        for _ in 0..n {
            self.rows.insert(0, Row::new(self.cols));
        }
    }

    /// Clears the entire grid.
    pub fn clear(&mut self) {
        for row in &mut self.rows {
            row.clear();
        }
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    /// Clears from cursor to end of line.
    pub fn clear_to_eol(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cursor_row as usize) {
            row.clear_from(self.cursor_col);
        }
    }

    /// Clears from start of line to cursor.
    pub fn clear_to_bol(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cursor_row as usize) {
            row.clear_to(self.cursor_col + 1);
        }
    }

    /// Clears entire current line.
    pub fn clear_line(&mut self) {
        if let Some(row) = self.rows.get_mut(self.cursor_row as usize) {
            row.clear();
        }
    }

    /// Clears from cursor to end of screen.
    pub fn clear_to_eos(&mut self) {
        self.clear_to_eol();
        let start_row = (self.cursor_row + 1) as usize;
        for row in self.rows.iter_mut().skip(start_row) {
            row.clear();
        }
    }

    /// Clears from start of screen to cursor.
    pub fn clear_to_bos(&mut self) {
        self.clear_to_bol();
        for row in self.rows.iter_mut().take(self.cursor_row as usize) {
            row.clear();
        }
    }

    /// Resizes the grid to new dimensions.
    pub fn resize(&mut self, new_cols: u16, new_rows: u16) {
        assert!(new_cols > 0, "Columns must be positive");
        assert!(new_rows > 0, "Rows must be positive");

        // Resize existing rows
        for row in &mut self.rows {
            row.resize(new_cols);
        }

        // Add or remove rows
        while self.rows.len() < new_rows as usize {
            self.rows.push(Row::new(new_cols));
        }
        self.rows.truncate(new_rows as usize);

        self.cols = new_cols;
        self.visible_rows = new_rows;

        // Clamp cursor
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
    }

    /// Returns cursor visibility.
    #[must_use]
    pub const fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Sets cursor visibility.
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Returns cursor shape.
    #[must_use]
    pub const fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Sets cursor shape.
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
    }

    /// Saves cursor position.
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_col, self.cursor_row));
    }

    /// Restores cursor position.
    pub fn restore_cursor(&mut self) {
        if let Some((col, row)) = self.saved_cursor {
            self.set_cursor_pos(col, row);
        }
    }

    /// Enters alternate screen buffer.
    pub fn enter_alternate_screen(&mut self) {
        if self.alternate_screen.is_none() {
            let main_screen = std::mem::replace(
                &mut self.rows,
                (0..self.visible_rows)
                    .map(|_| Row::new(self.cols))
                    .collect(),
            );
            self.alternate_screen = Some(main_screen);
            self.alternate_cursor = Some((self.cursor_col, self.cursor_row));
            self.cursor_col = 0;
            self.cursor_row = 0;
        }
    }

    /// Exits alternate screen buffer.
    pub fn exit_alternate_screen(&mut self) {
        if let Some(main_screen) = self.alternate_screen.take() {
            self.rows = main_screen;
            if let Some((col, row)) = self.alternate_cursor.take() {
                self.cursor_col = col;
                self.cursor_row = row;
            }
        }
    }

    /// Returns true if in alternate screen.
    #[must_use]
    pub const fn is_alternate_screen(&self) -> bool {
        self.alternate_screen.is_some()
    }

    /// Sets scrollback limit.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        while self.scrollback.len() > limit {
            self.scrollback.remove(0);
        }
    }

    /// Returns number of lines in scrollback.
    #[must_use]
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Returns true if scrollback line exists at index.
    #[must_use]
    pub fn has_scrollback_line(&self, index: usize) -> bool {
        index < self.scrollback.len()
    }

    /// Returns a scrollback row by index (0 = most recent scrollback line).
    #[must_use]
    pub fn scrollback_row(&self, index: usize) -> Option<&Row> {
        // Scrollback is stored oldest first, so we reverse the index
        let len = self.scrollback.len();
        if index < len {
            self.scrollback.get(len - 1 - index)
        } else {
            None
        }
    }

    /// Returns a visible row by index.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    /// Inserts n blank lines at cursor position.
    pub fn insert_lines(&mut self, n: u16) {
        let n = n.min(self.visible_rows - self.cursor_row) as usize;
        let cursor = self.cursor_row as usize;

        for _ in 0..n {
            self.rows.pop();
        }

        for _ in 0..n {
            self.rows.insert(cursor, Row::new(self.cols));
        }
    }

    /// Deletes n lines at cursor position.
    pub fn delete_lines(&mut self, n: u16) {
        let n = n.min(self.visible_rows - self.cursor_row) as usize;
        let cursor = self.cursor_row as usize;

        for _ in 0..n {
            if cursor < self.rows.len() {
                self.rows.remove(cursor);
            }
        }

        for _ in 0..n {
            self.rows.push(Row::new(self.cols));
        }
    }

    /// Inserts n blank characters at cursor position.
    pub fn insert_chars(&mut self, n: u16) {
        if let Some(row) = self.rows.get_mut(self.cursor_row as usize) {
            let cursor = self.cursor_col as usize;
            let width = row.len() as usize;
            let cells = row.cells_mut();

            for i in (cursor..width.saturating_sub(n as usize)).rev() {
                if i + (n as usize) < width {
                    cells[i + (n as usize)] = cells[i];
                }
            }

            for cell in cells.iter_mut().skip(cursor).take(n as usize) {
                *cell = Cell::default();
            }
        }
    }

    /// Deletes n characters at cursor position.
    pub fn delete_chars(&mut self, n: u16) {
        if let Some(row) = self.rows.get_mut(self.cursor_row as usize) {
            let cursor = self.cursor_col as usize;
            let width = row.len() as usize;
            let n = n as usize;
            let cells = row.cells_mut();

            for i in cursor..width {
                if i + n < width {
                    cells[i] = cells[i + n];
                } else {
                    cells[i] = Cell::default();
                }
            }
        }
    }

    // ========== Selection Methods ==========

    /// Starts a new selection at the given position.
    pub fn start_selection(&mut self, col: u16, row: u16) {
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.visible_rows.saturating_sub(1));
        self.selection = Some(Selection::new(col, row));
    }

    /// Starts a new selection with a specific mode.
    pub fn start_selection_with_mode(&mut self, col: u16, row: u16, mode: SelectionMode) {
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.visible_rows.saturating_sub(1));
        self.selection = Some(Selection::with_mode(col, row, mode));
    }

    /// Updates the selection end position.
    pub fn update_selection(&mut self, col: u16, row: u16) {
        let col = col.min(self.cols.saturating_sub(1));
        let row = row.min(self.visible_rows.saturating_sub(1));
        if let Some(ref mut sel) = self.selection {
            sel.update(col, row);
        }
    }

    /// Finalizes the selection (e.g., mouse released).
    pub fn finalize_selection(&mut self) {
        if let Some(ref mut sel) = self.selection {
            sel.finalize();
        }
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Returns a reference to the current selection.
    #[must_use]
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Returns whether there is an active selection.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selection.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Checks if a cell is within the current selection.
    #[must_use]
    pub fn is_cell_selected(&self, col: u16, row: u16) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|sel| sel.contains(col, row))
    }

    /// Returns the selected text from the grid.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection.as_ref()?;
        if selection.is_empty() {
            return None;
        }

        let ((start_col, start_row), (end_col, end_row)) = selection.normalized();
        let mut result = String::new();

        for row_idx in start_row..=end_row {
            let row = self.rows.get(row_idx as usize)?;

            let col_start = if row_idx == start_row { start_col } else { 0 };
            let col_end = if row_idx == end_row {
                end_col
            } else {
                self.cols.saturating_sub(1)
            };

            for col in col_start..=col_end {
                if let Some(cell) = row.cell(col) {
                    let ch = cell.character();
                    // Skip wide character continuations
                    if !cell.is_wide_continuation() {
                        result.push(ch);
                    }
                }
            }

            // Add newline between lines (not after last line)
            if row_idx < end_row {
                result.push('\n');
            }
        }

        // Trim trailing whitespace from each line
        let trimmed: String = result
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Starts selection from cursor position (for keyboard selection).
    pub fn start_selection_at_cursor(&mut self) {
        self.start_selection(self.cursor_col, self.cursor_row);
    }

    /// Extends selection left by one character.
    pub fn extend_selection_left(&mut self) {
        if self.selection.is_none() {
            self.start_selection_at_cursor();
        }
        if let Some(ref mut sel) = self.selection {
            sel.extend_left();
        }
    }

    /// Extends selection right by one character.
    pub fn extend_selection_right(&mut self) {
        if self.selection.is_none() {
            self.start_selection_at_cursor();
        }
        if let Some(ref mut sel) = self.selection {
            sel.extend_right(self.cols);
        }
    }

    /// Extends selection up by one row.
    pub fn extend_selection_up(&mut self) {
        if self.selection.is_none() {
            self.start_selection_at_cursor();
        }
        if let Some(ref mut sel) = self.selection {
            sel.extend_up();
        }
    }

    /// Extends selection down by one row.
    pub fn extend_selection_down(&mut self) {
        if self.selection.is_none() {
            self.start_selection_at_cursor();
        }
        if let Some(ref mut sel) = self.selection {
            sel.extend_down(self.visible_rows);
        }
    }
}
