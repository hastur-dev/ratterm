//! The Docker manager's non-list modes: the "Docker is not available"
//! explainer, run options, confirmation, connecting, host selection, host
//! credentials and the log viewer.
//!
//! Split out of `widget.rs`, which had grown past this project's file-size
//! limit.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};

use super::widget::DockerManagerWidget;

impl DockerManagerWidget<'_> {
    /// Renders an explanation of why Docker cannot be used.
    pub(super) fn render_docker_unavailable(&self, area: Rect, buf: &mut Buffer) {
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

    /// Renders run options mode.
    pub(super) fn render_run_options_mode(&self, area: Rect, buf: &mut Buffer) {
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
    pub(super) fn render_confirm_mode(&self, area: Rect, buf: &mut Buffer) {
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
    pub(super) fn render_connecting_mode(&self, area: Rect, buf: &mut Buffer) {
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
    pub(super) fn render_host_selection_mode(&self, area: Rect, buf: &mut Buffer) {
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
    pub(super) fn render_host_credentials_mode(&self, area: Rect, buf: &mut Buffer) {
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

    /// Renders the Docker log viewer mode (placeholder until Phase 5).
    pub(super) fn render_log_view(&self, area: Rect, buf: &mut Buffer) {
        let title = " Docker Logs ";

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        if let Some(ref state) = self.selector.docker_logs_state {
            crate::docker_logs::ui::widget::render_docker_logs(state, inner, buf);
        } else {
            let para = Paragraph::new("No log state available");
            para.render(inner, buf);
        }
    }
}
