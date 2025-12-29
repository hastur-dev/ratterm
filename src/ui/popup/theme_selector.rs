//! Theme selector for choosing color theme.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::theme::ThemePreset;

/// Theme selector state for choosing color theme.
/// Supports both preset themes and custom themes.
pub struct ThemeSelector {
    /// All available theme names (presets + custom).
    theme_names: Vec<String>,
    /// Currently selected theme index.
    selected_index: usize,
    /// Original theme name when selector was opened (for cancel).
    original_theme_name: String,
}

impl ThemeSelector {
    /// Creates a new theme selector with all available themes.
    /// Takes the current theme name and a list of all available themes.
    #[must_use]
    pub fn new_with_themes(current_theme_name: &str, all_themes: Vec<String>) -> Self {
        let selected_index = all_themes
            .iter()
            .position(|t| t == current_theme_name)
            .unwrap_or(0);

        Self {
            theme_names: all_themes,
            selected_index,
            original_theme_name: current_theme_name.to_string(),
        }
    }

    /// Creates a new theme selector starting at the given preset theme.
    /// Only includes preset themes (legacy compatibility).
    #[must_use]
    pub fn new(current_theme: Option<ThemePreset>) -> Self {
        let theme_names: Vec<String> = ThemePreset::all()
            .iter()
            .map(|p| p.name().to_string())
            .collect();
        let current = current_theme.unwrap_or(ThemePreset::Dark);
        let current_name = current.name().to_string();

        let selected_index = theme_names
            .iter()
            .position(|t| t == &current_name)
            .unwrap_or(0);

        Self {
            theme_names,
            selected_index,
            original_theme_name: current_name,
        }
    }

    /// Cycles to the next theme.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.theme_names.len();
    }

    /// Cycles to the previous theme.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.theme_names.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected theme name.
    #[must_use]
    pub fn selected_theme_name(&self) -> &str {
        &self.theme_names[self.selected_index]
    }

    /// Returns the currently selected theme as a preset (if it is one).
    #[must_use]
    pub fn selected_theme(&self) -> ThemePreset {
        ThemePreset::from_name(&self.theme_names[self.selected_index]).unwrap_or(ThemePreset::Dark)
    }

    /// Returns the original theme name (for cancellation).
    #[must_use]
    pub fn original_theme_name(&self) -> &str {
        &self.original_theme_name
    }

    /// Returns the original theme as a preset (if it is one).
    #[must_use]
    pub fn original_theme(&self) -> ThemePreset {
        ThemePreset::from_name(&self.original_theme_name).unwrap_or(ThemePreset::Dark)
    }

    /// Returns all theme names with their selection state.
    #[must_use]
    pub fn themes_with_selection(&self) -> Vec<(&str, bool)> {
        self.theme_names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i == self.selected_index))
            .collect()
    }

    /// Returns the number of themes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.theme_names.len()
    }

    /// Returns whether there are no themes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.theme_names.is_empty()
    }
}

/// Widget for rendering the theme selector popup.
pub struct ThemeSelectorWidget<'a> {
    selector: &'a ThemeSelector,
}

impl<'a> ThemeSelectorWidget<'a> {
    /// Creates a new theme selector widget.
    #[must_use]
    pub fn new(selector: &'a ThemeSelector) -> Self {
        Self { selector }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 40_u16.min(area.width.saturating_sub(4));
        let height = 9_u16.min(area.height.saturating_sub(4)); // Title + 5 themes + padding

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ThemeSelectorWidget<'_> {
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
            .title(" Select Theme ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout for themes
        let theme_count = self.selector.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(theme_count + 1);
        for _ in 0..theme_count {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(1)); // Instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render each theme with explicit backgrounds
        for (i, (name, is_selected)) in self.selector.themes_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            // Capitalize display name for known themes, otherwise use as-is
            let display_name = match *name {
                "dark" => "Dark",
                "light" => "Light",
                "dracula" => "Dracula",
                "gruvbox" => "Gruvbox",
                "nord" => "Nord",
                "matrix" => "Matrix",
                _ => name, // Custom themes keep their name
            };

            let (style, prefix) = if *is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else {
                (Style::default().fg(Color::White).bg(bg_color), "  ")
            };

            let text = format!("{}{}", prefix, display_name);
            let para = Paragraph::new(text)
                .style(style)
                .alignment(Alignment::Center);
            para.render(chunks[i], buf);
        }

        // Render instructions at the bottom with explicit backgrounds
        if chunks.len() > theme_count {
            let instructions = Line::from(vec![
                Span::styled("↑↓", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Select  ", Style::default().fg(Color::White).bg(bg_color)),
                Span::styled("Enter", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Apply  ", Style::default().fg(Color::White).bg(bg_color)),
                Span::styled("Esc", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Cancel", Style::default().fg(Color::White).bg(bg_color)),
            ]);
            Paragraph::new(instructions)
                .alignment(Alignment::Center)
                .render(chunks[theme_count], buf);
        }
    }
}
