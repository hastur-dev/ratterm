//! LSP hover popup widget.
//!
//! Renders hover information from the language server as a floating popup.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

use crate::lsp::hover::{HoverResult, hover_to_styled_lines};

/// Maximum width for the hover popup.
const MAX_WIDTH: u16 = 60;

/// Maximum height for the hover popup.
const MAX_HEIGHT: u16 = 15;

/// Padding added to content width for borders and spacing.
const WIDTH_PADDING: u16 = 4;

/// Widget for displaying LSP hover information.
pub struct LspHoverWidget<'a> {
    /// The hover result to display.
    hover: &'a HoverResult,
    /// Cursor X position on screen.
    cursor_x: u16,
    /// Cursor Y position on screen.
    cursor_y: u16,
}

impl<'a> LspHoverWidget<'a> {
    /// Creates a new hover widget positioned near the cursor.
    #[must_use]
    pub fn new(hover: &'a HoverResult, cursor_x: u16, cursor_y: u16) -> Self {
        Self {
            hover,
            cursor_x,
            cursor_y,
        }
    }

    /// Calculates the popup area based on content and cursor position.
    #[must_use]
    pub fn calculate_area(&self, screen: Rect) -> Rect {
        assert!(screen.width > 0, "screen width must be positive");
        assert!(screen.height > 0, "screen height must be positive");

        let styled = hover_to_styled_lines(self.hover);

        let content_width = styled
            .iter()
            .map(|(text, _)| text.len() as u16)
            .max()
            .unwrap_or(10)
            .min(MAX_WIDTH - 2)
            + WIDTH_PADDING;

        // +2 for top and bottom borders
        let content_height = (styled.len() as u16 + 2).min(MAX_HEIGHT);

        let width = content_width.min(screen.width);
        let height = content_height.min(screen.height);

        // Position above cursor if possible, below if not enough space
        let y = if self.cursor_y > height {
            self.cursor_y - height - 1
        } else {
            (self.cursor_y + 1).min(screen.height.saturating_sub(height))
        };

        let x = self.cursor_x.min(screen.width.saturating_sub(width));

        Rect::new(x, y, width, height)
    }

    /// Renders the hover popup into the given area.
    pub fn render_in_area(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let styled = hover_to_styled_lines(self.hover);

        let lines: Vec<Line> = styled
            .iter()
            .map(|(text, is_code)| {
                if *is_code {
                    Line::from(Span::styled(text.clone(), Style::default().fg(Color::Cyan)))
                } else {
                    Line::from(Span::styled(
                        text.clone(),
                        Style::default().fg(Color::White),
                    ))
                }
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Hover ");

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}
