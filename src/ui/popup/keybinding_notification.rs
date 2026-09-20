//! Windows 11 keybinding change notification widget.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Widget for rendering the keybinding change notification popup.
pub struct KeybindingNotificationWidget;

impl KeybindingNotificationWidget {
    /// Creates a new keybinding notification widget.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 56_u16.min(area.width.saturating_sub(4));
        let height = 11_u16.min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Default for KeybindingNotificationWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for KeybindingNotificationWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);
        let bg_color = Color::Rgb(30, 30, 30);

        // Clear background and fill with explicit color
        Clear.render(popup_area, buf);
        for y in popup_area.y..popup_area.bottom() {
            for x in popup_area.x..popup_area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }

        // Draw border with explicit background
        let block = Block::default()
            .title(" Windows Keybinding Notice ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout for message content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Summary
                Constraint::Length(2), // Examples
                Constraint::Length(1), // Spacer
                Constraint::Min(1),    // Dismiss instruction
            ])
            .split(inner);

        // Title
        let title_line = Line::from(vec![Span::styled(
            "Windows Terminal-safe keybindings enabled",
            Style::default()
                .fg(Color::Yellow)
                .bg(bg_color)
                .add_modifier(Modifier::BOLD),
        )]);
        Paragraph::new(title_line)
            .alignment(Alignment::Center)
            .render(chunks[0], buf);

        // Summary
        let old_line = Line::from(vec![Span::styled(
            "Host-reserved shortcuts were replaced on Windows.",
            Style::default().fg(Color::White).bg(bg_color),
        )]);
        Paragraph::new(old_line)
            .alignment(Alignment::Center)
            .render(chunks[2], buf);

        // Examples
        let new_line = Line::from(vec![
            Span::styled(
                "F1",
                Style::default()
                    .fg(Color::Green)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " Palette   Ctrl+F6 Pane   Ctrl+F4 Close Tab",
                Style::default().fg(Color::White).bg(bg_color),
            ),
        ]);
        Paragraph::new(new_line)
            .alignment(Alignment::Center)
            .render(chunks[3], buf);

        // Dismiss instruction
        let dismiss_line = Line::from(vec![Span::styled(
            "Press any key to dismiss",
            Style::default().fg(Color::Cyan).bg(bg_color),
        )]);
        Paragraph::new(dismiss_line)
            .alignment(Alignment::Center)
            .render(chunks[5], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_creation() {
        let widget = KeybindingNotificationWidget::new();
        // Just ensure it can be created
        let _ = widget;
    }
}
