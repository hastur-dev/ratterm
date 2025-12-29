//! Shell selector for choosing terminal shell.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::config::{ShellDetector, ShellInstaller, ShellType};

/// Shell selector state for choosing terminal shell.
pub struct ShellSelector {
    /// Available shells with their info.
    shells: Vec<ShellSelectorItem>,
    /// Currently selected shell index.
    selected_index: usize,
    /// Original shell when selector was opened (for cancel).
    original_shell: ShellType,
}

/// An item in the shell selector list.
#[derive(Debug, Clone)]
pub struct ShellSelectorItem {
    /// Shell type.
    pub shell_type: ShellType,
    /// Whether this shell is available.
    pub available: bool,
    /// Version string if available.
    pub version: Option<String>,
}

impl ShellSelector {
    /// Creates a new shell selector starting at the given shell.
    #[must_use]
    pub fn new(current_shell: ShellType) -> Self {
        let detected = ShellDetector::detect_all();
        let platform_shells = ShellType::available_for_platform();

        let mut shells: Vec<ShellSelectorItem> = platform_shells
            .iter()
            .map(|shell_type| {
                let info = detected.iter().find(|d| d.shell_type == *shell_type);
                ShellSelectorItem {
                    shell_type: *shell_type,
                    available: info.is_some_and(|i| i.available),
                    version: info.and_then(|i| i.version.clone()),
                }
            })
            .collect();

        // Add System default at the end
        shells.push(ShellSelectorItem {
            shell_type: ShellType::System,
            available: true,
            version: None,
        });

        let selected_index = shells
            .iter()
            .position(|s| s.shell_type == current_shell)
            .unwrap_or(shells.len().saturating_sub(1)); // Default to System

        Self {
            shells,
            selected_index,
            original_shell: current_shell,
        }
    }

    /// Cycles to the next shell.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.shells.len();
    }

    /// Cycles to the previous shell.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.shells.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected shell type.
    #[must_use]
    pub fn selected_shell(&self) -> ShellType {
        self.shells[self.selected_index].shell_type
    }

    /// Returns the currently selected shell item.
    #[must_use]
    pub fn selected_item(&self) -> &ShellSelectorItem {
        &self.shells[self.selected_index]
    }

    /// Returns true if the currently selected shell is available.
    #[must_use]
    pub fn is_selected_available(&self) -> bool {
        self.shells[self.selected_index].available
    }

    /// Returns the original shell (for cancellation).
    #[must_use]
    pub fn original_shell(&self) -> ShellType {
        self.original_shell
    }

    /// Returns all shells with their selection state.
    #[must_use]
    pub fn shells_with_selection(&self) -> Vec<(&ShellSelectorItem, bool)> {
        self.shells
            .iter()
            .enumerate()
            .map(|(i, shell)| (shell, i == self.selected_index))
            .collect()
    }
}

/// Widget for rendering the shell selector popup.
pub struct ShellSelectorWidget<'a> {
    selector: &'a ShellSelector,
}

impl<'a> ShellSelectorWidget<'a> {
    /// Creates a new shell selector widget.
    #[must_use]
    pub fn new(selector: &'a ShellSelector) -> Self {
        Self { selector }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let shell_count = self.selector.shells.len();
        let width = 50_u16.min(area.width.saturating_sub(4));
        // Height: 2 border + 1 instructions + shell_count lines + 1 padding
        let height = (4 + shell_count as u16).min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ShellSelectorWidget<'_> {
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
            .title(" Select Terminal Shell ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build constraints for each shell + instructions line
        let shell_count = self.selector.shells.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(shell_count + 1);
        for _ in 0..shell_count {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(1)); // Instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render each shell with explicit backgrounds
        for (i, (shell, is_selected)) in self.selector.shells_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let name = shell.shell_type.display_name();
            let status = if shell.available {
                if let Some(ref ver) = shell.version {
                    format!(" ({})", ver)
                } else {
                    String::new()
                }
            } else {
                " [not installed]".to_string()
            };

            let (style, prefix) = if *is_selected {
                let selected_bg = if shell.available {
                    Color::Cyan
                } else {
                    Color::Yellow
                };
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(selected_bg)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else if shell.available {
                (Style::default().fg(Color::White).bg(bg_color), "  ")
            } else {
                (Style::default().fg(Color::DarkGray).bg(bg_color), "  ")
            };

            let text = format!("{}{}{}", prefix, name, status);
            let para = Paragraph::new(text).style(style);
            para.render(chunks[i], buf);
        }

        // Render instructions at the bottom with explicit backgrounds
        if chunks.len() > shell_count {
            let instructions = Line::from(vec![
                Span::styled("↑↓", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Select  ", Style::default().fg(Color::White).bg(bg_color)),
                Span::styled("Enter", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Confirm  ", Style::default().fg(Color::White).bg(bg_color)),
                Span::styled("Esc", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Cancel", Style::default().fg(Color::White).bg(bg_color)),
            ]);
            Paragraph::new(instructions)
                .alignment(Alignment::Center)
                .render(chunks[shell_count], buf);
        }
    }
}

/// Shell install prompt state.
pub struct ShellInstallPrompt {
    /// The shell type that needs installation.
    shell_type: ShellType,
    /// Installation instructions.
    instructions: Vec<String>,
    /// Download URL if available.
    download_url: Option<String>,
    /// Whether user confirmed to proceed.
    confirmed: bool,
}

impl ShellInstallPrompt {
    /// Creates a new shell install prompt.
    #[must_use]
    pub fn new(shell_type: ShellType) -> Self {
        let info = ShellInstaller::get_instructions(shell_type);

        Self {
            shell_type,
            instructions: info.manual_steps,
            download_url: info.download_url,
            confirmed: false,
        }
    }

    /// Returns the shell type.
    #[must_use]
    pub fn shell_type(&self) -> ShellType {
        self.shell_type
    }

    /// Returns the installation instructions.
    #[must_use]
    pub fn instructions(&self) -> &[String] {
        &self.instructions
    }

    /// Returns the download URL if available.
    #[must_use]
    pub fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }

    /// Sets the confirmed state.
    pub fn set_confirmed(&mut self, confirmed: bool) {
        self.confirmed = confirmed;
    }

    /// Returns whether user confirmed.
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

/// Widget for rendering the shell install prompt.
pub struct ShellInstallPromptWidget<'a> {
    prompt: &'a ShellInstallPrompt,
}

impl<'a> ShellInstallPromptWidget<'a> {
    /// Creates a new shell install prompt widget.
    #[must_use]
    pub fn new(prompt: &'a ShellInstallPrompt) -> Self {
        Self { prompt }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let instruction_count = self.prompt.instructions.len();
        let width = 60_u16.min(area.width.saturating_sub(4));
        // Height: 2 border + 2 header + instructions + 2 footer
        let height = (6 + instruction_count as u16).min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ShellInstallPromptWidget<'_> {
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
        let title = format!(" {} Not Installed ", self.prompt.shell_type.display_name());
        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build content
        let instruction_count = self.prompt.instructions.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(instruction_count + 3);
        constraints.push(Constraint::Length(1)); // Header
        for _ in 0..instruction_count {
            constraints.push(Constraint::Length(1));
        }
        if self.prompt.download_url.is_some() {
            constraints.push(Constraint::Length(1)); // URL
        }
        constraints.push(Constraint::Length(1)); // Footer instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render header with explicit background
        let header = "To use this shell, please install it:";
        Paragraph::new(header)
            .style(Style::default().fg(Color::White).bg(bg_color))
            .render(chunks[0], buf);

        // Render instructions with explicit backgrounds
        for (i, instruction) in self.prompt.instructions.iter().enumerate() {
            if i + 1 >= chunks.len() {
                break;
            }
            let text = format!("  {}. {}", i + 1, instruction);
            Paragraph::new(text)
                .style(Style::default().fg(Color::Gray).bg(bg_color))
                .render(chunks[i + 1], buf);
        }

        // Render download URL if available
        let mut footer_idx = instruction_count + 1;
        if let Some(ref url) = self.prompt.download_url {
            if footer_idx < chunks.len() {
                let url_text = format!("  URL: {}", url);
                Paragraph::new(url_text)
                    .style(Style::default().fg(Color::Cyan).bg(bg_color))
                    .render(chunks[footer_idx], buf);
                footer_idx += 1;
            }
        }

        // Render footer instructions with explicit backgrounds
        if footer_idx < chunks.len() {
            let footer = Line::from(vec![
                Span::styled("Esc", Style::default().fg(Color::Cyan).bg(bg_color)),
                Span::styled(" Close  ", Style::default().fg(Color::White).bg(bg_color)),
            ]);
            Paragraph::new(footer)
                .alignment(Alignment::Center)
                .render(chunks[footer_idx], buf);
        }
    }
}
