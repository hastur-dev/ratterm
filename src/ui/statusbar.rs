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
use crate::editor::{buffer::Position, EditorMode};
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
}

impl<'a> Default for StatusBar<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut RatatuiBuffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Background
        let bg_style = Style::default().bg(Color::DarkGray).fg(Color::White);
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
            Some(EditorMode::Insert) => Style::default().bg(Color::Green).fg(Color::Black),
            Some(EditorMode::Visual) => Style::default().bg(Color::Magenta).fg(Color::White),
            Some(EditorMode::Command) => Style::default().bg(Color::Yellow).fg(Color::Black),
            _ => Style::default().bg(Color::Blue).fg(Color::White),
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

        // Right side: cursor position
        if let Some(pos) = self.cursor_position {
            let right_content = format!(" Ln {}, Col {} ", pos.line + 1, pos.col + 1);
            let start_x = area.x + area.width - right_content.len() as u16;

            for (i, c) in right_content.chars().enumerate() {
                let x = start_x + i as u16;
                if x < area.x || x >= area.x + area.width {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(c);
                    cell.set_style(bg_style);
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

impl<'a> Widget for HelpBar<'a> {
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
