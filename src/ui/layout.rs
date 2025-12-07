//! Split pane layout management.
//!
//! Handles horizontal split between terminal and editor panes.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum pane size as percentage.
const MIN_PANE_SIZE: u16 = 10;

/// Default split position as percentage.
const DEFAULT_SPLIT: u16 = 50;

/// Split resize step as percentage.
const RESIZE_STEP: u16 = 5;

/// Which pane is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPane {
    /// Terminal pane (left).
    #[default]
    Terminal,
    /// Editor pane (right).
    Editor,
}

impl FocusedPane {
    /// Toggles between panes.
    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Terminal => Self::Editor,
            Self::Editor => Self::Terminal,
        }
    }
}

/// Split layout manager.
#[derive(Debug, Clone)]
pub struct SplitLayout {
    /// Split position as percentage (0-100).
    split_percent: u16,
    /// Currently focused pane.
    focused: FocusedPane,
    /// Whether to show the terminal pane.
    show_terminal: bool,
    /// Whether to show the editor pane.
    show_editor: bool,
}

impl SplitLayout {
    /// Creates a new split layout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            split_percent: DEFAULT_SPLIT,
            focused: FocusedPane::Terminal,
            show_terminal: true,
            show_editor: true,
        }
    }

    /// Returns the split percentage.
    #[must_use]
    pub const fn split_percent(&self) -> u16 {
        self.split_percent
    }

    /// Returns the focused pane.
    #[must_use]
    pub const fn focused(&self) -> FocusedPane {
        self.focused
    }

    /// Sets the focused pane.
    pub fn set_focused(&mut self, pane: FocusedPane) {
        self.focused = pane;
    }

    /// Toggles between panes.
    pub fn toggle_focus(&mut self) {
        self.focused = self.focused.toggle();
    }

    /// Moves split left (increases terminal size).
    pub fn move_split_left(&mut self) {
        self.split_percent = self
            .split_percent
            .saturating_sub(RESIZE_STEP)
            .max(MIN_PANE_SIZE);
    }

    /// Moves split right (increases editor size).
    pub fn move_split_right(&mut self) {
        self.split_percent = (self.split_percent + RESIZE_STEP).min(100 - MIN_PANE_SIZE);
    }

    /// Sets the split percentage.
    pub fn set_split(&mut self, percent: u16) {
        self.split_percent = percent.clamp(MIN_PANE_SIZE, 100 - MIN_PANE_SIZE);
    }

    /// Shows only the terminal (fullscreen).
    pub fn fullscreen_terminal(&mut self) {
        self.show_terminal = true;
        self.show_editor = false;
        self.focused = FocusedPane::Terminal;
    }

    /// Shows only the editor (fullscreen).
    pub fn fullscreen_editor(&mut self) {
        self.show_terminal = false;
        self.show_editor = true;
        self.focused = FocusedPane::Editor;
    }

    /// Shows both panes.
    pub fn show_both(&mut self) {
        self.show_terminal = true;
        self.show_editor = true;
    }

    /// Returns true if terminal is visible.
    #[must_use]
    pub const fn terminal_visible(&self) -> bool {
        self.show_terminal
    }

    /// Returns true if editor is visible.
    #[must_use]
    pub const fn editor_visible(&self) -> bool {
        self.show_editor
    }

    /// Calculates the layout areas.
    #[must_use]
    pub fn calculate(&self, area: Rect) -> LayoutAreas {
        if !self.show_terminal && !self.show_editor {
            return LayoutAreas {
                terminal: Rect::default(),
                editor: Rect::default(),
                status_bar: Rect::default(),
            };
        }

        // Reserve space for status bar
        let status_height = 1;
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(status_height),
        };

        let status_bar = Rect {
            x: area.x,
            y: area.y + main_area.height,
            width: area.width,
            height: status_height.min(area.height),
        };

        if !self.show_terminal {
            return LayoutAreas {
                terminal: Rect::default(),
                editor: main_area,
                status_bar,
            };
        }

        if !self.show_editor {
            return LayoutAreas {
                terminal: main_area,
                editor: Rect::default(),
                status_bar,
            };
        }

        // Calculate split
        let constraints = [
            Constraint::Percentage(self.split_percent),
            Constraint::Percentage(100 - self.split_percent),
        ];

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);

        LayoutAreas {
            terminal: chunks[0],
            editor: chunks[1],
            status_bar,
        }
    }
}

impl Default for SplitLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculated layout areas.
#[derive(Debug, Clone, Copy)]
pub struct LayoutAreas {
    /// Terminal pane area.
    pub terminal: Rect,
    /// Editor pane area.
    pub editor: Rect,
    /// Status bar area.
    pub status_bar: Rect,
}

impl LayoutAreas {
    /// Returns true if terminal has area.
    #[must_use]
    pub const fn has_terminal(&self) -> bool {
        self.terminal.width > 0 && self.terminal.height > 0
    }

    /// Returns true if editor has area.
    #[must_use]
    pub const fn has_editor(&self) -> bool {
        self.editor.width > 0 && self.editor.height > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_new() {
        let layout = SplitLayout::new();
        assert_eq!(layout.split_percent(), 50);
        assert_eq!(layout.focused(), FocusedPane::Terminal);
    }

    #[test]
    fn test_layout_toggle() {
        let mut layout = SplitLayout::new();
        layout.toggle_focus();
        assert_eq!(layout.focused(), FocusedPane::Editor);
        layout.toggle_focus();
        assert_eq!(layout.focused(), FocusedPane::Terminal);
    }

    #[test]
    fn test_layout_resize() {
        let mut layout = SplitLayout::new();
        layout.move_split_left();
        assert!(layout.split_percent() < 50);
        layout.move_split_right();
        layout.move_split_right();
        assert!(layout.split_percent() > 50);
    }

    #[test]
    fn test_layout_calculate() {
        let layout = SplitLayout::new();
        let area = Rect::new(0, 0, 100, 50);
        let areas = layout.calculate(area);

        assert!(areas.has_terminal());
        assert!(areas.has_editor());
        assert_eq!(areas.terminal.width + areas.editor.width, 100);
    }
}
