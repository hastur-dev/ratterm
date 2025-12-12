//! Editor viewport management.
//!
//! Handles scroll position and visible area calculations.

use super::buffer::Position;

/// Scroll margin (lines to keep visible around cursor).
const SCROLL_MARGIN: usize = 3;

/// Editor viewport.
#[derive(Debug, Clone)]
pub struct View {
    /// First visible line (0-indexed).
    scroll_top: usize,
    /// First visible column (for horizontal scroll).
    scroll_left: usize,
    /// Number of visible lines.
    height: usize,
    /// Number of visible columns.
    width: usize,
    /// Line number gutter width.
    gutter_width: usize,
}

impl View {
    /// Creates a new viewport.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        assert!(width > 0, "Width must be positive");
        assert!(height > 0, "Height must be positive");

        Self {
            scroll_top: 0,
            scroll_left: 0,
            height,
            width,
            gutter_width: 4, // Default line number width
        }
    }

    /// Returns the first visible line.
    #[must_use]
    pub const fn scroll_top(&self) -> usize {
        self.scroll_top
    }

    /// Returns the first visible column.
    #[must_use]
    pub const fn scroll_left(&self) -> usize {
        self.scroll_left
    }

    /// Returns the viewport height in lines.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns the viewport width in columns.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the gutter width for line numbers.
    #[must_use]
    pub const fn gutter_width(&self) -> usize {
        self.gutter_width
    }

    /// Returns the width available for text.
    #[must_use]
    pub fn text_width(&self) -> usize {
        self.width.saturating_sub(self.gutter_width + 1)
    }

    /// Returns the last visible line (exclusive).
    #[must_use]
    pub fn scroll_bottom(&self) -> usize {
        self.scroll_top + self.height
    }

    /// Sets the viewport dimensions.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    /// Sets the gutter width based on line count.
    pub fn update_gutter_width(&mut self, line_count: usize) {
        let digits = if line_count == 0 {
            1
        } else {
            (line_count as f64).log10().floor() as usize + 1
        };
        self.gutter_width = digits + 2; // padding
    }

    /// Resets scroll position to origin (0, 0).
    ///
    /// Call this when opening a new file to ensure the view starts from the top.
    pub fn reset_scroll(&mut self) {
        self.scroll_top = 0;
        self.scroll_left = 0;
    }

    /// Scrolls to ensure the cursor is visible.
    pub fn ensure_cursor_visible(&mut self, cursor: Position) {
        // Vertical scrolling
        let margin = SCROLL_MARGIN.min(self.height / 2);

        // Scroll up if cursor is above visible area
        if cursor.line < self.scroll_top + margin {
            self.scroll_top = cursor.line.saturating_sub(margin);
        }

        // Scroll down if cursor is below visible area
        let visible_bottom = self.scroll_top + self.height;
        if cursor.line >= visible_bottom.saturating_sub(margin) {
            self.scroll_top = (cursor.line + margin + 1).saturating_sub(self.height);
        }

        // Horizontal scrolling
        let text_width = self.text_width();
        let margin_h = 5.min(text_width / 4);

        // Scroll left if cursor is to the left
        if cursor.col < self.scroll_left + margin_h {
            self.scroll_left = cursor.col.saturating_sub(margin_h);
        }

        // Scroll right if cursor is to the right
        if cursor.col >= self.scroll_left + text_width.saturating_sub(margin_h) {
            self.scroll_left = (cursor.col + margin_h + 1).saturating_sub(text_width);
        }
    }

    /// Scrolls up by n lines.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_top = self.scroll_top.saturating_sub(n);
    }

    /// Scrolls down by n lines.
    pub fn scroll_down(&mut self, n: usize, max_lines: usize) {
        let max_scroll = max_lines.saturating_sub(1);
        self.scroll_top = (self.scroll_top + n).min(max_scroll);
    }

    /// Scrolls left by n columns.
    pub fn scroll_left_by(&mut self, n: usize) {
        self.scroll_left = self.scroll_left.saturating_sub(n);
    }

    /// Scrolls right by n columns.
    pub fn scroll_right_by(&mut self, n: usize) {
        self.scroll_left += n;
    }

    /// Centers the view on a specific line.
    pub fn center_on_line(&mut self, line: usize, max_lines: usize) {
        let half_height = self.height / 2;
        if line > half_height {
            self.scroll_top = (line - half_height).min(max_lines.saturating_sub(self.height));
        } else {
            self.scroll_top = 0;
        }
    }

    /// Converts a buffer position to a screen position.
    #[must_use]
    pub fn buffer_to_screen(&self, pos: Position) -> Option<(u16, u16)> {
        if pos.line < self.scroll_top || pos.line >= self.scroll_top + self.height {
            return None;
        }

        if pos.col < self.scroll_left {
            return None;
        }

        let screen_x = pos.col - self.scroll_left + self.gutter_width + 1;
        let screen_y = pos.line - self.scroll_top;

        if screen_x >= self.width {
            return None;
        }

        Some((screen_x as u16, screen_y as u16))
    }

    /// Converts a screen position to a buffer position.
    #[must_use]
    pub fn screen_to_buffer(&self, x: u16, y: u16) -> Position {
        let line = self.scroll_top + y as usize;
        let col = if x as usize > self.gutter_width {
            self.scroll_left + (x as usize - self.gutter_width - 1)
        } else {
            0
        };

        Position::new(line, col)
    }

    /// Returns true if the given line is visible.
    #[must_use]
    pub fn is_line_visible(&self, line: usize) -> bool {
        line >= self.scroll_top && line < self.scroll_top + self.height
    }

    /// Returns the visible line range.
    #[must_use]
    pub fn visible_lines(&self) -> std::ops::Range<usize> {
        self.scroll_top..self.scroll_top + self.height
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_new() {
        let view = View::new(80, 24);
        assert_eq!(view.width(), 80);
        assert_eq!(view.height(), 24);
        assert_eq!(view.scroll_top(), 0);
    }

    #[test]
    fn test_view_scroll() {
        let mut view = View::new(80, 10);

        view.scroll_down(5, 100);
        assert_eq!(view.scroll_top(), 5);

        view.scroll_up(3);
        assert_eq!(view.scroll_top(), 2);
    }

    #[test]
    fn test_view_ensure_cursor_visible() {
        let mut view = View::new(80, 10);

        // Cursor below visible area
        view.ensure_cursor_visible(Position::new(20, 0));
        assert!(view.scroll_top() > 0);
        assert!(view.is_line_visible(20));
    }

    #[test]
    fn test_view_buffer_to_screen() {
        let view = View::new(80, 10);

        // Position at origin
        let screen = view.buffer_to_screen(Position::new(0, 0));
        assert!(screen.is_some());

        // Position outside view
        let screen = view.buffer_to_screen(Position::new(100, 0));
        assert!(screen.is_none());
    }
}
