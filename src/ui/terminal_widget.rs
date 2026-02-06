//! Terminal pane widget.
//!
//! Renders the terminal emulator content with scrollback support.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use tracing::debug;

use crate::terminal::{Terminal, grid::Grid};
use crate::theme::TerminalTheme;

/// Terminal widget for rendering.
pub struct TerminalWidget<'a> {
    /// Terminal to render.
    terminal: &'a Terminal,
    /// Whether the terminal is focused.
    focused: bool,
    /// Title for the terminal pane.
    title: Option<&'a str>,
    /// Theme for rendering colors.
    theme: Option<&'a TerminalTheme>,
}

impl<'a> TerminalWidget<'a> {
    /// Creates a new terminal widget.
    #[must_use]
    pub fn new(terminal: &'a Terminal) -> Self {
        Self {
            terminal,
            focused: false,
            title: None,
            theme: None,
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

    /// Sets the theme.
    #[must_use]
    pub fn theme(mut self, theme: &'a TerminalTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Renders the terminal grid with scroll offset support.
    fn render_grid(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let grid = self.terminal.grid();
        let scroll_offset = self.terminal.scroll_offset();
        let visible_rows = area.height as usize;
        let cols = grid.cols().min(area.width) as usize;

        // DIAGNOSTIC: Track if grid dimensions mismatch render area
        let grid_cols = grid.cols();
        let grid_rows = grid.rows();
        let area_cols = area.width;
        let area_rows = area.height;
        let cols_mismatch = grid_cols != area_cols;
        let rows_mismatch = grid_rows != area_rows;

        debug!(
            "TERMINAL_GRID: area=({}, {}, {}x{}), grid_cols={}, grid_rows={}, scroll_offset={}, MISMATCH=cols:{} rows:{}",
            area.x,
            area.y,
            area.width,
            area.height,
            grid_cols,
            grid_rows,
            scroll_offset,
            cols_mismatch,
            rows_mismatch
        );

        // NOTE: Widget::render() already clears the inner area before calling render_grid()
        // So we don't need to clear again here - redundant clearing can cause flicker

        // DIAGNOSTIC: Log how many rows would be filled vs empty at the top
        let grid_rows_usize = grid_rows as usize;
        let empty_top_rows = visible_rows.saturating_sub(grid_rows_usize);
        if empty_top_rows > 0 || cols_mismatch || rows_mismatch {
            debug!(
                "TERMINAL_GRID_FILL: visible_rows={}, grid_rows={}, empty_top_rows={}, effective_cols={}",
                visible_rows, grid_rows, empty_top_rows, cols
            );
        }

        // DIAGNOSTIC: Log grid content for first few rows to check if empty
        let sample_rows = 3.min(grid_rows_usize);
        for row_idx in 0..sample_rows {
            if let Some(row) = grid.row(row_idx) {
                let mut non_space_count = 0;
                let mut last_char: Option<char> = None;
                for col_idx in 0..cols.min(20) {
                    if let Some(cell) = row.cell(col_idx as u16) {
                        let c = cell.character();
                        if c != ' ' && c != '\0' {
                            non_space_count += 1;
                            last_char = Some(c);
                        }
                    }
                }
                if row_idx < 3 || non_space_count > 0 {
                    debug!(
                        "TERMINAL_ROW_{}: non_space_first20={}, last_char={:?}",
                        row_idx, non_space_count, last_char
                    );
                }
            }
        }

        for screen_row in 0..visible_rows {
            // Calculate which row to render based on scroll offset
            // scroll_offset = 0 means we're at the bottom (current view)
            // scroll_offset > 0 means we're looking at scrollback
            //
            // When scrolled, scrollback content appears at the TOP of the screen,
            // and visible grid content is pushed DOWN (and partially off-screen).

            if screen_row < scroll_offset {
                // Top rows show scrollback content (older history at top)
                // scrollback_row(0) = most recent scrollback line (just above visible grid)
                // So screen_row=0 with scroll_offset=5 should show scrollback[4] (oldest of visible)
                // screen_row=4 with scroll_offset=5 should show scrollback[0] (most recent)
                let scrollback_idx = scroll_offset - 1 - screen_row;
                self.render_scrollback_row(grid, scrollback_idx, screen_row, cols, area, buf);
            } else {
                // Remaining rows show visible grid content
                // screen_row - scroll_offset gives us which grid row to show
                // But we need to account for grid possibly being smaller than visible area
                let grid_rows = grid.rows() as usize;
                let rows_from_grid = visible_rows - scroll_offset;
                let grid_start_row = grid_rows.saturating_sub(rows_from_grid);
                let row_to_render = grid_start_row + (screen_row - scroll_offset);

                if row_to_render < grid_rows {
                    if let Some(row) = grid.row(row_to_render) {
                        self.render_row_cells(row, row_to_render, screen_row, cols, area, buf, grid);
                    }
                }
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

    /// Renders a row from the scrollback buffer (no selection support in scrollback).
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
            // Scrollback rows don't support selection, render without grid reference
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
                            // Use theme palette and default colors if available
                            let style = if let Some(theme) = self.theme {
                                cell.style().to_ratatui_with_palette_and_defaults(
                                    &theme.palette,
                                    Some(theme.foreground),
                                    Some(theme.background),
                                )
                            } else {
                                cell.style().to_ratatui()
                            };
                            ratatui_cell.set_style(style);
                        }
                    }
                }
            }
        }
    }

    /// Renders cells from a row.
    #[allow(clippy::too_many_arguments)]
    fn render_row_cells(
        &self,
        row: &crate::terminal::cell::Row,
        grid_row: usize,
        screen_row: usize,
        cols: usize,
        area: Rect,
        buf: &mut RatatuiBuffer,
        grid: &Grid,
    ) {
        let y = area.y + screen_row as u16;
        if y >= area.y + area.height {
            return;
        }

        // Selection highlight style - use theme if available
        let selection_color = self
            .theme
            .map(|t| t.selection)
            .unwrap_or(Color::Rgb(38, 79, 120));
        let selection_style = Style::default().bg(selection_color).fg(Color::White);

        // Log characters near the right edge for the first few rows (debug boundary issues)
        if screen_row < 3 && cols > 5 {
            let mut edge_chars = Vec::new();
            for check_col in (cols.saturating_sub(5))..cols {
                if let Some(cell) = row.cell(check_col as u16) {
                    let c = cell.character();
                    let is_wide = cell.is_wide();
                    edge_chars.push(format!(
                        "c{}='{}'{} U+{:04X}",
                        check_col,
                        if c.is_control() { '?' } else { c },
                        if is_wide { "W" } else { "" },
                        c as u32
                    ));
                }
            }
            debug!(
                "TERM_ROW_EDGE row={}: area_right={}, cols={} | {}",
                screen_row,
                area.x + area.width,
                cols,
                edge_chars.join(" ")
            );
        }

        for col in 0..cols {
            if let Some(cell) = row.cell(col as u16) {
                let x = area.x + col as u16;
                // CRITICAL: Check bounds strictly to prevent overflow into adjacent panes
                if x >= area.x + area.width {
                    break; // Don't render beyond the area boundary
                }

                // Skip wide continuation cells (they're rendered as part of the wide char)
                if cell.is_wide_continuation() {
                    continue;
                }

                if let Some(ratatui_cell) = buf.cell_mut((x, y)) {
                    let c = cell.character();

                    // For wide characters at the right edge, render a space instead
                    // to prevent overflow into adjacent panes
                    if cell.is_wide() && x + 1 >= area.x + area.width {
                        ratatui_cell.set_char(' ');
                    } else {
                        ratatui_cell.set_char(c);
                    }

                    // Check if this cell is selected
                    if grid.is_cell_selected(col as u16, grid_row as u16) {
                        ratatui_cell.set_style(selection_style);
                    } else {
                        // Use theme palette and default colors if available
                        let style = if let Some(theme) = self.theme {
                            cell.style().to_ratatui_with_palette_and_defaults(
                                &theme.palette,
                                Some(theme.foreground),
                                Some(theme.background),
                            )
                        } else {
                            cell.style().to_ratatui()
                        };
                        ratatui_cell.set_style(style);
                    }
                }
            }
        }
    }
}

impl Widget for TerminalWidget<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        debug!(
            "TERMINAL_WIDGET_START: area=({}, {}, {}x{}), right_edge={}, focused={}, title={:?}",
            area.x,
            area.y,
            area.width,
            area.height,
            area.x + area.width,
            self.focused,
            self.title
        );

        // Log pre-render state of cells at the right edge to detect corruption
        {
            let right_edge = area.x + area.width;
            for sample_y in [2, 10, 20].iter() {
                let y = *sample_y;
                if y < area.y + area.height {
                    let mut edge_info = Vec::new();
                    for x in (right_edge.saturating_sub(3))..right_edge.saturating_add(3) {
                        if let Some(cell) = buf.cell((x, y)) {
                            let symbol = cell.symbol();
                            let c = symbol.chars().next().unwrap_or(' ');
                            edge_info.push(format!("x{}=U+{:04X}", x, c as u32));
                        }
                    }
                    debug!("TERM_PRE_RENDER y={}: {}", y, edge_info.join(" "));
                }
            }
        }

        // Validate area bounds against buffer
        let buf_area = buf.area();
        if area.x + area.width > buf_area.width || area.y + area.height > buf_area.height {
            debug!(
                "TERMINAL_WIDGET WARNING: area extends beyond buffer! buf=({}, {}, {}x{})",
                buf_area.x, buf_area.y, buf_area.width, buf_area.height
            );
        }

        // Create block with border - use theme if available
        let (border_focused, border_unfocused) = self
            .theme
            .map(|t| (t.border_focused, t.border))
            .unwrap_or((Color::Cyan, Color::DarkGray));

        // Get background color for border - must be explicit to prevent Windows rendering artifacts
        let border_bg = self
            .theme
            .map(|t| t.background)
            .unwrap_or(Color::Rgb(30, 30, 30));

        let border_style = if self.focused {
            Style::default().fg(border_focused).bg(border_bg)
        } else {
            Style::default().fg(border_unfocused).bg(border_bg)
        };

        let title = self.title.unwrap_or("Terminal");
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // CRITICAL: Clear entire inner area first to prevent ghost characters
        let bg_color = self
            .theme
            .map(|t| t.background)
            .unwrap_or(Color::Rgb(30, 30, 30));
        let clear_style = Style::default().bg(bg_color).fg(Color::Reset);
        for y in inner_area.y..inner_area.y + inner_area.height {
            for x in inner_area.x..inner_area.x + inner_area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(clear_style);
                }
            }
        }

        // Render terminal content with scroll support
        self.render_grid(inner_area, buf);

        // Log post-render state of cells at the right edge
        {
            let right_edge = area.x + area.width;
            for sample_y in [2, 10, 20].iter() {
                let y = *sample_y;
                if y < area.y + area.height {
                    let mut edge_info = Vec::new();
                    for x in (right_edge.saturating_sub(3))..right_edge.saturating_add(3) {
                        if let Some(cell) = buf.cell((x, y)) {
                            let symbol = cell.symbol();
                            let c = symbol.chars().next().unwrap_or(' ');
                            edge_info.push(format!("x{}=U+{:04X}", x, c as u32));
                        }
                    }
                    debug!("TERM_POST_RENDER y={}: {}", y, edge_info.join(" "));
                }
            }
        }
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
