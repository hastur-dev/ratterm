//! Mode switcher for cycling through editor keybinding modes.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::config::KeybindingMode;

/// Mode switcher state for cycling through editor keybinding modes.
pub struct ModeSwitcher {
    /// All available modes in order.
    modes: Vec<KeybindingMode>,
    /// Currently selected mode index.
    selected_index: usize,
    /// Original mode when switcher was opened (for cancel).
    original_mode: KeybindingMode,
}

impl ModeSwitcher {
    /// Creates a new mode switcher starting at the given mode.
    #[must_use]
    pub fn new(current_mode: KeybindingMode) -> Self {
        let modes = vec![
            KeybindingMode::Vim,
            KeybindingMode::Emacs,
            KeybindingMode::Default,
        ];

        let selected_index = modes.iter().position(|m| *m == current_mode).unwrap_or(0);

        Self {
            modes,
            selected_index,
            original_mode: current_mode,
        }
    }

    /// Cycles to the next mode.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.modes.len();
    }

    /// Cycles to the previous mode.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.modes.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected mode.
    #[must_use]
    pub fn selected_mode(&self) -> KeybindingMode {
        self.modes[self.selected_index]
    }

    /// Returns the original mode (for cancellation).
    #[must_use]
    pub fn original_mode(&self) -> KeybindingMode {
        self.original_mode
    }

    /// Returns all modes with their selection state.
    #[must_use]
    pub fn modes_with_selection(&self) -> Vec<(KeybindingMode, bool)> {
        self.modes
            .iter()
            .enumerate()
            .map(|(i, mode)| (*mode, i == self.selected_index))
            .collect()
    }

    /// Returns the display name for a mode.
    #[must_use]
    pub fn mode_name(mode: KeybindingMode) -> &'static str {
        match mode {
            KeybindingMode::Vim => "Vim",
            KeybindingMode::Emacs => "Emacs",
            KeybindingMode::Default => "Default",
        }
    }
}

/// Widget for rendering the mode switcher popup.
pub struct ModeSwitcherWidget<'a> {
    switcher: &'a ModeSwitcher,
}

impl<'a> ModeSwitcherWidget<'a> {
    /// Creates a new mode switcher widget.
    #[must_use]
    pub fn new(switcher: &'a ModeSwitcher) -> Self {
        Self { switcher }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 40_u16.min(area.width.saturating_sub(4));
        let height = 7_u16.min(area.height.saturating_sub(4)); // Title + 3 modes + padding

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ModeSwitcherWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);
        let bg_color = Color::Rgb(30, 30, 30);

        // Clear background and fill with explicit color
        Clear.render(popup_area, buf);
        for y in popup_area.y..popup_area.bottom() {
            for x in popup_area.x..popup_area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }

        // Draw border with explicit background
        let block = Block::default()
            .title(" Switch Editor Mode ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout for modes
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        // Render each mode with explicit backgrounds
        for (i, (mode, is_selected)) in self.switcher.modes_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let name = ModeSwitcher::mode_name(*mode);
            let (style, prefix) = if *is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else {
                (Style::default().fg(Color::DarkGray).bg(bg_color), "  ")
            };

            let text = format!("{}{}", prefix, name);
            let para = Paragraph::new(text)
                .style(style)
                .alignment(Alignment::Center);
            para.render(chunks[i], buf);
        }
    }
}
