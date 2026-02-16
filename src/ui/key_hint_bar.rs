//! Context-aware key hint bar widget.
//!
//! Renders styled key badges with descriptions at the bottom of the screen,
//! adapting to the current context (terminal, editor, popup).

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// Style variant for a key hint badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyHintStyle {
    /// Default gray badge.
    #[default]
    Normal,
    /// Cyan badge for primary actions.
    Highlighted,
    /// Red badge for destructive actions.
    Danger,
    /// Green badge for connection/confirmation.
    Success,
}

impl KeyHintStyle {
    /// Returns the background color for the key badge.
    #[must_use]
    pub const fn badge_bg(&self) -> Color {
        match self {
            Self::Normal => Color::DarkGray,
            Self::Highlighted => Color::Cyan,
            Self::Danger => Color::Red,
            Self::Success => Color::Green,
        }
    }

    /// Returns the foreground color for the key badge text.
    #[must_use]
    pub const fn badge_fg(&self) -> Color {
        match self {
            Self::Normal | Self::Highlighted => Color::White,
            Self::Danger => Color::White,
            Self::Success => Color::Black,
        }
    }
}

/// A single key hint with key text, description, and style.
#[derive(Debug, Clone)]
pub struct KeyHint<'a> {
    /// Key combination text (e.g., "Ctrl+P").
    pub key: &'a str,
    /// Description of the action (e.g., "Palette").
    pub description: &'a str,
    /// Visual style for the badge.
    pub style: KeyHintStyle,
}

impl<'a> KeyHint<'a> {
    /// Creates a new key hint with Normal style.
    #[must_use]
    pub const fn new(key: &'a str, description: &'a str) -> Self {
        Self {
            key,
            description,
            style: KeyHintStyle::Normal,
        }
    }

    /// Creates a new key hint with a specific style.
    #[must_use]
    pub const fn styled(key: &'a str, description: &'a str, style: KeyHintStyle) -> Self {
        Self {
            key,
            description,
            style,
        }
    }

    /// Returns the total display width of this hint (key + space + desc).
    fn display_width(&self) -> usize {
        // " key " + " desc "
        self.key.len() + 2 + self.description.len() + 1
    }
}

/// Context-aware key hint bar widget.
///
/// Renders a row of styled key badges separated by thin dividers.
/// If hints overflow the available width, truncates with a `…+N more` indicator.
pub struct KeyHintBar<'a> {
    /// Hints to display.
    hints: Vec<KeyHint<'a>>,
}

impl<'a> KeyHintBar<'a> {
    /// Creates a new key hint bar with the given hints.
    #[must_use]
    pub fn new(hints: Vec<KeyHint<'a>>) -> Self {
        Self { hints }
    }
}

/// Divider between hints.
const DIVIDER: &str = " \u{2502} ";
/// Divider width.
const DIVIDER_WIDTH: usize = 3;

impl Widget for KeyHintBar<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
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

        if self.hints.is_empty() {
            return;
        }

        let available = area.width as usize;

        // Calculate how many hints fit
        let mut total_width: usize = 0;
        let mut fits_count: usize = 0;

        for (i, hint) in self.hints.iter().enumerate() {
            let hint_w = hint.display_width();
            let divider_w = if i > 0 { DIVIDER_WIDTH } else { 1 }; // leading space or divider
            let needed = total_width + divider_w + hint_w;

            if needed > available {
                break;
            }
            total_width = needed;
            fits_count += 1;
        }

        let remaining = self.hints.len().saturating_sub(fits_count);

        // If we need a truncation indicator, check if it fits
        if remaining > 0 && fits_count > 0 {
            let indicator = format!(" …+{}", remaining);
            let indicator_w = indicator.len();

            // Remove hints until indicator fits
            while fits_count > 0 {
                let mut w: usize = 0;
                for (i, hint) in self.hints.iter().take(fits_count).enumerate() {
                    let divider_w = if i > 0 { DIVIDER_WIDTH } else { 1 };
                    w += divider_w + hint.display_width();
                }
                if w + indicator_w <= available {
                    break;
                }
                fits_count -= 1;
            }
        }

        // Render hints
        let mut x = area.x + 1; // 1-char left padding

        for (i, hint) in self.hints.iter().take(fits_count).enumerate() {
            // Divider between hints (not before first)
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

            // Key badge: " key " with colored background
            let badge_style = Style::default()
                .bg(hint.style.badge_bg())
                .fg(hint.style.badge_fg())
                .add_modifier(Modifier::BOLD);

            // Space before key text
            if x < area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(' ');
                    cell.set_style(badge_style);
                }
                x += 1;
            }

            // Key text
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

            // Space after key text (still badge bg)
            if x < area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(' ');
                    cell.set_style(badge_style);
                }
                x += 1;
            }

            // Description text
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

        // Render truncation indicator if needed
        let actual_remaining = self.hints.len().saturating_sub(fits_count);
        if actual_remaining > 0 {
            let indicator = format!(" \u{2026}+{}", actual_remaining);
            let indicator_style = Style::default().fg(Color::DarkGray).bg(Color::Black);
            for c in indicator.chars() {
                if x >= area.x + area.width {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(c);
                    cell.set_style(indicator_style);
                }
                x += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: render a KeyHintBar to a buffer and return the text content.
    fn render_to_string(hints: Vec<KeyHint>, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = RatatuiBuffer::empty(area);
        KeyHintBar::new(hints).render(area, &mut buf);

        (0..width)
            .map(|x| {
                buf.cell((x, 0))
                    .map(|c| c.symbol().chars().next().unwrap_or(' '))
                    .unwrap_or(' ')
            })
            .collect::<String>()
    }

    #[test]
    fn test_key_hint_bar_renders_empty() {
        let content = render_to_string(vec![], 80);
        // Should be all spaces (background filled)
        assert_eq!(content.trim(), "");
    }

    #[test]
    fn test_key_hint_bar_renders_single_hint() {
        let hints = vec![KeyHint::new("Ctrl+P", "Palette")];
        let content = render_to_string(hints, 80);
        assert!(
            content.contains("Ctrl+P"),
            "Should contain key text: '{}'",
            content
        );
        assert!(
            content.contains("Palette"),
            "Should contain description: '{}'",
            content
        );
    }

    #[test]
    fn test_key_hint_bar_renders_multiple_hints() {
        let hints = vec![
            KeyHint::new("Ctrl+P", "Palette"),
            KeyHint::new("Ctrl+Q", "Quit"),
            KeyHint::new("Ctrl+S", "Save"),
        ];
        let content = render_to_string(hints, 80);
        assert!(content.contains("Ctrl+P"), "Missing first key");
        assert!(content.contains("Quit"), "Missing second desc");
        assert!(content.contains("Save"), "Missing third desc");
        // Should contain divider character
        assert!(
            content.contains('\u{2502}'),
            "Missing divider: '{}'",
            content
        );
    }

    #[test]
    fn test_key_hint_bar_truncation() {
        let hints = vec![
            KeyHint::new("Ctrl+P", "Palette"),
            KeyHint::new("Ctrl+Q", "Quit"),
            KeyHint::new("Ctrl+S", "Save"),
            KeyHint::new("Ctrl+T", "New Tab"),
            KeyHint::new("Ctrl+O", "Open"),
            KeyHint::new("Ctrl+F", "Find"),
            KeyHint::new("Ctrl+W", "Close"),
            KeyHint::new("Alt+Tab", "Switch"),
            KeyHint::new("Ctrl+D", "Docker"),
            KeyHint::new("Ctrl+U", "SSH"),
        ];
        // Very narrow area — can't fit all 10 hints
        let content = render_to_string(hints, 40);
        // Should contain truncation indicator
        assert!(
            content.contains('\u{2026}'),
            "Should show truncation indicator: '{}'",
            content
        );
    }

    #[test]
    fn test_key_hint_style_colors() {
        // Verify each style variant maps to distinct colors
        let normal = KeyHintStyle::Normal;
        let highlighted = KeyHintStyle::Highlighted;
        let danger = KeyHintStyle::Danger;
        let success = KeyHintStyle::Success;

        assert_eq!(normal.badge_bg(), Color::DarkGray);
        assert_eq!(highlighted.badge_bg(), Color::Cyan);
        assert_eq!(danger.badge_bg(), Color::Red);
        assert_eq!(success.badge_bg(), Color::Green);

        // All should have distinct backgrounds
        let bgs = [
            normal.badge_bg(),
            highlighted.badge_bg(),
            danger.badge_bg(),
            success.badge_bg(),
        ];
        for i in 0..bgs.len() {
            for j in (i + 1)..bgs.len() {
                assert_ne!(bgs[i], bgs[j], "Styles {} and {} share bg color", i, j);
            }
        }
    }
}
