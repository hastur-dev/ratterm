//! Docker Manager widget.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Widget},
};

use crate::ui::key_hint_bar::{KeyHint, KeyHintStyle};
use crate::ui::manager_footer::ManagerFooter;

use super::selector::DockerManagerSelector;
use super::types::DockerManagerMode;

/// Docker Manager widget.
pub struct DockerManagerWidget<'a> {
    selector: &'a DockerManagerSelector,
    position: Option<crate::ui::window_position::WindowPosition>,
}

impl<'a> DockerManagerWidget<'a> {
    /// Creates a new Docker manager widget.
    #[must_use]
    pub fn new(selector: &'a DockerManagerSelector) -> Self {
        Self {
            selector,
            position: None,
        }
    }

    /// Sets the window position from config.
    #[must_use]
    pub fn position(mut self, pos: crate::ui::window_position::WindowPosition) -> Self {
        self.position = Some(pos);
        self
    }
}

impl Widget for DockerManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Minimum dimensions check
        if area.width < 40 || area.height < 10 {
            return;
        }

        // Calculate popup dimensions (60% width, 70% height)
        let popup_width = area.width.saturating_mul(60).saturating_div(100).max(50);
        let popup_height = area.height.saturating_mul(70).saturating_div(100).max(15);

        // Use configured position or default center
        let popup_area = match &self.position {
            Some(pos) => pos.resolve(popup_width, popup_height, area.width, area.height),
            None => {
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                Rect::new(popup_x, popup_y, popup_width, popup_height)
            }
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Render based on mode
        match self.selector.mode() {
            DockerManagerMode::List | DockerManagerMode::Discovering => {
                self.render_list_mode(popup_area, buf);
            }
            DockerManagerMode::RunOptions => {
                self.render_run_options_mode(popup_area, buf);
            }
            DockerManagerMode::Confirming => {
                self.render_confirm_mode(popup_area, buf);
            }
            DockerManagerMode::Connecting => {
                self.render_connecting_mode(popup_area, buf);
            }
            DockerManagerMode::HostSelection => {
                self.render_host_selection_mode(popup_area, buf);
            }
            DockerManagerMode::HostCredentials => {
                self.render_host_credentials_mode(popup_area, buf);
            }
            // Container creation workflow modes
            DockerManagerMode::SearchingHub
            | DockerManagerMode::SearchResults
            | DockerManagerMode::CheckingImage
            | DockerManagerMode::DownloadingImage
            | DockerManagerMode::VolumeMountHostPath
            | DockerManagerMode::VolumeMountContainerPath
            | DockerManagerMode::VolumeMountConfirm
            | DockerManagerMode::StartupCommand
            | DockerManagerMode::CreateConfirm
            | DockerManagerMode::CreationError => {
                super::widget_create::render_creation_mode(self.selector, popup_area, buf);
            }
        }
    }
}

impl DockerManagerWidget<'_> {
    /// Renders the main list mode.
    fn render_list_mode(&self, area: Rect, buf: &mut Buffer) {
        // Build title with section tabs
        let section = self.selector.section();
        let title = format!(
            " Docker Manager - {} ({}/{}) ",
            section.title(),
            self.selector.current_section_count(),
            self.selector.total_count()
        );

        let title_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(Span::styled(title, title_style))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: tabs, list, footer (2 rows for ManagerFooter)
        let chunks = Layout::vertical([
            Constraint::Length(2), // Section tabs
            Constraint::Min(5),    // List
            Constraint::Length(2), // ManagerFooter (2 rows)
        ])
        .split(inner);

        // Render section tabs
        self.render_section_tabs(chunks[0], buf);

        // Render list or status
        if !self.selector.docker_available() {
            self.render_docker_unavailable(chunks[1], buf);
        } else if self.selector.mode() == DockerManagerMode::Discovering {
            self.render_discovering(chunks[1], buf);
        } else if self.selector.is_section_empty() {
            self.render_empty_section(chunks[1], buf);
        } else {
            self.render_item_list(chunks[1], buf);
        }

        // Render ManagerFooter with essential hints + [?] for full list
        let primary = vec![
            KeyHint::styled("Enter", "Action", KeyHintStyle::Success),
            KeyHint::new("Tab", "Section"),
            KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
        ];
        let secondary = vec![KeyHint::new("?", "All shortcuts")];
        let footer = ManagerFooter::new(primary).secondary(secondary);
        footer.render(chunks[2], buf);
    }

    /// Renders the section tabs.
    fn render_section_tabs(&self, area: Rect, buf: &mut Buffer) {
        use super::types::DockerListSection;

        let current = self.selector.section();
        let sections = [
            (
                DockerListSection::RunningContainers,
                self.selector.running_containers.len(),
            ),
            (
                DockerListSection::StoppedContainers,
                self.selector.stopped_containers.len(),
            ),
            (DockerListSection::Images, self.selector.images.len()),
        ];

        let mut spans = Vec::new();
        for (i, (section, count)) in sections.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" | "));
            }

            let label = format!("{} ({})", section.title(), count);
            let style = if *section == current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Gray)
            };

            spans.push(Span::styled(label, style));
        }

        let line = Line::from(spans);
        let para = Paragraph::new(line);
        para.render(area, buf);
    }

    /// Renders the item list.
    fn render_item_list(&self, area: Rect, buf: &mut Buffer) {
        let items = self.selector.visible_items();
        let selected_idx = self.selector.selected_index();

        for (row, (idx, item)) in items.iter().enumerate() {
            if row >= area.height as usize {
                break;
            }

            let y = area.y + row as u16;
            let is_selected = *idx == selected_idx;

            // Build line
            let prefix = if is_selected { ">" } else { " " };
            let type_label = item.item_type().label();
            let summary = item.summary();

            // Truncate if needed
            let max_len = (area.width as usize).saturating_sub(10);
            let text = format!("{} {} {}", prefix, type_label, summary);
            let truncated = if text.len() > max_len {
                format!("{}...", &text[..max_len.saturating_sub(3)])
            } else {
                text
            };

            let style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Render line
            let x = area.x;
            let width = area.width as usize;
            for (i, c) in truncated.chars().enumerate() {
                if i >= width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
                    cell.set_char(c).set_style(style);
                }
            }

            // Fill rest with background if selected
            if is_selected {
                for i in truncated.len()..width {
                    if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
                        cell.set_char(' ').set_style(style);
                    }
                }
            }
        }

        // Render scrollbar if needed
        let total = self.selector.current_section_count();
        if total > area.height as usize {
            self.render_scrollbar(area, buf, total);
        }
    }

    /// Renders a simple scrollbar.
    fn render_scrollbar(&self, area: Rect, buf: &mut Buffer, total: usize) {
        if area.height < 3 || total == 0 {
            return;
        }

        let x = area.x + area.width - 1;
        let scroll = self.selector.scroll_offset();
        let visible = area.height as usize;

        // Calculate thumb position and size
        let thumb_size = ((visible as f64 / total as f64) * area.height as f64).max(1.0) as u16;
        let thumb_pos = ((scroll as f64 / total as f64) * area.height as f64) as u16;

        for row in 0..area.height {
            let y = area.y + row;
            let c = if row >= thumb_pos && row < thumb_pos + thumb_size {
                '█'
            } else {
                '░'
            };
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(c)
                    .set_style(Style::default().fg(Color::DarkGray));
            }
        }
    }

    /// Renders "Docker unavailable" message based on availability status.
    fn render_docker_unavailable(&self, area: Rect, buf: &mut Buffer) {
        use crate::docker::DockerAvailability;

        let availability = self.selector.availability();

        let lines = match &availability {
            DockerAvailability::NotInstalled => vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Docker is not installed locally.",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("You can manage Docker on a remote host via SSH,"),
                Line::from("or install Docker locally to manage containers here."),
                Line::from(""),
                Line::from(Span::styled(
                    "Visit https://www.docker.com/get-started to install Docker.",
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[h] ", Style::default().fg(Color::Green)),
                    Span::styled("Select Remote Host", Style::default().fg(Color::White)),
                    Span::styled("    ", Style::default()),
                    Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                    Span::styled("Close", Style::default().fg(Color::White)),
                ]),
            ],
            DockerAvailability::NotRunning => vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Docker is not currently running.",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Start Docker locally, or manage Docker on a remote host."),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                    Span::styled("Start Docker", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[h] ", Style::default().fg(Color::Magenta)),
                    Span::styled("Remote Host", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[r] ", Style::default().fg(Color::Cyan)),
                    Span::styled("Retry", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                    Span::styled("Close", Style::default().fg(Color::White)),
                ]),
            ],
            DockerAvailability::DaemonError(msg) => {
                // Show the actual error message from Docker
                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Docker daemon error:",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                ];

                // Wrap the error message if too long
                let max_width = area.width.saturating_sub(4) as usize;
                for chunk in msg.as_bytes().chunks(max_width.max(40)) {
                    if let Ok(s) = std::str::from_utf8(chunk) {
                        lines.push(Line::from(Span::styled(
                            s.to_string(),
                            Style::default().fg(Color::Yellow),
                        )));
                    }
                }

                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Restart Docker locally, or manage Docker on a remote host.",
                ));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                    Span::styled("Restart", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[h] ", Style::default().fg(Color::Magenta)),
                    Span::styled("Remote Host", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[r] ", Style::default().fg(Color::Cyan)),
                    Span::styled("Retry", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                    Span::styled("Close", Style::default().fg(Color::White)),
                ]));

                lines
            }
            _ => vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Docker is not available locally.",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Manage Docker on a remote host, or install locally."),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[h] ", Style::default().fg(Color::Green)),
                    Span::styled("Remote Host", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[r] ", Style::default().fg(Color::Cyan)),
                    Span::styled("Retry", Style::default().fg(Color::White)),
                    Span::styled("  ", Style::default()),
                    Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                    Span::styled("Close", Style::default().fg(Color::White)),
                ]),
            ],
        };

        let para = Paragraph::new(lines);
        para.render(area, buf);
    }

    /// Renders "Discovering" message.
    fn render_discovering(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from("Discovering Docker containers and images..."),
            Line::from(""),
            Line::from("Please wait..."),
        ];

        let style = Style::default().fg(Color::Yellow);
        let para = Paragraph::new(lines).style(style);
        para.render(area, buf);
    }

    /// Renders "Empty section" message.
    fn render_empty_section(&self, area: Rect, buf: &mut Buffer) {
        let message = match self.selector.section() {
            super::types::DockerListSection::RunningContainers => "No running containers found.",
            super::types::DockerListSection::StoppedContainers => "No stopped containers found.",
            super::types::DockerListSection::Images => "No images found.",
        };

        let lines = vec![
            Line::from(message),
            Line::from(""),
            Line::from("Press Tab to switch sections or r to refresh."),
        ];

        let style = Style::default().fg(Color::Gray);
        let para = Paragraph::new(lines).style(style);
        para.render(area, buf);
    }

    /// Renders run options mode.
    fn render_run_options_mode(&self, area: Rect, buf: &mut Buffer) {
        let target = self.selector.run_target().unwrap_or("image");
        let title = format!(" Run Options - {} ", target);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        super::widget_forms::render_run_options_form(self.selector, inner, buf);
    }

    /// Renders confirm mode.
    fn render_confirm_mode(&self, area: Rect, buf: &mut Buffer) {
        let target = self.selector.confirm_target().unwrap_or("image");
        let title = " Confirm ";

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = vec![
            Line::from(""),
            Line::from(format!("Run image: {}", target)),
            Line::from(""),
            Line::from("The image is not running as a container."),
            Line::from("Do you want to start a new container from this image?"),
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Yes, run it    [Ctrl+O] Run with options    [Esc] Cancel",
                Style::default().fg(Color::Cyan),
            )),
        ];

        let para = Paragraph::new(lines);
        para.render(inner, buf);
    }

    /// Renders connecting mode.
    fn render_connecting_mode(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Connecting ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines = vec![
            Line::from(""),
            Line::from("Connecting to container..."),
            Line::from(""),
            Line::from("Please wait..."),
        ];

        let style = Style::default().fg(Color::Green);
        let para = Paragraph::new(lines).style(style);
        para.render(inner, buf);
    }

    /// Renders host selection mode.
    fn render_host_selection_mode(&self, area: Rect, buf: &mut Buffer) {
        let current_host = self.selector.selected_host().display_name();
        let title = format!(" Select Docker Host (current: {}) ", current_host);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render available hosts
        super::widget_forms::render_host_selection_list(self.selector, inner, buf);
    }

    /// Renders host credentials entry mode.
    fn render_host_credentials_mode(&self, area: Rect, buf: &mut Buffer) {
        let host_name = self.selector.cred_host_name().unwrap_or("Host");
        let title = format!(" Enter Credentials - {} ", host_name);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        super::widget_forms::render_host_credentials_form(self.selector, inner, buf);
    }
}
