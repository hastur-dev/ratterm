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
    pub(super) selector: &'a DockerManagerSelector,
    pub(super) position: Option<crate::ui::window_position::WindowPosition>,
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
            // Docker log viewer mode
            DockerManagerMode::LogView => {
                self.render_log_view(popup_area, buf);
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
            KeyHint::new("l", "Logs"),
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
}
