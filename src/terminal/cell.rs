//! Terminal cell and cursor types.
//!
//! Defines individual cell content and cursor shapes.

use unicode_width::UnicodeWidthChar;

use super::style::Style;

/// Cursor shape variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// Block cursor (default).
    #[default]
    Block,
    /// Underline cursor.
    Underline,
    /// Vertical bar cursor.
    Bar,
}

/// Cell flags for special states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellFlags {
    /// This cell contains a wide character.
    pub wide: bool,
    /// This cell is a continuation of a wide character.
    pub wide_continuation: bool,
}

/// A single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The character in this cell.
    character: char,
    /// Cell style (colors, attributes).
    style: Style,
    /// Cell flags.
    flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            style: Style::new(),
            flags: CellFlags::default(),
        }
    }
}

impl Cell {
    /// Creates a new cell with the given character and style.
    #[must_use]
    pub fn new(character: char, style: Style) -> Self {
        Self {
            character,
            style,
            flags: CellFlags::default(),
        }
    }

    /// Returns the character in this cell.
    #[must_use]
    pub const fn character(&self) -> char {
        self.character
    }

    /// Returns the style of this cell.
    #[must_use]
    pub const fn style(&self) -> Style {
        self.style
    }

    /// Returns true if this cell contains a wide character.
    #[must_use]
    pub const fn is_wide(&self) -> bool {
        self.flags.wide
    }

    /// Returns true if this cell is a wide character continuation.
    #[must_use]
    pub const fn is_wide_continuation(&self) -> bool {
        self.flags.wide_continuation
    }

    /// Resets the cell to default (space with no style).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Sets the character and updates wide flags.
    pub fn set_char(&mut self, c: char) {
        self.character = c;
        self.flags.wide = c.width().unwrap_or(0) > 1;
        self.flags.wide_continuation = false;
    }

    /// Marks this cell as a wide character continuation.
    pub fn set_wide_continuation(&mut self) {
        self.character = ' ';
        self.flags.wide = false;
        self.flags.wide_continuation = true;
    }

    /// Sets the style.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }
}

/// A single row in the terminal grid.
#[derive(Debug, Clone)]
pub struct Row {
    /// Cells in this row.
    cells: Vec<Cell>,
}

impl Row {
    /// Creates a new row with the given width.
    pub fn new(width: u16) -> Self {
        assert!(width > 0, "Row width must be positive");
        Self {
            cells: vec![Cell::default(); width as usize],
        }
    }

    /// Returns the cell at the given column.
    pub fn cell(&self, col: u16) -> Option<&Cell> {
        self.cells.get(col as usize)
    }

    /// Returns a mutable reference to the cell at the given column.
    pub fn cell_mut(&mut self, col: u16) -> Option<&mut Cell> {
        self.cells.get_mut(col as usize)
    }

    /// Returns the number of columns.
    pub fn len(&self) -> u16 {
        self.cells.len() as u16
    }

    /// Returns true if the row is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Clears all cells in this row.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.reset();
        }
    }

    /// Clears cells from `start` to end of row.
    pub fn clear_from(&mut self, start: u16) {
        let start = start as usize;
        for cell in self.cells.iter_mut().skip(start) {
            cell.reset();
        }
    }

    /// Clears cells from start of row to `end` (exclusive).
    pub fn clear_to(&mut self, end: u16) {
        let end = (end as usize).min(self.cells.len());
        for cell in self.cells.iter_mut().take(end) {
            cell.reset();
        }
    }

    /// Resizes the row to the new width.
    pub fn resize(&mut self, new_width: u16) {
        self.cells.resize(new_width as usize, Cell::default());
    }

    /// Returns direct access to cells for advanced operations.
    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Returns a slice of cells (read-only).
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
}
