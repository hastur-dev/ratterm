//! Editor pane widget.
//!
//! Renders the code editor content with line numbers and cursor.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use unicode_width::UnicodeWidthChar;

use crate::editor::{Editor, EditorMode};
use crate::theme::EditorTheme;

/// Editor widget for rendering.
pub struct EditorWidget<'a> {
    /// Editor to render.
    editor: &'a Editor,
    /// Whether the editor is focused.
    focused: bool,
    /// Theme for rendering colors.
    theme: Option<&'a EditorTheme>,
}

impl<'a> EditorWidget<'a> {
    /// Creates a new editor widget.
    #[must_use]
    pub fn new(editor: &'a Editor) -> Self {
        Self {
            editor,
            focused: false,
            theme: None,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Sets the theme.
    #[must_use]
    pub fn theme(mut self, theme: &'a EditorTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Renders the line numbers.
    fn render_line_numbers(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let view = self.editor.view();
        let buffer = self.editor.buffer();
        let gutter_width = view.gutter_width();

        // Use theme colors if available
        let line_num_fg = self
            .theme
            .map(|t| t.line_numbers_fg)
            .unwrap_or(Color::DarkGray);
        let gutter_bg = self
            .theme
            .map(|t| t.line_numbers_bg)
            .unwrap_or(Color::Reset);
        let current_line_fg = self.theme.map(|t| t.cursor).unwrap_or(Color::Yellow);

        let style = Style::default().fg(line_num_fg).bg(gutter_bg);
        let current_line_style = Style::default().fg(current_line_fg).bg(gutter_bg);

        // Clear gutter area first to prevent ghost characters
        for row in 0..area.height {
            for col in 0..gutter_width as u16 {
                let x = area.x + col;
                let y = area.y + row;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(style);
                }
            }
        }

        let cursor_line = self.editor.cursor_position().line;

        for (screen_row, line_idx) in view.visible_lines().enumerate() {
            if screen_row >= area.height as usize {
                break;
            }

            if line_idx >= buffer.len_lines() {
                // Render ~ for lines past end of file
                let x = area.x;
                let y = area.y + screen_row as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char('~');
                    cell.set_style(Style::default().fg(line_num_fg));
                }
            } else {
                // Render line number
                let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width - 1);
                let line_style = if line_idx == cursor_line {
                    current_line_style
                } else {
                    style
                };

                for (i, c) in line_num.chars().enumerate() {
                    if i >= gutter_width {
                        break;
                    }
                    let x = area.x + i as u16;
                    let y = area.y + screen_row as u16;
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(c);
                        cell.set_style(line_style);
                    }
                }
            }
        }
    }

    /// Renders the text content.
    fn render_content(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let view = self.editor.view();
        let buffer = self.editor.buffer();
        let gutter_width = view.gutter_width();

        let text_x = area.x + gutter_width as u16 + 1;
        let text_width = area.width.saturating_sub(gutter_width as u16 + 1);

        // Use theme colors if available
        let text_fg = self.theme.map(|t| t.foreground).unwrap_or(Color::Reset);
        let text_bg = self.theme.map(|t| t.background).unwrap_or(Color::Reset);
        let selection_bg = self.theme.map(|t| t.selection).unwrap_or(Color::Blue);

        let default_style = Style::default().fg(text_fg).bg(text_bg);
        let selection_style = Style::default().bg(selection_bg);

        let selection = self.editor.cursor().selection_range();

        // Clear the content area first to prevent ghost characters when scrolling
        for row in 0..area.height {
            for col in 0..text_width {
                let x = text_x + col;
                let y = area.y + row;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(default_style);
                }
            }
        }

        for (screen_row, line_idx) in view.visible_lines().enumerate() {
            if screen_row >= area.height as usize {
                break;
            }

            if line_idx >= buffer.len_lines() {
                continue;
            }

            let line = buffer.line(line_idx).unwrap_or_default();
            let scroll_left = view.scroll_left();

            let y = area.y + screen_row as u16;

            // Track visual column position (accounts for wide chars and tabs)
            let mut visual_col: usize = 0;
            let tab_width: usize = 4;

            for (col_idx, c) in line.chars().enumerate() {
                // Calculate character width
                let char_width = if c == '\t' {
                    // Tab expands to next tab stop
                    tab_width - (visual_col % tab_width)
                } else if c == '\n' {
                    0 // Newline has no width
                } else {
                    c.width().unwrap_or(1)
                };

                // Skip characters that are scrolled off the left
                if visual_col + char_width <= scroll_left {
                    visual_col += char_width;
                    continue;
                }

                // Calculate screen position
                let screen_col = visual_col.saturating_sub(scroll_left);
                if screen_col >= text_width as usize {
                    break;
                }

                // Determine if this position is in selection
                let in_selection = if let Some((start, end)) = selection {
                    let pos = crate::editor::buffer::Position::new(line_idx, col_idx);
                    self.position_in_range(pos, start, end)
                } else {
                    false
                };

                let style = if in_selection {
                    selection_style
                } else {
                    default_style
                };

                // Render the character (or spaces for tabs)
                if c == '\t' {
                    // Render tab as spaces
                    for i in 0..char_width {
                        let x = text_x + (screen_col + i) as u16;
                        if x < text_x + text_width {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(' ');
                                cell.set_style(style);
                            }
                        }
                    }
                } else if c != '\n' {
                    let x = text_x + screen_col as u16;
                    if x < text_x + text_width {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(c);
                            cell.set_style(style);
                        }
                    }
                    // For wide characters, fill the second cell with a space
                    if char_width > 1 {
                        for i in 1..char_width {
                            let x2 = text_x + (screen_col + i) as u16;
                            if x2 < text_x + text_width {
                                if let Some(cell) = buf.cell_mut((x2, y)) {
                                    cell.set_char(' ');
                                    cell.set_style(style);
                                }
                            }
                        }
                    }
                }

                visual_col += char_width;
            }
        }
    }

    /// Checks if a position is within a range.
    fn position_in_range(
        &self,
        pos: crate::editor::buffer::Position,
        start: crate::editor::buffer::Position,
        end: crate::editor::buffer::Position,
    ) -> bool {
        if pos.line < start.line || pos.line > end.line {
            return false;
        }

        if pos.line == start.line && pos.col < start.col {
            return false;
        }

        if pos.line == end.line && pos.col >= end.col {
            return false;
        }

        true
    }

    /// Renders the cursor.
    fn render_cursor(&self, area: Rect, buf: &mut RatatuiBuffer) {
        let view = self.editor.view();
        let cursor_pos = self.editor.cursor_position();

        if let Some((screen_x, screen_y)) = view.buffer_to_screen(cursor_pos) {
            let x = area.x + screen_x;
            let y = area.y + screen_y;

            if x < area.x + area.width && y < area.y + area.height {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    let current_style = cell.style();
                    let cursor_style = match self.editor.mode() {
                        EditorMode::Insert => current_style.add_modifier(Modifier::REVERSED),
                        EditorMode::Normal => current_style.add_modifier(Modifier::REVERSED),
                        EditorMode::Visual => current_style.bg(Color::Magenta),
                        EditorMode::Command => current_style.add_modifier(Modifier::UNDERLINED),
                    };
                    cell.set_style(cursor_style);
                }
            }
        }
    }
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        // Create block with border - use theme if available
        // Fallback colors match EditorTheme::default() for consistency
        let (border_focused, border_unfocused) = self
            .theme
            .map(|t| (t.border_focused, t.border))
            .unwrap_or((Color::Rgb(86, 156, 214), Color::DarkGray));

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

        // Build title with file info
        let path_str = self
            .editor
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[No File]".to_string());

        let modified = if self.editor.is_modified() {
            " [+]"
        } else {
            ""
        };
        let mode = match self.editor.mode() {
            EditorMode::Normal => " NORMAL",
            EditorMode::Insert => " INSERT",
            EditorMode::Visual => " VISUAL",
            EditorMode::Command => " COMMAND",
        };

        let title = format!("{}{}{}", path_str, modified, mode);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // Render editor content
        self.render_line_numbers(inner_area, buf);
        self.render_content(inner_area, buf);

        if self.focused {
            self.render_cursor(inner_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_widget_builder() {
        let editor = Editor::new(80, 24);
        let widget = EditorWidget::new(&editor).focused(true);
        assert!(widget.focused);
    }
}
