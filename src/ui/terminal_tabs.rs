//! Terminal tab bar widget.
//!
//! Displays terminal tabs at the top of the terminal pane.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::terminal::multiplexer::TabInfo;

/// Terminal tab bar widget.
pub struct TerminalTabBar<'a> {
    /// Tab information.
    tabs: &'a [TabInfo],
    /// Whether the terminal pane is focused.
    focused: bool,
}

impl<'a> TerminalTabBar<'a> {
    /// Creates a new terminal tab bar.
    #[must_use]
    pub fn new(tabs: &'a [TabInfo]) -> Self {
        Self {
            tabs,
            focused: false,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for TerminalTabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let bg_color = Color::Black;

        // Render background first with explicit color to prevent Windows rendering artifacts
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(Style::default().fg(Color::White).bg(bg_color));
            }
        }

        // Build the tab line
        let mut spans = Vec::new();
        let tab_count = self.tabs.len();

        for (i, tab) in self.tabs.iter().enumerate() {
            // Tab number (1-indexed for display)
            let tab_num = format!(" {} ", i + 1);

            let style = if tab.is_active {
                if self.focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                }
            } else {
                Style::default().fg(Color::Gray).bg(bg_color)
            };

            spans.push(Span::styled(tab_num, style));

            // Add separator between tabs (except after last) with explicit background
            if i < tab_count - 1 {
                spans.push(Span::styled(
                    "│",
                    Style::default().fg(Color::DarkGray).bg(bg_color),
                ));
            }
        }

        // Fill remaining width with background (hints now shown in KeyHintBar)
        let remaining_width = area.width as usize - spans.iter().map(|s| s.width()).sum::<usize>();
        if remaining_width > 0 {
            spans.push(Span::styled(
                " ".repeat(remaining_width),
                Style::default().fg(Color::DarkGray).bg(bg_color),
            ));
        }

        let line = Line::from(spans);

        // Render the tab line
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::multiplexer::SplitDirection;

    #[test]
    fn test_tab_bar_creation() {
        let tabs = vec![
            TabInfo {
                index: 0,
                name: "Terminal 1".to_string(),
                is_active: true,
                split: SplitDirection::None,
            },
            TabInfo {
                index: 1,
                name: "Terminal 2".to_string(),
                is_active: false,
                split: SplitDirection::None,
            },
        ];

        let bar = TerminalTabBar::new(&tabs).focused(true);
        assert!(bar.focused);
        assert_eq!(bar.tabs.len(), 2);
    }
}
