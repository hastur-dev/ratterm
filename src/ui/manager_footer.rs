//! Reusable two-row key hint footer for manager popups.
//!
//! Used by SSH Manager, Docker Manager, and other popup UIs that need
//! to display primary and secondary action hints.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use super::key_hint_bar::KeyHint;

/// Divider between hints.
const DIVIDER: &str = " \u{2502} ";
/// Divider width.
const DIVIDER_WIDTH: usize = 3;

/// A two-row key hint footer for manager popups.
///
/// Row 1 shows primary actions (connect, add, edit, delete).
/// Row 2 shows secondary/navigation actions (tab, quick connect, esc).
pub struct ManagerFooter<'a> {
    /// Primary action hints (row 1).
    primary_hints: Vec<KeyHint<'a>>,
    /// Secondary action hints (row 2).
    secondary_hints: Vec<KeyHint<'a>>,
}

impl<'a> ManagerFooter<'a> {
    /// Creates a new manager footer with primary hints only.
    #[must_use]
    pub fn new(primary_hints: Vec<KeyHint<'a>>) -> Self {
        Self {
            primary_hints,
            secondary_hints: Vec::new(),
        }
    }

    /// Sets the secondary hints (row 2).
    #[must_use]
    pub fn secondary(mut self, hints: Vec<KeyHint<'a>>) -> Self {
        self.secondary_hints = hints;
        self
    }

    /// Returns the required height (1 or 2 rows).
    #[must_use]
    pub fn height(&self) -> u16 {
        if self.secondary_hints.is_empty() {
            1
        } else {
            2
        }
    }
}

/// Renders a row of key hints into a single line of the buffer.
fn render_hint_row(hints: &[KeyHint], area: Rect, buf: &mut RatatuiBuffer) {
    if area.height == 0 || area.width == 0 || hints.is_empty() {
        return;
    }

    let bar_bg = Style::default().bg(Color::Black).fg(Color::DarkGray);

    // Fill background
    for x in area.x..area.x + area.width {
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_char(' ');
            cell.set_style(bar_bg);
        }
    }

    let available = area.width as usize;
    let mut x = area.x + 1; // 1-char padding

    // Calculate how many hints fit
    let mut total_w: usize = 0;
    let mut fits: usize = 0;

    for (i, hint) in hints.iter().enumerate() {
        let hint_w = hint.key.len() + 2 + hint.description.len() + 1;
        let div_w = if i > 0 { DIVIDER_WIDTH } else { 1 };
        let needed = total_w + div_w + hint_w;
        if needed > available {
            break;
        }
        total_w = needed;
        fits += 1;
    }

    for (i, hint) in hints.iter().take(fits).enumerate() {
        // Divider
        if i > 0 {
            let divider_style = Style::default().fg(Color::DarkGray).bg(Color::Black);
            for c in DIVIDER.chars() {
                if x >= area.x + area.width {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(c);
                    cell.set_style(divider_style);
                }
                x += 1;
            }
        }

        // Badge
        let badge_style = Style::default()
            .bg(hint.style.badge_bg())
            .fg(hint.style.badge_fg())
            .add_modifier(Modifier::BOLD);

        // Space + key + space
        if x < area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(badge_style);
            }
            x += 1;
        }
        for c in hint.key.chars() {
            if x >= area.x + area.width {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(badge_style);
            }
            x += 1;
        }
        if x < area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(badge_style);
            }
            x += 1;
        }

        // Description
        let desc_style = Style::default().fg(Color::Gray).bg(Color::Black);
        for c in hint.description.chars() {
            if x >= area.x + area.width {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(desc_style);
            }
            x += 1;
        }
    }

    // Truncation indicator
    let remaining = hints.len().saturating_sub(fits);
    if remaining > 0 {
        let indicator = format!(" \u{2026}+{}", remaining);
        let style = Style::default().fg(Color::DarkGray).bg(Color::Black);
        for c in indicator.chars() {
            if x >= area.x + area.width {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(style);
            }
            x += 1;
        }
    }
}

impl Widget for ManagerFooter<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Row 1: primary hints
        let row1 = Rect::new(area.x, area.y, area.width, 1);
        render_hint_row(&self.primary_hints, row1, buf);

        // Row 2: secondary hints (if present and area allows)
        if !self.secondary_hints.is_empty() && area.height >= 2 {
            let row2 = Rect::new(area.x, area.y + 1, area.width, 1);
            render_hint_row(&self.secondary_hints, row2, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::key_hint_bar::KeyHintStyle;

    /// Helper: render footer to buffer and extract row text.
    fn render_footer(
        primary: Vec<KeyHint>,
        secondary: Vec<KeyHint>,
        width: u16,
        height: u16,
    ) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = RatatuiBuffer::empty(area);
        let footer = ManagerFooter::new(primary).secondary(secondary);
        footer.render(area, &mut buf);

        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map(|c| c.symbol().chars().next().unwrap_or(' '))
                            .unwrap_or(' ')
                    })
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn test_manager_footer_renders_two_rows() {
        let primary = vec![
            KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
            KeyHint::new("a", "Add"),
        ];
        let secondary = vec![KeyHint::new("Esc", "Close"), KeyHint::new("Tab", "Next")];

        let rows = render_footer(primary, secondary, 80, 2);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("Enter"), "Row 1 missing Enter");
        assert!(rows[0].contains("Connect"), "Row 1 missing Connect");
        assert!(rows[1].contains("Esc"), "Row 2 missing Esc");
        assert!(rows[1].contains("Close"), "Row 2 missing Close");
    }

    #[test]
    fn test_manager_footer_renders_one_row_if_no_secondary() {
        let primary = vec![KeyHint::new("Enter", "Connect")];
        let footer = ManagerFooter::new(primary);
        assert_eq!(footer.height(), 1);
    }

    #[test]
    fn test_manager_footer_badge_styling() {
        let success = KeyHintStyle::Success;
        let danger = KeyHintStyle::Danger;

        assert_eq!(success.badge_bg(), Color::Green);
        assert_eq!(danger.badge_bg(), Color::Red);
    }

    #[test]
    fn test_manager_footer_truncation() {
        let hints: Vec<KeyHint> = vec![
            KeyHint::new("K1", "Act1"),
            KeyHint::new("K2", "Act2"),
            KeyHint::new("K3", "Act3"),
            KeyHint::new("K4", "Act4"),
            KeyHint::new("K5", "Act5"),
            KeyHint::new("K6", "Act6"),
        ];

        let rows = render_footer(hints, vec![], 30, 1);
        // With width=30, not all 6 hints fit, so truncation indicator should appear
        let row = &rows[0];
        // At least some hints should render
        assert!(
            row.contains("K1"),
            "Should contain at least first key: '{}'",
            row
        );
    }
}
