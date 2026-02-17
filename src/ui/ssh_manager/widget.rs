//! SSH Manager widget.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use super::selector::SSHManagerSelector;
use super::types::SSHManagerMode;
use super::widget_forms::{render_add_host, render_edit_name, render_scan_credential_entry};
use super::widget_render::{
    render_authenticated_scanning, render_credential_entry, render_list, render_scanning,
};

/// Widget for rendering the SSH Manager popup.
pub struct SSHManagerWidget<'a> {
    /// Selector state.
    selector: &'a SSHManagerSelector,
    /// Whether the widget is focused.
    focused: bool,
    /// Window position from config.
    position: Option<crate::ui::window_position::WindowPosition>,
}

impl<'a> SSHManagerWidget<'a> {
    /// Creates a new SSH Manager widget.
    #[must_use]
    pub fn new(selector: &'a SSHManagerSelector) -> Self {
        Self {
            selector,
            focused: true,
            position: None,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Sets the window position from config.
    #[must_use]
    pub fn position(mut self, pos: crate::ui::window_position::WindowPosition) -> Self {
        self.position = Some(pos);
        self
    }

    /// Calculates the popup area.
    fn popup_area(&self, area: Rect) -> Rect {
        let width = area.width.saturating_sub(8).min(80);
        let height = area.height.saturating_sub(4).min(20);

        match &self.position {
            Some(pos) => pos.resolve(width, height, area.width, area.height),
            None => {
                let x = (area.width.saturating_sub(width)) / 2;
                let y = (area.height.saturating_sub(height)) / 2;
                Rect::new(x, y, width, height)
            }
        }
    }

    /// Renders the content based on current mode.
    fn render_mode_content(&self, area: Rect, buf: &mut Buffer) {
        match self.selector.mode() {
            SSHManagerMode::List => render_list(self.selector, area, buf),
            SSHManagerMode::Scanning => render_scanning(self.selector, area, buf),
            SSHManagerMode::CredentialEntry => render_credential_entry(self.selector, area, buf),
            SSHManagerMode::Connecting => {
                let text = Paragraph::new("Connecting...")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow));
                text.render(area, buf);
            }
            SSHManagerMode::AddHost => render_add_host(self.selector, area, buf),
            SSHManagerMode::ScanCredentialEntry => {
                render_scan_credential_entry(self.selector, area, buf);
            }
            SSHManagerMode::AuthenticatedScanning => {
                render_authenticated_scanning(self.selector, area, buf);
            }
            SSHManagerMode::EditName => render_edit_name(self.selector, area, buf),
        }
    }
}

impl Widget for SSHManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        Clear.render(popup_area, buf);

        let title = match self.selector.mode() {
            SSHManagerMode::List => "SSH Manager",
            SSHManagerMode::Scanning => "SSH Manager - Scanning",
            SSHManagerMode::CredentialEntry => "SSH Manager - Credentials",
            SSHManagerMode::Connecting => "SSH Manager - Connecting",
            SSHManagerMode::AddHost => "SSH Manager - Add Host",
            SSHManagerMode::ScanCredentialEntry => "SSH Manager - Scan with Credentials",
            SSHManagerMode::AuthenticatedScanning => "SSH Manager - Authenticated Scan",
            SSHManagerMode::EditName => "SSH Manager - Edit Name",
        };

        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(ratatui::text::Span::styled(
                format!(" {} ", title),
                title_style,
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        if let Some(error) = self.selector.error() {
            let error_area = Rect::new(inner.x, inner.y, inner.width, 1);
            let error_para = Paragraph::new(error)
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);
            error_para.render(error_area, buf);

            let remaining = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
            self.render_mode_content(remaining, buf);
        } else {
            self.render_mode_content(inner, buf);
        }
    }
}
