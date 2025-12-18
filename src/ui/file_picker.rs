//! File picker widget.
//!
//! Displays the file browser for file selection.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::filebrowser::{EntryKind, FileBrowser};

/// File picker widget.
pub struct FilePickerWidget<'a> {
    browser: &'a FileBrowser,
    focused: bool,
}

impl<'a> FilePickerWidget<'a> {
    /// Creates a new file picker widget.
    #[must_use]
    pub fn new(browser: &'a FileBrowser) -> Self {
        Self {
            browser,
            focused: false,
        }
    }

    /// Sets whether the widget is focused.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for FilePickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Draw border with explicit background to prevent Windows rendering artifacts
        let bg_color = Color::Rgb(30, 30, 30);
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan).bg(bg_color)
        } else {
            Style::default().fg(Color::DarkGray).bg(bg_color)
        };

        let title = format!(" {} ", self.browser.path().display());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 2 || inner.height < 1 {
            return;
        }

        // Get entries to display
        let entries = self.browser.filtered_entries();
        let selected = self.browser.selected();
        let scroll_offset = self.browser.scroll_offset();

        // Filter hint if filtering
        let filter = self.browser.filter();
        let header_height = if filter.is_empty() { 0 } else { 1 };

        // Draw filter line if active
        if !filter.is_empty() && inner.height > 0 {
            let filter_line = Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(Color::Yellow)),
                Span::styled(filter, Style::default().fg(Color::White)),
            ]);
            let filter_para = Paragraph::new(filter_line);
            filter_para.render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
        }

        // Calculate visible area for entries
        let entries_area = Rect::new(
            inner.x,
            inner.y + header_height,
            inner.width,
            inner.height.saturating_sub(header_height),
        );

        // Draw entries
        let visible_count = entries_area.height as usize;

        for (i, entry) in entries
            .iter()
            .skip(scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            let y = entries_area.y + i as u16;
            if y >= entries_area.bottom() {
                break;
            }

            let is_selected = scroll_offset + i == selected;

            // Determine style
            let (icon_style, name_style) = if is_selected {
                (
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                    Style::default().bg(Color::Blue).fg(Color::White),
                )
            } else {
                let fg = match entry.kind() {
                    EntryKind::ParentDir => Color::Cyan,
                    EntryKind::Directory => Color::Blue,
                    EntryKind::File => Color::White,
                };
                // Explicit background for non-selected items to prevent Windows rendering artifacts
                (
                    Style::default()
                        .fg(fg)
                        .bg(bg_color)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(fg).bg(bg_color),
                )
            };

            // Build the line
            let icon = entry.icon();
            let name = entry.name();

            // Calculate available space
            let icon_len = icon.len();
            let max_name_len = (entries_area.width as usize).saturating_sub(icon_len + 2);
            let display_name: String = if name.len() > max_name_len {
                format!("{}...", &name[..max_name_len.saturating_sub(3)])
            } else {
                name.to_string()
            };

            let line = Line::from(vec![
                Span::styled(icon, icon_style),
                Span::raw(" "),
                Span::styled(display_name, name_style),
            ]);

            // Fill background for selected item
            if is_selected {
                for x in entries_area.x..entries_area.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_bg(Color::Blue);
                    }
                }
            }

            // Render the line
            let line_para = Paragraph::new(line);
            line_para.render(Rect::new(entries_area.x, y, entries_area.width, 1), buf);
        }

        // Draw scrollbar if needed
        if entries.len() > visible_count {
            let scrollbar_height = entries_area.height.max(1);
            let scroll_ratio =
                scroll_offset as f32 / entries.len().saturating_sub(visible_count).max(1) as f32;
            let thumb_pos = (scroll_ratio * (scrollbar_height - 1) as f32) as u16;

            let scrollbar_x = entries_area.right().saturating_sub(1);
            for y in 0..scrollbar_height {
                let char = if y == thumb_pos { '█' } else { '░' };
                // Explicit background for scrollbar to prevent Windows rendering artifacts
                let style = Style::default().fg(Color::DarkGray).bg(bg_color);
                if let Some(cell) = buf.cell_mut((scrollbar_x, entries_area.y + y)) {
                    cell.set_char(char);
                    cell.set_style(style);
                }
            }
        }
    }
}

/// Compact info bar showing current path and file count.
pub struct FileInfoBar<'a> {
    browser: &'a FileBrowser,
}

impl<'a> FileInfoBar<'a> {
    /// Creates a new file info bar.
    #[must_use]
    pub fn new(browser: &'a FileBrowser) -> Self {
        Self { browser }
    }
}

impl Widget for FileInfoBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let entries = self.browser.filtered_entries();
        let total = entries.len();
        let files = entries.iter().filter(|e| e.is_file()).count();
        let dirs = entries.iter().filter(|e| e.is_directory()).count();

        let info = format!("{} items ({} files, {} dirs)", total, files, dirs);

        // Explicit background to prevent Windows rendering artifacts
        let bg_color = Color::Rgb(30, 30, 30);
        let style = Style::default().fg(Color::DarkGray).bg(bg_color);
        let para = Paragraph::new(info).style(style);
        para.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_picker_widget_creation() {
        let browser = FileBrowser::default();
        let widget = FilePickerWidget::new(&browser);
        assert!(!widget.focused);
    }
}
