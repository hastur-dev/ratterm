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

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Use theme colors if available
        let bg_color = self.theme.map(|t| t.background).unwrap_or(Color::DarkGray);
        let fg_color = self.theme.map(|t| t.foreground).unwrap_or(Color::White);
        let mode_normal = self.theme.map(|t| t.mode_normal).unwrap_or(Color::Blue);
        let mode_insert = self.theme.map(|t| t.mode_insert).unwrap_or(Color::Green);
        let mode_visual = self.theme.map(|t| t.mode_visual).unwrap_or(Color::Magenta);
        let mode_command = self.theme.map(|t| t.mode_command).unwrap_or(Color::Yellow);

        // Background
        let bg_style = Style::default().bg(bg_color).fg(fg_color);
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }

        // Left side: pane indicator and mode
        let pane_str = match self.focused_pane {
            FocusedPane::Terminal => "TERM",
            FocusedPane::Editor => "EDIT",
        };

        // Show keybinding mode indicator
        let kb_mode_str = match self.keybinding_mode {
            KeybindingMode::Vim => "VIM",
            KeybindingMode::Emacs => "EMACS",
            KeybindingMode::Default => "STD",
        };

        // Show editor mode for vim, or just keybinding mode for others
        let mode_str = match (self.keybinding_mode, self.editor_mode) {
            (KeybindingMode::Vim, Some(EditorMode::Normal)) => "NORMAL",
            (KeybindingMode::Vim, Some(EditorMode::Insert)) => "INSERT",
            (KeybindingMode::Vim, Some(EditorMode::Visual)) => "VISUAL",
            (KeybindingMode::Vim, Some(EditorMode::Command)) => "CMD",
            (KeybindingMode::Emacs, _) => "",
            (KeybindingMode::Default, _) => "",
            _ => "",
        };

        let left_content = if mode_str.is_empty() {
            format!(" {} | {} ", pane_str, kb_mode_str)
        } else {
            format!(" {} | {} {} ", pane_str, kb_mode_str, mode_str)
        };

        // Mode indicator with color
        let mode_style = match self.editor_mode {
            Some(EditorMode::Insert) => Style::default().bg(mode_insert).fg(Color::Black),
            Some(EditorMode::Visual) => Style::default().bg(mode_visual).fg(Color::White),
            Some(EditorMode::Command) => Style::default().bg(mode_command).fg(Color::Black),
            _ => Style::default().bg(mode_normal).fg(Color::White),
        };

        for (i, c) in left_content.chars().enumerate() {
            if i >= area.width as usize {
                break;
            }
            if let Some(cell) = buf.cell_mut((area.x + i as u16, area.y)) {
                cell.set_char(c);
                cell.set_style(mode_style);
            }
        }

        // Center: message or file path
        let center_start = left_content.len() + 1;
        let center_content = if !self.message.is_empty() {
            self.message.to_string()
        } else if let Some(path) = self.file_path {
            path.to_string()
        } else if let Some(title) = self.terminal_title {
            title.to_string()
        } else {
            String::new()
        };

        for (i, c) in center_content.chars().enumerate() {
            let x = area.x + center_start as u16 + i as u16;
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(c);
                cell.set_style(bg_style);
            }
        }

        // Right side: background indicators + cursor position
        let mut right_parts: Vec<(String, Style)> = Vec::new();

        // Background process indicators
        if self.bg_error_count > 0 {
            // Red error indicator
            let error_style = Style::default().bg(Color::Red).fg(Color::White);
            right_parts.push((format!(" ERR:{} ", self.bg_error_count), error_style));
        }
        if self.bg_running_count > 0 {
            // Green running indicator
            let running_style = Style::default().bg(Color::Green).fg(Color::Black);
            right_parts.push((format!(" BG:{} ", self.bg_running_count), running_style));
        }

        // Cursor position
        if let Some(pos) = self.cursor_position {
            right_parts.push((
                format!(" Ln {}, Col {} ", pos.line + 1, pos.col + 1),
                bg_style,
            ));
        }

        // Calculate total width of right parts
        let right_total_width: usize = right_parts.iter().map(|(s, _)| s.len()).sum();

        if right_total_width > 0 && area.width as usize > right_total_width {
            let mut x = area.x + area.width - right_total_width as u16;

            for (content, style) in &right_parts {
                for c in content.chars() {
                    if x >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, area.y)) {
                        cell.set_char(c);
                        cell.set_style(*style);
                    }
                    x += 1;
                }
            }
        }
    }
}

/// Help bar widget for keybinding hints.
pub struct HelpBar<'a> {
    /// Key hints to display.
    hints: &'a [(&'a str, &'a str)],
}

impl<'a> HelpBar<'a> {
    /// Creates a new help bar.
    #[must_use]
    pub fn new(hints: &'a [(&'a str, &'a str)]) -> Self {
        Self { hints }
    }
}

impl Widget for HelpBar<'_> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let bg_style = Style::default().bg(Color::Black).fg(Color::DarkGray);
        let key_style = Style::default().bg(Color::Black).fg(Color::Yellow);

        // Clear background
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }

        let mut x = area.x + 1;

        for (key, desc) in self.hints {
            if x >= area.x + area.width {
                break;
            }

            // Render key
            for c in key.chars() {
                if x >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(c);
                    cell.set_style(key_style);
                }
                x += 1;
            }

            // Render description
            let desc_with_space = format!(" {} ", desc);
            for c in desc_with_space.chars() {
                if x >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(c);
                    cell.set_style(bg_style);
                }
                x += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_builder() {
        let bar = StatusBar::new()
            .message("Test message")
            .focused_pane(FocusedPane::Editor)
            .editor_mode(EditorMode::Insert);

        assert_eq!(bar.message, "Test message");
    }
}
