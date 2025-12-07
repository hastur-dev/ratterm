//! Tests for terminal grid (cell buffer) operations.
//!
//! Tests cover: cell operations, cursor movement, scrolling, line wrapping.

use ratterm::terminal::grid::Grid;
use ratterm::terminal::CursorShape;
use ratterm::terminal::style::{Color, Style, Attr};

/// Test grid initialization with correct dimensions.
#[test]
fn test_grid_new_dimensions() {
    let grid = Grid::new(80, 24);

    assert_eq!(grid.cols(), 80, "Grid columns mismatch");
    assert_eq!(grid.rows(), 24, "Grid rows mismatch");
    assert_eq!(grid.cursor_pos(), (0, 0), "Initial cursor should be at origin");
}

/// Test grid initialization creates empty cells.
#[test]
fn test_grid_initial_cells_empty() {
    let grid = Grid::new(10, 5);

    for row in 0..5 {
        for col in 0..10 {
            let cell = grid.cell(col, row);
            assert!(cell.is_some(), "Cell at ({col}, {row}) should exist");
            let cell = cell.expect("cell exists");
            assert_eq!(cell.character(), ' ', "Initial cell should be space");
        }
    }
}

/// Test writing a single character to the grid.
#[test]
fn test_grid_write_char() {
    let mut grid = Grid::new(80, 24);

    grid.write_char('A');

    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.character(), 'A', "Written character mismatch");
    assert_eq!(grid.cursor_pos(), (1, 0), "Cursor should advance after write");
}

/// Test cursor movement commands.
#[test]
fn test_cursor_movement() {
    let mut grid = Grid::new(80, 24);

    // Move cursor to specific position
    grid.set_cursor_pos(10, 5);
    assert_eq!(grid.cursor_pos(), (10, 5), "Cursor position mismatch");

    // Move relative
    grid.move_cursor_right(5);
    assert_eq!(grid.cursor_pos(), (15, 5), "Cursor right movement failed");

    grid.move_cursor_down(3);
    assert_eq!(grid.cursor_pos(), (15, 8), "Cursor down movement failed");

    grid.move_cursor_left(10);
    assert_eq!(grid.cursor_pos(), (5, 8), "Cursor left movement failed");

    grid.move_cursor_up(4);
    assert_eq!(grid.cursor_pos(), (5, 4), "Cursor up movement failed");
}

/// Test cursor bounds clamping.
#[test]
fn test_cursor_bounds_clamping() {
    let mut grid = Grid::new(80, 24);

    // Try to move past left edge
    grid.set_cursor_pos(0, 0);
    grid.move_cursor_left(10);
    assert_eq!(grid.cursor_pos().0, 0, "Cursor should clamp at left edge");

    // Try to move past top edge
    grid.move_cursor_up(10);
    assert_eq!(grid.cursor_pos().1, 0, "Cursor should clamp at top edge");

    // Try to move past right edge
    grid.set_cursor_pos(79, 0);
    grid.move_cursor_right(10);
    assert_eq!(grid.cursor_pos().0, 79, "Cursor should clamp at right edge");

    // Try to move past bottom (should scroll instead in real impl)
    grid.set_cursor_pos(0, 23);
    grid.move_cursor_down(10);
    // Behavior depends on scroll mode - for now just clamp
    assert!(grid.cursor_pos().1 <= 23, "Cursor should not exceed bottom");
}

/// Test line wrapping when writing past right edge.
#[test]
fn test_line_wrap_on_write() {
    let mut grid = Grid::new(10, 5);

    // Write 15 characters - should wrap to next line
    for c in "Hello World!!!".chars() {
        grid.write_char(c);
    }

    // Check first line
    let first_line: String = (0..10)
        .filter_map(|col| grid.cell(col, 0))
        .map(|c| c.character())
        .collect();
    assert_eq!(first_line, "Hello Worl", "First line content mismatch");

    // Check second line (wrapped content)
    let second_line: String = (0..4)
        .filter_map(|col| grid.cell(col, 1))
        .map(|c| c.character())
        .collect();
    assert_eq!(second_line, "d!!!", "Wrapped content mismatch");
}

/// Test newline handling.
#[test]
fn test_newline() {
    let mut grid = Grid::new(80, 24);

    grid.write_char('A');
    grid.newline();

    assert_eq!(grid.cursor_pos(), (0, 1), "Newline should move to next row");

    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.character(), 'A', "Previous character should remain");
}

/// Test carriage return.
#[test]
fn test_carriage_return() {
    let mut grid = Grid::new(80, 24);

    grid.write_char('A');
    grid.write_char('B');
    grid.write_char('C');
    grid.carriage_return();

    assert_eq!(grid.cursor_pos(), (0, 0), "CR should return to column 0");
}

/// Test tab character handling.
#[test]
fn test_tab_stops() {
    let mut grid = Grid::new(80, 24);

    grid.tab();
    assert_eq!(grid.cursor_pos().0, 8, "Default tab stop at 8");

    grid.tab();
    assert_eq!(grid.cursor_pos().0, 16, "Second tab at 16");

    // Start from middle of tab stop
    grid.set_cursor_pos(10, 0);
    grid.tab();
    assert_eq!(grid.cursor_pos().0, 16, "Tab should go to next stop");
}

/// Test backspace handling.
#[test]
fn test_backspace() {
    let mut grid = Grid::new(80, 24);

    grid.write_char('A');
    grid.write_char('B');
    grid.backspace();

    assert_eq!(grid.cursor_pos(), (1, 0), "Backspace should move cursor back");
}

/// Test scroll up operation.
#[test]
fn test_scroll_up() {
    let mut grid = Grid::new(10, 3);

    // Write identifiable content on each row
    grid.set_cursor_pos(0, 0);
    grid.write_char('A');
    grid.set_cursor_pos(0, 1);
    grid.write_char('B');
    grid.set_cursor_pos(0, 2);
    grid.write_char('C');

    grid.scroll_up(1);

    // Row 0 should now have 'B' (was row 1)
    let cell_0 = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell_0.character(), 'B', "After scroll up, row 0 should have B");

    // Row 1 should now have 'C' (was row 2)
    let cell_1 = grid.cell(0, 1).expect("cell exists");
    assert_eq!(cell_1.character(), 'C', "After scroll up, row 1 should have C");

    // Row 2 should be empty (new row)
    let cell_2 = grid.cell(0, 2).expect("cell exists");
    assert_eq!(cell_2.character(), ' ', "After scroll up, row 2 should be empty");
}

/// Test scroll down operation.
#[test]
fn test_scroll_down() {
    let mut grid = Grid::new(10, 3);

    grid.set_cursor_pos(0, 0);
    grid.write_char('A');
    grid.set_cursor_pos(0, 1);
    grid.write_char('B');
    grid.set_cursor_pos(0, 2);
    grid.write_char('C');

    grid.scroll_down(1);

    // Row 0 should be empty (new row)
    let cell_0 = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell_0.character(), ' ', "After scroll down, row 0 should be empty");

    // Row 1 should have 'A' (was row 0)
    let cell_1 = grid.cell(0, 1).expect("cell exists");
    assert_eq!(cell_1.character(), 'A', "After scroll down, row 1 should have A");

    // Row 2 should have 'B' (was row 1)
    let cell_2 = grid.cell(0, 2).expect("cell exists");
    assert_eq!(cell_2.character(), 'B', "After scroll down, row 2 should have B");
}

/// Test clear screen operation.
#[test]
fn test_clear_screen() {
    let mut grid = Grid::new(10, 5);

    // Write some content
    for c in "Hello".chars() {
        grid.write_char(c);
    }

    grid.clear();

    // All cells should be empty
    for row in 0..5 {
        for col in 0..10 {
            let cell = grid.cell(col, row).expect("cell exists");
            assert_eq!(cell.character(), ' ', "Cell should be empty after clear");
        }
    }

    // Cursor should be at origin
    assert_eq!(grid.cursor_pos(), (0, 0), "Cursor should be at origin after clear");
}

/// Test clear to end of line.
#[test]
fn test_clear_to_eol() {
    let mut grid = Grid::new(10, 1);

    for c in "HelloWorld".chars() {
        grid.write_char(c);
    }

    grid.set_cursor_pos(5, 0);
    grid.clear_to_eol();

    let line: String = (0..10)
        .filter_map(|col| grid.cell(col, 0))
        .map(|c| c.character())
        .collect();
    assert_eq!(line, "Hello     ", "Should clear from cursor to end");
}

/// Test cell styling.
#[test]
fn test_cell_styling() {
    let mut grid = Grid::new(80, 24);

    let style = Style::new()
        .fg(Color::Red)
        .bg(Color::Blue)
        .add_attr(Attr::Bold);

    grid.set_style(style);
    grid.write_char('X');

    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.style().fg_color(), Some(Color::Red), "Foreground color mismatch");
    assert_eq!(cell.style().bg_color(), Some(Color::Blue), "Background color mismatch");
    assert!(cell.style().has_attr(Attr::Bold), "Bold attribute missing");
}

/// Test grid resize - larger.
#[test]
fn test_resize_larger() {
    let mut grid = Grid::new(10, 5);

    grid.set_cursor_pos(5, 2);
    grid.write_char('X');

    grid.resize(20, 10);

    assert_eq!(grid.cols(), 20, "Columns should increase");
    assert_eq!(grid.rows(), 10, "Rows should increase");

    // Original content should be preserved
    let cell = grid.cell(5, 2).expect("cell exists");
    assert_eq!(cell.character(), 'X', "Content should be preserved");
}

/// Test grid resize - smaller.
#[test]
fn test_resize_smaller() {
    let mut grid = Grid::new(20, 10);

    grid.set_cursor_pos(5, 2);
    grid.write_char('X');

    grid.resize(10, 5);

    assert_eq!(grid.cols(), 10, "Columns should decrease");
    assert_eq!(grid.rows(), 5, "Rows should decrease");

    // Content within bounds should be preserved
    let cell = grid.cell(5, 2).expect("cell exists");
    assert_eq!(cell.character(), 'X', "Content within bounds preserved");

    // Cursor should be clamped to new bounds
    let (cx, cy) = grid.cursor_pos();
    assert!(cx < 10 && cy < 5, "Cursor should be within new bounds");
}

/// Test cursor visibility toggle.
#[test]
fn test_cursor_visibility() {
    let mut grid = Grid::new(80, 24);

    assert!(grid.cursor_visible(), "Cursor should be visible by default");

    grid.set_cursor_visible(false);
    assert!(!grid.cursor_visible(), "Cursor should be hidden");

    grid.set_cursor_visible(true);
    assert!(grid.cursor_visible(), "Cursor should be visible again");
}

/// Test cursor shape.
#[test]
fn test_cursor_shape() {
    let mut grid = Grid::new(80, 24);

    assert_eq!(grid.cursor_shape(), CursorShape::Block, "Default cursor is block");

    grid.set_cursor_shape(CursorShape::Underline);
    assert_eq!(grid.cursor_shape(), CursorShape::Underline, "Cursor shape change");

    grid.set_cursor_shape(CursorShape::Bar);
    assert_eq!(grid.cursor_shape(), CursorShape::Bar, "Cursor shape change to bar");
}

/// Test alternate screen buffer.
#[test]
fn test_alternate_screen() {
    let mut grid = Grid::new(10, 5);

    // Write to main screen
    grid.write_char('M');

    // Switch to alternate screen
    grid.enter_alternate_screen();
    assert!(grid.is_alternate_screen(), "Should be in alternate screen");

    // Alternate screen should be empty
    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.character(), ' ', "Alternate screen should be empty");

    // Write to alternate screen
    grid.write_char('A');

    // Exit alternate screen
    grid.exit_alternate_screen();
    assert!(!grid.is_alternate_screen(), "Should be back to main screen");

    // Main screen content should be preserved
    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.character(), 'M', "Main screen content preserved");
}

/// Test scrollback buffer.
#[test]
fn test_scrollback_buffer() {
    let mut grid = Grid::new(10, 3);
    grid.set_scrollback_limit(10);

    // Fill screen and scroll
    grid.set_cursor_pos(0, 0);
    grid.write_char('A');
    grid.set_cursor_pos(0, 1);
    grid.write_char('B');
    grid.set_cursor_pos(0, 2);
    grid.write_char('C');

    grid.scroll_up(1);

    // Line with 'A' should be in scrollback
    assert_eq!(grid.scrollback_len(), 1, "One line in scrollback");

    assert!(grid.has_scrollback_line(0), "Scrollback line should exist");
}

/// Test wide character handling (CJK).
#[test]
fn test_wide_characters() {
    let mut grid = Grid::new(10, 1);

    // '中' is a wide character (2 cells)
    grid.write_char('中');

    assert_eq!(grid.cursor_pos(), (2, 0), "Wide char should advance cursor by 2");

    let cell = grid.cell(0, 0).expect("cell exists");
    assert_eq!(cell.character(), '中', "Wide character stored");
    assert!(cell.is_wide(), "Cell marked as wide");

    // Second cell should be a placeholder
    let placeholder = grid.cell(1, 0).expect("cell exists");
    assert!(placeholder.is_wide_continuation(), "Second cell is continuation");
}
