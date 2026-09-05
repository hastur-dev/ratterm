//! LSP signature help popup widget.
//!
//! Renders function signature help with highlighted active parameter.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::lsp::signature::SignatureHelpResult;

/// Minimum width for the signature popup.
const MIN_WIDTH: u16 = 20;

/// Height for the signature popup (borders + 1 line).
const POPUP_HEIGHT: u16 = 3;

/// Widget for displaying signature help.
pub struct LspSignatureWidget<'a> {
    /// The signature help result to display.
    sig_help: &'a SignatureHelpResult,
    /// Cursor X position on screen.
    cursor_x: u16,
    /// Cursor Y position on screen.
    cursor_y: u16,
}

impl<'a> LspSignatureWidget<'a> {
    /// Creates a new signature help widget positioned near the cursor.
    #[must_use]
    pub fn new(sig_help: &'a SignatureHelpResult, cursor_x: u16, cursor_y: u16) -> Self {
        Self {
            sig_help,
            cursor_x,
            cursor_y,
        }
    }

    /// Calculate popup area based on signature content and cursor position.
    #[must_use]
    pub fn calculate_area(&self, screen: Rect) -> Rect {
        assert!(screen.width > 0, "screen width must be positive");
        assert!(screen.height > 0, "screen height must be positive");

        let sig = self.sig_help.signatures.get(self.sig_help.active_signature);
        let width = sig
            .map(|s| s.label.len() as u16 + 4)
            .unwrap_or(30)
            .min(screen.width)
            .max(MIN_WIDTH);

        let y = if self.cursor_y > POPUP_HEIGHT {
            self.cursor_y - POPUP_HEIGHT - 1
        } else {
            (self.cursor_y + 1).min(screen.height.saturating_sub(POPUP_HEIGHT))
        };
        let x = self.cursor_x.min(screen.width.saturating_sub(width));

        Rect::new(x, y, width, POPUP_HEIGHT)
    }

    /// Render the signature popup into the given area.
    pub fn render_in_area(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let sig = match self.sig_help.signatures.get(self.sig_help.active_signature) {
            Some(s) => s,
            None => return,
        };

        // Build the signature line with highlighted active parameter
        let active_param_idx = self.sig_help.active_parameter;
        let spans = build_signature_spans(sig, active_param_idx);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(Line::from(spans)).block(block);
        paragraph.render(area, buf);
    }
}

/// Builds styled spans for a signature, highlighting the active parameter.
fn build_signature_spans<'a>(
    sig: &'a crate::lsp::signature::SignatureInfo,
    active_param_idx: usize,
) -> Vec<Span<'a>> {
    let normal_style = Style::default().fg(Color::White);
    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let param = match sig.parameters.get(active_param_idx) {
        Some(p) => p,
        None => {
            return vec![Span::styled(sig.label.as_str(), normal_style)];
        }
    };

    // Try offset-based highlighting first
    if let (Some(start), Some(end)) = (param.label_start, param.label_end)
        && start < sig.label.len()
        && end <= sig.label.len()
    {
        return vec![
            Span::styled(&sig.label[..start], normal_style),
            Span::styled(&sig.label[start..end], highlight_style),
            Span::styled(&sig.label[end..], normal_style),
        ];
    }

    // Fall back to text-based highlighting
    if !param.label.is_empty()
        && let Some(pos) = sig.label.find(&param.label)
    {
        let end = pos + param.label.len();
        return vec![
            Span::styled(&sig.label[..pos], normal_style),
            Span::styled(&sig.label[pos..end], highlight_style),
            Span::styled(&sig.label[end..], normal_style),
        ];
    }

    vec![Span::styled(sig.label.as_str(), normal_style)]
}
