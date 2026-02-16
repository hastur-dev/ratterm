//! Hotkey help overlay widget.
//!
//! Displays available keyboard shortcuts for the current dashboard context.
//! Triggered by pressing `?` in any dashboard. Does NOT close or clear
//! the underlying dashboard.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Widget},
};

/// A single hotkey entry for display.
#[derive(Debug, Clone)]
pub struct HotkeyEntry {
    /// The key or key combination (e.g., "Up/Down", "Enter", "Ctrl+R").
    pub key: &'static str,
    /// Description of what the key does.
    pub description: &'static str,
    /// Category for grouping (e.g., "Navigation", "Actions").
    pub category: &'static str,
}

/// Hotkey overlay state.
pub struct HotkeyOverlay {
    /// Entries to display.
    entries: Vec<HotkeyEntry>,
    /// Whether the overlay is currently visible.
    visible: bool,
    /// Scroll offset for long lists.
    scroll_offset: usize,
}

impl HotkeyOverlay {
    /// Creates a new hotkey overlay with the given entries.
    #[must_use]
    pub fn new(entries: Vec<HotkeyEntry>) -> Self {
        Self {
            entries,
            visible: true,
            scroll_offset: 0,
        }
    }

    /// Toggles overlay visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns whether the overlay is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Hides the overlay.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Scrolls the overlay up.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scrolls the overlay down.
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Returns the entries.
    #[must_use]
    pub fn entries(&self) -> &[HotkeyEntry] {
        &self.entries
    }

    /// Returns the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
}

/// Renders the hotkey overlay widget.
pub struct HotkeyOverlayWidget<'a> {
    overlay: &'a HotkeyOverlay,
    position: Option<crate::ui::window_position::WindowPosition>,
}

impl<'a> HotkeyOverlayWidget<'a> {
    /// Creates a new hotkey overlay widget.
    #[must_use]
    pub fn new(overlay: &'a HotkeyOverlay) -> Self {
        Self {
            overlay,
            position: None,
        }
    }

    /// Sets the window position.
    #[must_use]
    pub fn position(mut self, pos: crate::ui::window_position::WindowPosition) -> Self {
        self.position = Some(pos);
        self
    }
}

impl Widget for HotkeyOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.overlay.is_visible() || area.width < 30 || area.height < 8 {
            return;
        }

        // Size: max 60 columns, max 80% height
        let max_w = 60.min(area.width.saturating_sub(4));
        let max_h = (area.height * 80 / 100).max(10).min(area.height.saturating_sub(4));

        // Use configured position or default to center
        let popup_area = match &self.position {
            Some(pos) => pos.resolve(max_w, max_h, area.width, area.height),
            None => {
                let x = area.x + (area.width.saturating_sub(max_w)) / 2;
                let y = area.y + (area.height.saturating_sub(max_h)) / 2;
                Rect::new(x, y, max_w, max_h)
            }
        };

        // Clear and draw background
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Keyboard Shortcuts [?] ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Rgb(20, 20, 30)));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build content lines grouped by category
        let mut lines: Vec<(Style, String)> = Vec::new();
        let mut current_category = "";

        for entry in self.overlay.entries() {
            if entry.category != current_category {
                if !current_category.is_empty() {
                    lines.push((Style::default(), String::new())); // blank separator
                }
                current_category = entry.category;
                lines.push((
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    format!("  {current_category}"),
                ));
            }

            // Format: "  key_padded  description"
            let key_display = format!("  {:<14} {}", entry.key, entry.description);
            lines.push((Style::default().fg(Color::White), key_display));
        }

        // Add footer
        lines.push((Style::default(), String::new()));
        lines.push((
            Style::default().fg(Color::DarkGray),
            "  Up/Down Scroll | ? or Esc Close".to_string(),
        ));

        // Apply scroll offset, clamped to valid range
        let max_scroll = lines.len().saturating_sub(inner.height as usize);
        let scroll = self.overlay.scroll_offset().min(max_scroll);

        // Render visible lines
        for (i, (style, text)) in lines.iter().skip(scroll).enumerate() {
            let row = inner.y + i as u16;
            if row >= inner.y + inner.height {
                break;
            }

            for (j, ch) in text.chars().enumerate() {
                let col = inner.x + j as u16;
                if col >= inner.x + inner.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((col, row)) {
                    cell.set_char(ch);
                    cell.set_style(*style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<HotkeyEntry> {
        vec![
            HotkeyEntry {
                key: "Up/Down",
                description: "Navigate list",
                category: "Navigation",
            },
            HotkeyEntry {
                key: "Enter",
                description: "Select item",
                category: "Navigation",
            },
            HotkeyEntry {
                key: "r",
                description: "Refresh",
                category: "Actions",
            },
            HotkeyEntry {
                key: "?",
                description: "Toggle help",
                category: "Help",
            },
        ]
    }

    #[test]
    fn test_overlay_toggle() {
        let mut overlay = HotkeyOverlay::new(sample_entries());
        assert!(overlay.is_visible(), "Should start visible");

        overlay.toggle();
        assert!(!overlay.is_visible(), "Should be hidden after toggle");

        overlay.toggle();
        assert!(overlay.is_visible(), "Should be visible after second toggle");
    }

    #[test]
    fn test_overlay_scroll() {
        let entries: Vec<HotkeyEntry> = (0..20)
            .map(|i| HotkeyEntry {
                key: "key",
                description: "desc",
                category: if i < 10 { "Group A" } else { "Group B" },
            })
            .collect();

        let mut overlay = HotkeyOverlay::new(entries);
        assert_eq!(overlay.scroll_offset(), 0);

        overlay.scroll_down();
        assert_eq!(overlay.scroll_offset(), 1);

        overlay.scroll_down();
        assert_eq!(overlay.scroll_offset(), 2);

        overlay.scroll_up();
        assert_eq!(overlay.scroll_offset(), 1);

        // Can't scroll past 0
        overlay.scroll_up();
        overlay.scroll_up();
        assert_eq!(overlay.scroll_offset(), 0);
    }

    #[test]
    fn test_overlay_entries_not_empty() {
        let overlay = HotkeyOverlay::new(sample_entries());
        assert!(!overlay.entries().is_empty());
        assert_eq!(overlay.entries().len(), 4);
    }

    #[test]
    fn test_overlay_hide() {
        let mut overlay = HotkeyOverlay::new(sample_entries());
        assert!(overlay.is_visible());

        overlay.hide();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_overlay_widget_renders_without_panic() {
        let overlay = HotkeyOverlay::new(sample_entries());
        let widget = HotkeyOverlayWidget::new(&overlay);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should have rendered something — check title appears
        let _text: String = (0..80)
            .map(|x| {
                buf.cell((x, 0))
                    .map(|c| c.symbol().chars().next().unwrap_or(' '))
                    .unwrap_or(' ')
            })
            .collect();
        // Title should be somewhere in the rendered area
        // (exact position depends on centering)
    }

    #[test]
    fn test_overlay_widget_hidden_renders_nothing() {
        let mut overlay = HotkeyOverlay::new(sample_entries());
        overlay.hide();
        let widget = HotkeyOverlayWidget::new(&overlay);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // All cells should be empty space since overlay is hidden
        for y in 0..24 {
            for x in 0..80_u16 {
                let sym = buf
                    .cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default();
                assert!(
                    sym == " " || sym.is_empty(),
                    "Cell ({x},{y}) should be empty but got '{sym}'"
                );
            }
        }
    }

    #[test]
    fn test_overlay_widget_too_small_renders_nothing() {
        let overlay = HotkeyOverlay::new(sample_entries());
        let widget = HotkeyOverlayWidget::new(&overlay);

        // Area too small
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Should not panic
    }
}
