//! Terminal pane widget.
//!
//! Renders the terminal emulator content with scrollback support.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

use crate::terminal::{grid::Grid, Terminal};

/// Terminal widget for rendering.
pub struct TerminalWidget<'a> {
    /// Terminal to render.
    terminal: &'a Terminal,
    /// Whether the terminal is focused.
    focused: bool,
    /// Title for the terminal pane.
    title: Option<&'a str>,
}

impl<'a> TerminalWidget<'a> {
    /// Creates a new terminal widget.
    #[must_use]
    pub fn new(terminal: &'a Terminal) -> Self {
        Self {
            terminal,
            focused: false,
            title: None,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Sets the title.
    #[must_use]
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Renders the terminal grid with scroll offset support.
    fn render_grid(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let grid = self.terminal.grid();
        let scroll_offset = self.terminal.scroll_offset();
        let visible_rows = area.height as usize;
        let cols = grid.cols().min(area.width) as usize;

        for screen_row in 0..visible_rows {
            // Calculate which row to render based on scroll offset
            // scroll_offset = 0 means we're at the bottom (current view)
            // scroll_offset > 0 means we're looking at scrollback

            let row_from_bottom = visible_rows - 1 - screen_row;
            let row_to_render = if row_from_bottom < scroll_offset {
                // This row is in scrollback
                let scrollback_idx = scroll_offset - 1 - row_from_bottom;
                self.render_scrollback_row(grid, scrollback_idx, screen_row, cols, area, buf);
                continue;
            } else {
                // This row is in visible grid
                let grid_row = (grid.rows() as usize)
                    .saturating_sub(scroll_offset)
                    .saturating_sub(visible_rows - screen_row);
                grid_row
            };

            // Render row from visible grid
            if let Some(row) = grid.row(row_to_render) {
                self.render_row_cells(row, screen_row, cols, area, buf);
            }
        }

        // Only render cursor if we're at the bottom (scroll_offset == 0)
        if scroll_offset == 0 && grid.cursor_visible() {
            let (cursor_col, cursor_row) = grid.cursor_pos();

            // Calculate offset when render area is larger than grid
            // Grid content is rendered at the bottom, so cursor needs same offset
            let grid_rows = grid.rows() as usize;
            let y_offset = visible_rows.saturating_sub(grid_rows) as u16;

            let cursor_x = area.x + cursor_col;
            let cursor_y = area.y + cursor_row + y_offset;

            if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                if let Some(cell) = buf.cell_mut((cursor_x, cursor_y)) {
                    let current_style = cell.style();
                    let cursor_style = current_style.add_modifier(Modifier::REVERSED);
                    cell.set_style(cursor_style);
                }
            }
        }
    }

    /// Renders a row from the scrollback buffer.
    fn render_scrollback_row(
        &self,
        grid: &Grid,
        scrollback_idx: usize,
        screen_row: usize,
        cols: usize,
        area: Rect,
        buf: &mut RatatuiBuffer,
    ) {
        if let Some(row) = grid.scrollback_row(scrollback_idx) {
            self.render_row_cells(row, screen_row, cols, area, buf);
        }
    }

    /// Renders cells from a row.
    fn render_row_cells(
        &self,
        row: &crate::terminal::cell::Row,
        screen_row: usize,
        cols: usize,
        area: Rect,
        buf: &mut RatatuiBuffer,
    ) {
        let y = area.y + screen_row as u16;
        if y >= area.y + area.height {
            return;
        }

        for col in 0..cols {
            if let Some(cell) = row.cell(col as u16) {
                let x = area.x + col as u16;
                if x < area.x + area.width {
                    if let Some(ratatui_cell) = buf.cell_mut((x, y)) {
                        ratatui_cell.set_char(cell.character());
                        ratatui_cell.set_style(cell.style().to_ratatui());
                    }
                }
            }
        }
    }
}

impl<'a> Widget for TerminalWidget<'a> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        // Create block with border
        let border_style = if self.focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = self.title.unwrap_or("Terminal");
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Render terminal content with scroll support
        self.render_grid(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    // Widget rendering tests would require a mock terminal
    // Basic structural tests here
    #[test]
    fn test_terminal_widget_placeholder() {
        // Can't easily test without a real terminal
        // This is a placeholder for widget tests
        assert!(true);
    }
}
