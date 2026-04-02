//! LSP code actions popup widget.
//!
//! Renders a popup menu of available code actions.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::lsp::actions::CodeActionResult;

/// Minimum width for the code actions popup.
const MIN_WIDTH: u16 = 20;

/// Minimum height for the code actions popup.
const MIN_HEIGHT: u16 = 3;

/// Widget for displaying a code action menu.
pub struct LspActionsWidget<'a> {
    /// Available code actions.
    actions: &'a [CodeActionResult],
    /// Currently selected index.
    selected: usize,
}

impl<'a> LspActionsWidget<'a> {
    /// Creates a new code actions widget.
    #[must_use]
    pub fn new(actions: &'a [CodeActionResult], selected: usize) -> Self {
        Self { actions, selected }
    }

    /// Calculate popup area based on actions and cursor position.
    #[must_use]
    pub fn calculate_area(
        &self,
        cursor_x: u16,
        cursor_y: u16,
        screen: Rect,
    ) -> Rect {
        assert!(screen.width > 0, "screen width must be positive");
        assert!(screen.height > 0, "screen height must be positive");

        let max_title_len = self
            .actions
            .iter()
            .map(|a| a.title.len())
            .max()
            .unwrap_or(20);
        // +6 for prefix, borders, and preferred marker
        let width = (max_title_len as u16 + 6)
            .min(screen.width)
            .max(MIN_WIDTH);
        // +2 for borders
        let height = (self.actions.len() as u16 + 2)
            .min(screen.height)
            .max(MIN_HEIGHT);

        let y = (cursor_y + 1).min(screen.height.saturating_sub(height));
        let x = cursor_x.min(screen.width.saturating_sub(width));

        Rect::new(x, y, width, height)
    }

    /// Render the code actions popup into the given area.
    pub fn render_in_area(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Code Actions ");

        let inner = block.inner(area);
        block.render(area, buf);

        let lines: Vec<Line> = self
            .actions
            .iter()
            .enumerate()
            .map(|(idx, action)| {
                let is_selected = idx == self.selected;
                let prefix = if is_selected { "> " } else { "  " };
                let preferred = if action.is_preferred { " *" } else { "" };
                let text = format!("{prefix}{}{preferred}", action.title);

                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                Line::from(Span::styled(text, style))
            })
            .collect();

        Paragraph::new(lines).render(inner, buf);
    }
}
