//! Status bar widget.
//!
//! Renders the status bar with mode, position, and messages.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

use crate::config::KeybindingMode;
use crate::editor::{EditorMode, buffer::Position};
use crate::theme::StatusBarTheme;
use crate::ui::layout::FocusedPane;

/// Status bar widget.
pub struct StatusBar<'a> {
    /// Status message.
    message: &'a str,
    /// Focused pane.
    focused_pane: FocusedPane,
    /// Keybinding mode.
    keybinding_mode: KeybindingMode,
    /// Editor mode (if editor is focused).
    editor_mode: Option<EditorMode>,
    /// Cursor position (if editor is focused).
    cursor_position: Option<Position>,
    /// File path (if available).
    file_path: Option<&'a str>,
    /// Terminal title (if terminal is focused).
    terminal_title: Option<&'a str>,
    /// Theme for rendering colors.
    theme: Option<&'a StatusBarTheme>,
    /// Number of running background processes.
    bg_running_count: usize,
    /// Number of background processes with errors.
    bg_error_count: usize,
}

impl<'a> StatusBar<'a> {
    /// Creates a new status bar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            message: "",
            focused_pane: FocusedPane::Terminal,
            keybinding_mode: KeybindingMode::Vim,
            editor_mode: None,
            cursor_position: None,
            file_path: None,
            terminal_title: None,
            theme: None,
            bg_running_count: 0,
            bg_error_count: 0,
        }
    }

    /// Sets the status message.
    #[must_use]
    pub fn message(mut self, message: &'a str) -> Self {
        self.message = message;
        self
    }

    /// Sets the focused pane.
    #[must_use]
    pub fn focused_pane(mut self, pane: FocusedPane) -> Self {
        self.focused_pane = pane;
        self
    }

    /// Sets the keybinding mode.
    #[must_use]
    pub fn keybinding_mode(mut self, mode: KeybindingMode) -> Self {
        self.keybinding_mode = mode;
        self
    }

    /// Sets the editor mode.
    #[must_use]
    pub fn editor_mode(mut self, mode: EditorMode) -> Self {
        self.editor_mode = Some(mode);
        self
    }

    /// Sets the cursor position.
    #[must_use]
    pub fn cursor_position(mut self, pos: Position) -> Self {
        self.cursor_position = Some(pos);
        self
    }

    /// Sets the file path.
    #[must_use]
    pub fn file_path(mut self, path: &'a str) -> Self {
        self.file_path = Some(path);
        self
    }

    /// Sets the terminal title.
    #[must_use]
    pub fn terminal_title(mut self, title: &'a str) -> Self {
        self.terminal_title = Some(title);
        self
    }

    /// Sets the theme.
    #[must_use]
    pub fn theme(mut self, theme: &'a StatusBarTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Sets the background process counts.
    #[must_use]
    pub fn background_processes(mut self, running: usize, errors: usize) -> Self {
        self.bg_running_count = running;
        self.bg_error_count = errors;
        self
    }
}

impl Default for StatusBar<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Separator character for status bar segments.
const SEG_SEPARATOR: char = '\u{2502}';

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Use theme colors if available, with improved defaults
        let bg_color = self
            .theme
            .map(|t| t.background)
            .unwrap_or(Color::Rgb(30, 30, 40));
        let fg_color = self.theme.map(|t| t.foreground).unwrap_or(Color::White);
        let mode_normal = self.theme.map(|t| t.mode_normal).unwrap_or(Color::Blue);
        let mode_insert = self.theme.map(|t| t.mode_insert).unwrap_or(Color::Green);
        let mode_visual = self.theme.map(|t| t.mode_visual).unwrap_or(Color::Magenta);
        let mode_command = self.theme.map(|t| t.mode_command).unwrap_or(Color::Yellow);

        // Background fill
        let bg_style = Style::default().bg(bg_color).fg(fg_color);
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }

        // === Left segment: Mode badge ===
        let pane_str = match self.focused_pane {
            FocusedPane::Terminal => "TERM",
            FocusedPane::Editor => "EDIT",
        };

        let kb_mode_str = match self.keybinding_mode {
            KeybindingMode::Vim => "VIM",
            KeybindingMode::Emacs => "EMACS",
            KeybindingMode::Default => "STD",
        };

        let mode_str = match (self.keybinding_mode, self.editor_mode) {
            (KeybindingMode::Vim, Some(EditorMode::Normal)) => "NORMAL",
            (KeybindingMode::Vim, Some(EditorMode::Insert)) => "INSERT",
            (KeybindingMode::Vim, Some(EditorMode::Visual)) => "VISUAL",
            (KeybindingMode::Vim, Some(EditorMode::Command)) => "CMD",
            _ => "",
        };

        // Build mode badge with padding
        let left_content = if mode_str.is_empty() {
            format!("  {} \u{2502} {}  ", pane_str, kb_mode_str)
        } else {
            format!("  {} \u{2502} {} {}  ", pane_str, kb_mode_str, mode_str)
        };

        let mode_style = match self.editor_mode {
            Some(EditorMode::Insert) => Style::default().bg(mode_insert).fg(Color::Black),
            Some(EditorMode::Visual) => Style::default().bg(mode_visual).fg(Color::White),
            Some(EditorMode::Command) => Style::default().bg(mode_command).fg(Color::Black),
            _ => Style::default().bg(mode_normal).fg(Color::White),
        };

        let mut x = area.x;
        for c in left_content.chars() {
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(mode_style);
            }
            x += 1;
        }

        // Separator after mode badge
        let sep_style = Style::default().fg(Color::DarkGray).bg(bg_color);
        if x < area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(SEG_SEPARATOR);
                cell.set_style(sep_style);
            }
            x += 1;
        }

        // === Center segment: message or file path ===
        let center_start = x;
        let center_content = if !self.message.is_empty() {
            self.message.to_string()
        } else if let Some(path) = self.file_path {
            path.to_string()
        } else if let Some(title) = self.terminal_title {
            title.to_string()
        } else {
            String::new()
        };

        // Build right parts first to know available center width
        let mut right_parts: Vec<(String, Style)> = Vec::new();

        if self.bg_error_count > 0 {
            let error_style = Style::default().bg(Color::Red).fg(Color::White);
            right_parts.push((format!(" ERR:{} ", self.bg_error_count), error_style));
        }
        if self.bg_running_count > 0 {
            let running_style = Style::default().bg(Color::Green).fg(Color::Black);
            right_parts.push((format!(" BG:{} ", self.bg_running_count), running_style));
        }
        if let Some(pos) = self.cursor_position {
            // Add separator before cursor position
            right_parts.push((
                format!(
                    "{} Ln {}, Col {} ",
                    SEG_SEPARATOR,
                    pos.line + 1,
                    pos.col + 1
                ),
                bg_style,
            ));
        }

        let right_total_width: usize = right_parts.iter().map(|(s, _)| s.len()).sum();
        let available_center = (area.width as usize)
            .saturating_sub((center_start - area.x) as usize)
            .saturating_sub(right_total_width)
            .saturating_sub(1); // 1 for padding

        // Truncate center content if needed
        let truncated_center = if center_content.len() > available_center && available_center > 3 {
            format!(
                " {}\u{2026}",
                &center_content[..available_center.saturating_sub(2)]
            )
        } else if !center_content.is_empty() {
            format!(" {}", center_content)
        } else {
            String::new()
        };

        for c in truncated_center.chars() {
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(bg_style);
            }
            x += 1;
        }

        // === Right segments: background indicators + cursor position ===
        if right_total_width > 0 && area.width as usize > right_total_width {
            let mut rx = area.x + area.width - right_total_width as u16;

            for (content, style) in &right_parts {
                for c in content.chars() {
                    if rx >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((rx, area.y)) {
                        cell.set_char(c);
                        cell.set_style(*style);
                    }
                    rx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer as TestBuffer;

    /// Helper: render status bar to buffer and return text content.
    fn render_status_bar_to_string(bar: StatusBar, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = TestBuffer::empty(area);
        bar.render(area, &mut buf);

        (0..width)
            .map(|x| {
                buf.cell((x, 0))
                    .map(|c| c.symbol().chars().next().unwrap_or(' '))
                    .unwrap_or(' ')
            })
            .collect()
    }

    #[test]
    fn test_status_bar_builder() {
        let bar = StatusBar::new()
            .message("Test message")
            .focused_pane(FocusedPane::Editor)
            .editor_mode(EditorMode::Insert);

        assert_eq!(bar.message, "Test message");
    }

    #[test]
    fn test_status_bar_mode_badge_colors() {
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = TestBuffer::empty(area);

        let bar = StatusBar::new()
            .focused_pane(FocusedPane::Editor)
            .editor_mode(EditorMode::Insert);
        bar.render(area, &mut buf);

        // The mode badge should have a non-default background color
        if let Some(cell) = buf.cell((2, 0)) {
            let bg = cell.bg;
            // Insert mode should use green (or theme's insert color)
            assert_ne!(bg, Color::Reset, "Mode badge should have colored bg");
        }
    }

    #[test]
    fn test_status_bar_segments_have_separators() {
        let content = render_status_bar_to_string(
            StatusBar::new()
                .focused_pane(FocusedPane::Terminal)
                .keybinding_mode(KeybindingMode::Vim),
            80,
        );

        // Should contain the separator character between pane and mode
        assert!(
            content.contains(SEG_SEPARATOR),
            "Status bar should have separator: '{}'",
            content
        );
    }

    #[test]
    fn test_status_bar_truncates_long_paths() {
        let long_path = "a".repeat(200);
        let content = render_status_bar_to_string(
            StatusBar::new()
                .focused_pane(FocusedPane::Editor)
                .file_path(&long_path),
            80,
        );

        // Should contain truncation indicator
        assert!(
            content.contains('\u{2026}') || content.len() <= 80,
            "Long path should be truncated: '{}'",
            content
        );
    }

    #[test]
    fn test_status_bar_background_indicators() {
        let content = render_status_bar_to_string(StatusBar::new().background_processes(3, 0), 80);

        assert!(content.contains("BG:3"), "Should show BG:3: '{}'", content);
    }

    #[test]
    fn test_status_bar_mode_badge_has_padding() {
        let content = render_status_bar_to_string(
            StatusBar::new()
                .focused_pane(FocusedPane::Terminal)
                .keybinding_mode(KeybindingMode::Vim),
            80,
        );

        // The mode badge should have spaces around TERM and VIM
        assert!(
            content.contains("  TERM"),
            "Badge should have left padding: '{}'",
            content
        );
    }
}
