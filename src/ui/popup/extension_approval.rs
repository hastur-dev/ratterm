//! Extension approval prompt for first-time extension load.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Extension approval prompt state.
pub struct ExtensionApprovalPrompt {
    /// Extension name.
    name: String,
    /// Extension version.
    version: String,
    /// Extension author (if available).
    author: Option<String>,
    /// Extension description (if available).
    description: Option<String>,
    /// Command that will be run.
    command: String,
}

impl ExtensionApprovalPrompt {
    /// Creates a new extension approval prompt.
    #[must_use]
    pub fn new(
        name: String,
        version: String,
        author: Option<String>,
        description: Option<String>,
        command: String,
    ) -> Self {
        Self {
            name,
            version,
            author,
            description,
            command,
        }
    }

    /// Returns the extension name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the extension version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Widget for rendering the extension approval prompt.
pub struct ExtensionApprovalWidget<'a> {
    prompt: &'a ExtensionApprovalPrompt,
}

impl<'a> ExtensionApprovalWidget<'a> {
    /// Creates a new extension approval widget.
    #[must_use]
    pub fn new(prompt: &'a ExtensionApprovalPrompt) -> Self {
        Self { prompt }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 60_u16.min(area.width.saturating_sub(4));
        let height = 12_u16.min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ExtensionApprovalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);
        let bg_color = Color::Rgb(30, 30, 30);
        let warning_color = Color::Yellow;

        // Clear background
        Clear.render(popup_area, buf);
        for y in popup_area.y..popup_area.bottom() {
            for x in popup_area.x..popup_area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }

        // Draw border with warning color
        let title = " Extension Approval Required ";
        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(warning_color).bg(bg_color));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build content layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // Name + version
                Constraint::Length(1), // Author
                Constraint::Length(1), // Description
                Constraint::Length(1), // Blank
                Constraint::Length(1), // Command header
                Constraint::Length(1), // Command
                Constraint::Length(1), // Blank
                Constraint::Length(1), // Warning
                Constraint::Length(1), // Instructions
            ])
            .split(inner);

        // Extension name and version
        let name_line = format!("{} v{}", self.prompt.name, self.prompt.version);
        Paragraph::new(name_line)
            .style(
                Style::default()
                    .fg(Color::White)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            )
            .render(chunks[0], buf);

        // Author
        let author_text = match &self.prompt.author {
            Some(author) => format!("by {}", author),
            None => "Author: unknown".to_string(),
        };
        Paragraph::new(author_text)
            .style(Style::default().fg(Color::Gray).bg(bg_color))
            .render(chunks[1], buf);

        // Description (truncated if needed)
        let desc_text = match &self.prompt.description {
            Some(desc) => {
                let max_len = chunks[2].width as usize;
                if desc.len() > max_len {
                    format!("{}...", &desc[..max_len.saturating_sub(3)])
                } else {
                    desc.clone()
                }
            }
            None => String::new(),
        };
        Paragraph::new(desc_text)
            .style(Style::default().fg(Color::DarkGray).bg(bg_color))
            .render(chunks[2], buf);

        // Command header
        Paragraph::new("This extension will run:")
            .style(Style::default().fg(Color::White).bg(bg_color))
            .render(chunks[4], buf);

        // Command (truncated if needed)
        let cmd_text = {
            let max_len = chunks[5].width as usize;
            if self.prompt.command.len() > max_len {
                format!("{}...", &self.prompt.command[..max_len.saturating_sub(3)])
            } else {
                self.prompt.command.clone()
            }
        };
        Paragraph::new(cmd_text)
            .style(Style::default().fg(Color::Cyan).bg(bg_color))
            .render(chunks[5], buf);

        // Warning
        Paragraph::new("Warning: Extension has full API access")
            .style(Style::default().fg(warning_color).bg(bg_color))
            .render(chunks[7], buf);

        // Instructions
        let instructions = Line::from(vec![
            Span::styled("Y", Style::default().fg(Color::Green).bg(bg_color)),
            Span::styled(" Approve  ", Style::default().fg(Color::White).bg(bg_color)),
            Span::styled("N", Style::default().fg(Color::Red).bg(bg_color)),
            Span::styled(" Deny  ", Style::default().fg(Color::White).bg(bg_color)),
            Span::styled("Esc", Style::default().fg(Color::Gray).bg(bg_color)),
            Span::styled(" Cancel", Style::default().fg(Color::White).bg(bg_color)),
        ]);
        Paragraph::new(instructions)
            .alignment(Alignment::Center)
            .render(chunks[8], buf);
    }
}
