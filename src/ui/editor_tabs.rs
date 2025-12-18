//! Editor tab bar widget.
//!
//! Displays open file tabs at the top of the editor pane.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Information about an open file tab.
#[derive(Debug, Clone)]
pub struct EditorTabInfo {
    /// Tab index.
    pub index: usize,
    /// File name (short).
    pub name: String,
    /// Whether this tab is active.
    pub is_active: bool,
    /// Whether the file has unsaved changes.
    pub is_modified: bool,
}

/// Editor tab bar widget.
pub struct EditorTabBar<'a> {
    /// Tab information.
    tabs: &'a [EditorTabInfo],
    /// Whether the editor pane is focused.
    focused: bool,
}

impl<'a> EditorTabBar<'a> {
    /// Creates a new editor tab bar.
    #[must_use]
    pub fn new(tabs: &'a [EditorTabInfo]) -> Self {
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

impl Widget for EditorTabBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let bg_color = Color::Black;

        // Render background with explicit color to prevent Windows rendering artifacts
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(Style::default().fg(Color::White).bg(bg_color));
            }
        }

        if self.tabs.is_empty() {
            // Show hint when no files are open with explicit background
            let hint = " Ctrl+O: Open file ";
            let hint_span = Span::styled(hint, Style::default().fg(Color::DarkGray).bg(bg_color));
            let line = Line::from(vec![hint_span]);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        // Build the tab line
        let mut spans = Vec::new();
        let mut total_width = 0usize;
        let max_width = area.width as usize;

        for (i, tab) in self.tabs.iter().enumerate() {
            // Calculate tab text
            let modified_marker = if tab.is_modified { "*" } else { "" };
            let tab_text = format!(" {}{} ", tab.name, modified_marker);
            let tab_width = tab_text.len();

            // Check if we have room
            if total_width + tab_width + 1 > max_width {
                // Add overflow indicator with explicit background
                spans.push(Span::styled("...", Style::default().fg(Color::DarkGray).bg(bg_color)));
                break;
            }

            let style = if tab.is_active {
                if self.focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                }
            } else if tab.is_modified {
                Style::default().fg(Color::Yellow).bg(bg_color)
            } else {
                Style::default().fg(Color::Gray).bg(bg_color)
            };

            spans.push(Span::styled(tab_text, style));
            total_width += tab_width;

            // Add separator between tabs (except after last) with explicit background
            if i < self.tabs.len() - 1 {
                spans.push(Span::styled("│", Style::default().fg(Color::DarkGray).bg(bg_color)));
                total_width += 1;
            }
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_tab_bar_creation() {
        let tabs = vec![
            EditorTabInfo {
                index: 0,
                name: "main.rs".to_string(),
                is_active: true,
                is_modified: false,
            },
            EditorTabInfo {
                index: 1,
                name: "lib.rs".to_string(),
                is_active: false,
                is_modified: true,
            },
        ];

        let bar = EditorTabBar::new(&tabs).focused(true);
        assert!(bar.focused);
        assert_eq!(bar.tabs.len(), 2);
    }

    #[test]
    fn test_empty_tabs() {
        let tabs: Vec<EditorTabInfo> = vec![];
        let bar = EditorTabBar::new(&tabs);
        assert!(bar.tabs.is_empty());
    }
}
