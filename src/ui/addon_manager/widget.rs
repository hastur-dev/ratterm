//! Add-on Manager widget.
//!
//! Main widget for rendering the add-on manager popup.

use super::selector::AddonManagerSelector;
use super::types::AddonManagerMode;
use super::widget_render::{render_error_view, render_list_mode, render_loading_view};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Widget},
};

/// Widget for rendering the add-on manager.
pub struct AddonManagerWidget<'a> {
    /// Reference to the selector state.
    selector: &'a AddonManagerSelector,
    /// Whether the widget is focused.
    focused: bool,
}

impl<'a> AddonManagerWidget<'a> {
    /// Creates a new add-on manager widget.
    #[must_use]
    pub fn new(selector: &'a AddonManagerSelector) -> Self {
        Self {
            selector,
            focused: true,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Calculates the popup area.
    fn popup_area(&self, area: Rect) -> Rect {
        // 70% width, 70% height, centered
        let width = (area.width * 70 / 100).clamp(40, 80);
        let height = (area.height * 70 / 100).clamp(10, 25);

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for AddonManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);
        let bg_color = Color::Rgb(30, 30, 30);

        // Clear and fill background
        Clear.render(popup_area, buf);
        for y in popup_area.y..popup_area.bottom() {
            for x in popup_area.x..popup_area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }

        // Get title based on mode
        let title = self.selector.mode().title();

        // Get border color based on mode and focus
        let border_color = if !self.focused {
            Color::DarkGray
        } else {
            match self.selector.mode() {
                AddonManagerMode::List => Color::Cyan,
                AddonManagerMode::Fetching | AddonManagerMode::Installing => Color::Yellow,
                AddonManagerMode::ConfirmUninstall => Color::Magenta,
                AddonManagerMode::Error => Color::Red,
            }
        };

        // Draw border
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render content based on mode
        match self.selector.mode() {
            AddonManagerMode::List => {
                render_list_mode(self.selector, inner, buf, bg_color);
            }
            AddonManagerMode::Fetching | AddonManagerMode::Installing => {
                render_loading_view(self.selector, inner, buf, bg_color);
            }
            AddonManagerMode::ConfirmUninstall => {
                render_error_view(
                    "Press Enter to confirm uninstall, Esc to cancel",
                    inner,
                    buf,
                    bg_color,
                );
            }
            AddonManagerMode::Error => {
                let error_msg = self.selector.error().unwrap_or("Unknown error");
                render_error_view(error_msg, inner, buf, bg_color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_creation() {
        let selector = AddonManagerSelector::new();
        let widget = AddonManagerWidget::new(&selector);
        assert!(widget.focused);
    }

    #[test]
    fn test_widget_focused() {
        let selector = AddonManagerSelector::new();
        let widget = AddonManagerWidget::new(&selector).focused(false);
        assert!(!widget.focused);
    }
}
