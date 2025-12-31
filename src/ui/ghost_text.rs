//! Ghost text widget for inline completion suggestions.
//!
//! Renders grayed-out completion text at the cursor position,
//! showing what will be inserted if the user accepts the completion.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use unicode_width::UnicodeWidthChar;

use crate::editor::view::View;

/// Default ghost text foreground color.
const GHOST_TEXT_FG: Color = Color::DarkGray;

/// Ghost text widget for rendering inline completion suggestions.
pub struct GhostTextWidget<'a> {
    /// The completion suggestion text to display.
    suggestion: Option<&'a str>,

    /// Current cursor line (0-indexed).
    cursor_line: usize,

    /// Current cursor column (0-indexed).
    cursor_col: usize,

    /// Editor view for coordinate conversion.
    view: &'a View,

    /// Gutter width (line numbers area).
    gutter_width: usize,

    /// Ghost text style.
    style: Style,

    /// The current line content (to calculate where ghost text starts).
    line_content: &'a str,
}

impl<'a> GhostTextWidget<'a> {
    /// Creates a new ghost text widget.
    #[must_use]
    pub fn new(
        suggestion: Option<&'a str>,
        cursor_line: usize,
        cursor_col: usize,
        view: &'a View,
        line_content: &'a str,
    ) -> Self {
        assert!(cursor_line < usize::MAX, "cursor_line must be valid");

        Self {
            suggestion,
            cursor_line,
            cursor_col,
            view,
            gutter_width: view.gutter_width(),
            style: Style::default()
                .fg(GHOST_TEXT_FG)
                .add_modifier(Modifier::ITALIC),
            line_content,
        }
    }

    /// Sets a custom style for the ghost text.
    #[must_use]
    pub const fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Sets the ghost text foreground color.
    #[must_use]
    pub fn with_fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    /// Renders the ghost text.
    pub fn render(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let suggestion = match self.suggestion {
            Some(s) if !s.is_empty() => s,
            _ => return,
        };

        // Check if cursor line is visible
        let visible_lines = self.view.visible_lines();
        if !visible_lines.contains(&self.cursor_line) {
            return;
        }

        // Calculate screen row for cursor
        let screen_row = self.cursor_line.saturating_sub(self.view.scroll_top());
        if screen_row >= area.height as usize {
            return;
        }

        // Calculate text area start (after gutter)
        let text_x = area.x + self.gutter_width as u16 + 1;
        let text_width = area.width.saturating_sub(self.gutter_width as u16 + 1);

        // Calculate visual column for cursor position
        let visual_cursor_col = self.calculate_visual_column();

        // Account for horizontal scroll
        let scroll_left = self.view.scroll_left();
        let screen_col = visual_cursor_col.saturating_sub(scroll_left);

        if screen_col >= text_width as usize {
            return;
        }

        // Render the ghost text
        let y = area.y + screen_row as u16;
        let mut current_col = screen_col;

        for ch in suggestion.chars() {
            if current_col >= text_width as usize {
                break;
            }

            // Skip newlines in suggestion
            if ch == '\n' || ch == '\r' {
                continue;
            }

            let char_width = ch.width().unwrap_or(1);
            let x = text_x + current_col as u16;

            if x < text_x + text_width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(self.style);
                }

                // Handle wide characters
                for i in 1..char_width {
                    let x2 = text_x + (current_col + i) as u16;
                    if x2 < text_x + text_width {
                        if let Some(cell) = buf.cell_mut((x2, y)) {
                            cell.set_char(' ');
                            cell.set_style(self.style);
                        }
                    }
                }
            }

            current_col += char_width;
        }
    }

    /// Calculates the visual column position accounting for tabs and wide chars.
    fn calculate_visual_column(&self) -> usize {
        let tab_width: usize = 4;
        let mut visual_col: usize = 0;

        for (idx, ch) in self.line_content.chars().enumerate() {
            if idx >= self.cursor_col {
                break;
            }

            let char_width = if ch == '\t' {
                tab_width - (visual_col % tab_width)
            } else if ch == '\n' {
                0
            } else {
                ch.width().unwrap_or(1)
            };

            visual_col += char_width;
        }

        visual_col
    }
}

/// Extracts the portion of the suggestion that should be shown.
///
/// If the suggestion starts with the current word at cursor,
/// returns only the remaining part.
#[must_use]
pub fn extract_ghost_text<'a>(suggestion: &'a str, word_at_cursor: &str) -> &'a str {
    assert!(!suggestion.is_empty() || word_at_cursor.is_empty());

    if word_at_cursor.is_empty() {
        return suggestion;
    }

    // If suggestion starts with the word at cursor, show only the rest
    if let Some(stripped) = suggestion.strip_prefix(word_at_cursor) {
        stripped
    } else if suggestion
        .to_lowercase()
        .starts_with(&word_at_cursor.to_lowercase())
    {
        // Case-insensitive match
        &suggestion[word_at_cursor.len()..]
    } else {
        suggestion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ghost_text_empty_word() {
        let result = extract_ghost_text("println!", "");
        assert_eq!(result, "println!");
    }

    #[test]
    fn test_extract_ghost_text_prefix_match() {
        let result = extract_ghost_text("println!", "print");
        assert_eq!(result, "ln!");
    }

    #[test]
    fn test_extract_ghost_text_case_insensitive() {
        let result = extract_ghost_text("Println!", "print");
        assert_eq!(result, "ln!");
    }

    #[test]
    fn test_extract_ghost_text_no_match() {
        let result = extract_ghost_text("hello", "world");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_extract_ghost_text_full_match() {
        let result = extract_ghost_text("hello", "hello");
        assert_eq!(result, "");
    }

    #[test]
    fn test_calculate_visual_column() {
        use crate::editor::view::View;

        let view = View::new(80, 24);
        let widget = GhostTextWidget::new(
            Some("test"),
            0,
            4,
            &view,
            "let x", // cursor at position 4 (after "let ")
        );

        let visual_col = widget.calculate_visual_column();
        assert_eq!(visual_col, 4);
    }

    #[test]
    fn test_calculate_visual_column_with_tabs() {
        use crate::editor::view::View;

        let view = View::new(80, 24);
        let widget = GhostTextWidget::new(
            Some("test"),
            0,
            2, // cursor after the tab and 'x'
            &view,
            "\tx", // tab + x
        );

        let visual_col = widget.calculate_visual_column();
        // Tab expands to 4 spaces, then 'x' is at position 4
        assert_eq!(visual_col, 5); // 4 (tab) + 1 (x)
    }
}
