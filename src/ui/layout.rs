//! Split pane layout management.
//!
//! Handles layout between terminal grid and IDE pane.
//! By default, only terminal is shown. IDE appears when toggled or via open command.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum pane size as percentage.
const MIN_PANE_SIZE: u16 = 10;

/// Default split position as percentage (terminal:IDE).
const DEFAULT_SPLIT: u16 = 50;

/// Split resize step as percentage.
const RESIZE_STEP: u16 = 5;

/// Which pane is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPane {
    /// Terminal pane (left/full).
    #[default]
    Terminal,
    /// Editor pane (right, when IDE visible).
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
/// Manages terminal-first layout with optional IDE pane.
#[derive(Debug, Clone)]
pub struct SplitLayout {
    /// Split position as percentage (0-100) - terminal width when IDE visible.
    split_percent: u16,
    /// Currently focused pane.
    focused: FocusedPane,
    /// Whether to show the terminal pane (always true in new architecture).
    show_terminal: bool,
    /// Whether to show the editor/IDE pane (false by default).
    show_editor: bool,
    /// Whether IDE is visible (separate from show_editor for state management).
    ide_visible: bool,
}

impl SplitLayout {
    /// Creates a new split layout (terminal-first, IDE hidden).
    #[must_use]
    pub fn new() -> Self {
        Self {
            split_percent: DEFAULT_SPLIT,
            focused: FocusedPane::Terminal,
            show_terminal: true,
            show_editor: false, // IDE hidden by default
            ide_visible: false,
        }
    }

    /// Creates a layout with IDE always visible (for ide-always = true).
    #[must_use]
    pub fn with_ide_visible() -> Self {
        Self {
            split_percent: DEFAULT_SPLIT,
            focused: FocusedPane::Terminal,
            show_terminal: true,
            show_editor: true,
            ide_visible: true,
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
        // Only allow focusing editor if IDE is visible
        if pane == FocusedPane::Editor && !self.ide_visible {
            return;
        }
        self.focused = pane;
    }

    /// Toggles between panes (only if IDE is visible).
    pub fn toggle_focus(&mut self) {
        if self.ide_visible {
            self.focused = self.focused.toggle();
        }
    }

    /// Moves split left (increases terminal size).
    pub fn move_split_left(&mut self) {
        if self.ide_visible {
            self.split_percent = self
                .split_percent
                .saturating_sub(RESIZE_STEP)
                .max(MIN_PANE_SIZE);
        }
    }

    /// Moves split right (increases editor size).
    pub fn move_split_right(&mut self) {
        if self.ide_visible {
            self.split_percent = (self.split_percent + RESIZE_STEP).min(100 - MIN_PANE_SIZE);
        }
    }

    /// Sets the split percentage.
    pub fn set_split(&mut self, percent: u16) {
        self.split_percent = percent.clamp(MIN_PANE_SIZE, 100 - MIN_PANE_SIZE);
    }

    /// Shows the IDE pane.
    pub fn show_ide(&mut self) {
        self.ide_visible = true;
        self.show_editor = true;
    }

    /// Hides the IDE pane.
    pub fn hide_ide(&mut self) {
        self.ide_visible = false;
        self.show_editor = false;
        // Focus terminal when IDE is hidden
        self.focused = FocusedPane::Terminal;
    }

    /// Toggles IDE visibility.
    pub fn toggle_ide(&mut self) {
        if self.ide_visible {
            self.hide_ide();
        } else {
            self.show_ide();
        }
    }

    /// Returns true if IDE is visible.
    #[must_use]
    pub const fn ide_visible(&self) -> bool {
        self.ide_visible
    }

    /// Shows only the terminal (fullscreen) - hides IDE.
    pub fn fullscreen_terminal(&mut self) {
        self.show_terminal = true;
        self.show_editor = false;
        self.ide_visible = false;
        self.focused = FocusedPane::Terminal;
    }

    /// Shows only the editor (fullscreen).
    pub fn fullscreen_editor(&mut self) {
        self.show_terminal = false;
        self.show_editor = true;
        self.ide_visible = true;
        self.focused = FocusedPane::Editor;
    }

    /// Shows both panes (enables IDE).
    pub fn show_both(&mut self) {
        self.show_terminal = true;
        self.show_editor = true;
        self.ide_visible = true;
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
                help_bar: Rect::default(),
                status_bar: Rect::default(),
            };
        }

        // Reserve space for status bar (1 row) and help bar (1 row)
        let chrome_height: u16 = 2; // help_bar + status_bar
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(chrome_height),
        };

        let help_bar = Rect {
            x: area.x,
            y: area.y + main_area.height,
            width: area.width,
            height: 1_u16.min(area.height.saturating_sub(main_area.height)),
        };

        let status_bar = Rect {
            x: area.x,
            y: help_bar.y + help_bar.height,
            width: area.width,
            height: 1_u16.min(
                area.height
                    .saturating_sub(main_area.height + help_bar.height),
            ),
        };

        if !self.show_terminal {
            return LayoutAreas {
                terminal: Rect::default(),
                editor: main_area,
                help_bar,
                status_bar,
            };
        }

        if !self.show_editor {
            return LayoutAreas {
                terminal: main_area,
                editor: Rect::default(),
                help_bar,
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
            help_bar,
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
    /// Help/key hint bar area (1 row between content and status bar).
    pub help_bar: Rect,
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
        assert!(!layout.ide_visible()); // IDE hidden by default
    }

    #[test]
    fn test_layout_toggle() {
        let mut layout = SplitLayout::new();
        layout.show_ide(); // Must show IDE first to toggle focus
        layout.toggle_focus();
        assert_eq!(layout.focused(), FocusedPane::Editor);
        layout.toggle_focus();
        assert_eq!(layout.focused(), FocusedPane::Terminal);
    }

    #[test]
    fn test_layout_toggle_without_ide() {
        let mut layout = SplitLayout::new();
        // Toggle should do nothing when IDE is hidden
        layout.toggle_focus();
        assert_eq!(layout.focused(), FocusedPane::Terminal);
    }

    #[test]
    fn test_layout_resize() {
        let mut layout = SplitLayout::new();
        layout.show_ide(); // Must show IDE to resize
        layout.move_split_left();
        assert!(layout.split_percent() < 50);
        layout.move_split_right();
        layout.move_split_right();
        assert!(layout.split_percent() > 50);
    }

    #[test]
    fn test_layout_calculate_terminal_only() {
        let layout = SplitLayout::new();
        let area = Rect::new(0, 0, 100, 50);
        let areas = layout.calculate(area);

        assert!(areas.has_terminal());
        assert!(!areas.has_editor()); // Editor hidden by default
        assert_eq!(areas.terminal.width, 100); // Terminal uses full width
    }

    #[test]
    fn test_layout_calculate_with_ide() {
        let mut layout = SplitLayout::new();
        layout.show_ide();
        let area = Rect::new(0, 0, 100, 50);
        let areas = layout.calculate(area);

        assert!(areas.has_terminal());
        assert!(areas.has_editor());
        assert_eq!(areas.terminal.width + areas.editor.width, 100);
    }

    #[test]
    fn test_layout_has_help_bar() {
        let layout = SplitLayout::new();
        let area = Rect::new(0, 0, 100, 50);
        let areas = layout.calculate(area);

        // Help bar should be 1 row tall, between content and status bar
        assert_eq!(areas.help_bar.height, 1);
        assert!(areas.help_bar.y > areas.terminal.y);
        assert!(areas.help_bar.y < areas.status_bar.y);
    }

    #[test]
    fn test_layout_help_bar_full_width() {
        let layout = SplitLayout::new();
        let area = Rect::new(0, 0, 120, 40);
        let areas = layout.calculate(area);

        assert_eq!(areas.help_bar.width, area.width);
    }

    #[test]
    fn test_ide_toggle() {
        let mut layout = SplitLayout::new();
        assert!(!layout.ide_visible());

        layout.toggle_ide();
        assert!(layout.ide_visible());

        layout.toggle_ide();
        assert!(!layout.ide_visible());
    }
}
