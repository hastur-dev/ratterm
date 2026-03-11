//! Docker log viewer widget rendering.
//!
//! Renders the log viewer based on the current `LogViewMode`.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::state::{DockerLogsState, LogViewMode};

/// Renders the Docker log viewer into the given area.
pub fn render_docker_logs(state: &DockerLogsState, area: Rect, buf: &mut Buffer) {
    match state.mode() {
        LogViewMode::ContainerList => render_container_list(state, area, buf),
        LogViewMode::Streaming | LogViewMode::Paused => render_log_stream(state, area, buf),
        LogViewMode::Searching => render_log_stream(state, area, buf),
        LogViewMode::SavedSearches => render_saved_searches(state, area, buf),
    }
}

/// Renders the container selection list.
fn render_container_list(state: &DockerLogsState, area: Rect, buf: &mut Buffer) {
    if area.height < 3 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1), // Header
        Constraint::Min(3),    // List
        Constraint::Length(1), // Footer
    ])
    .split(area);

    // Header
    let header = Line::from(Span::styled(
        "Select a container to view logs:",
        Style::default().fg(Color::Cyan),
    ));
    Paragraph::new(header).render(chunks[0], buf);

    // Container list
    let containers = state.containers();
    let selected = state.selected_idx();

    for (i, container) in containers.iter().enumerate() {
        let row = i as u16;
        if row >= chunks[1].height {
            break;
        }

        let is_selected = i == selected;
        let prefix = if is_selected { ">" } else { " " };

        let status_color = if container.status.contains("running") {
            Color::Green
        } else {
            Color::Yellow
        };

        let style = if is_selected {
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let line = Line::from(vec![
            Span::styled(format!("{} ", prefix), style),
            Span::styled(
                format!("{:<20}", truncate(&container.name, 20)),
                style,
            ),
            Span::styled(
                format!(" {:<15}", truncate(&container.image, 15)),
                if is_selected {
                    style
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::styled(
                format!(" {}", container.status),
                if is_selected {
                    style
                } else {
                    Style::default().fg(status_color)
                },
            ),
        ]);

        let y = chunks[1].y + row;
        let line_width = line.width() as u16;
        let para = Paragraph::new(line);
        let line_area = Rect::new(chunks[1].x, y, chunks[1].width, 1);
        para.render(line_area, buf);

        // Fill background for selected row
        if is_selected {
            for x in (chunks[1].x + line_width)..chunks[1].right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ').set_style(style);
                }
            }
        }
    }

    // Footer
    let footer = Line::from(vec![
        Span::styled("[Enter] ", Style::default().fg(Color::Green)),
        Span::raw("Stream  "),
        Span::styled("[Esc] ", Style::default().fg(Color::Red)),
        Span::raw("Back  "),
        Span::styled("[?] ", Style::default().fg(Color::Cyan)),
        Span::raw("Help"),
    ]);
    Paragraph::new(footer).render(chunks[2], buf);
}

/// Renders the log stream view (streaming, paused, or searching).
fn render_log_stream(state: &DockerLogsState, area: Rect, buf: &mut Buffer) {
    if area.height < 3 {
        return;
    }

    let search_height = if state.mode() == LogViewMode::Searching {
        1
    } else {
        0
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),              // Header/status
        Constraint::Min(3),                 // Log lines
        Constraint::Length(search_height),  // Search bar (if searching)
        Constraint::Length(1),              // Footer
    ])
    .split(area);

    // Header with status
    let container_name = state
        .active_container_name()
        .unwrap_or("unknown");

    let mode_label = match state.mode() {
        LogViewMode::Streaming => Span::styled(" LIVE ", Style::default().fg(Color::Black).bg(Color::Green)),
        LogViewMode::Paused => Span::styled(" PAUSED ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        LogViewMode::Searching => Span::styled(" SEARCH ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        _ => Span::raw(""),
    };

    let buffer_info = format!(
        " {} lines ({})",
        state.log_buffer().filtered_count(),
        if state.log_buffer().filter().is_empty() {
            "no filter".to_string()
        } else {
            format!("filter: {}", state.log_buffer().filter())
        },
    );

    let header = Line::from(vec![
        mode_label,
        Span::raw(" "),
        Span::styled(container_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(buffer_info, Style::default().fg(Color::Gray)),
    ]);
    Paragraph::new(header).render(chunks[0], buf);

    // Log lines
    let visible_rows = chunks[1].height as usize;
    let entries = state.log_buffer().visible_entries(visible_rows);

    for (i, entry) in entries.iter().enumerate() {
        let row = i as u16;
        if row >= chunks[1].height {
            break;
        }

        let y = chunks[1].y + row;
        let level_color = if state.config().color_coding {
            entry.level.color()
        } else {
            Color::White
        };

        let mut spans = Vec::new();

        // Timestamp (if enabled)
        if state.config().show_timestamps {
            let ts = truncate(&entry.timestamp, 19);
            spans.push(Span::styled(
                format!("{} ", ts),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Log level label
        spans.push(Span::styled(
            format!("{} ", entry.level.label()),
            Style::default().fg(level_color).add_modifier(Modifier::BOLD),
        ));

        // Message
        let remaining_width = area.width.saturating_sub(
            spans.iter().map(|s| s.width() as u16).sum::<u16>(),
        ) as usize;
        let msg = truncate(&entry.message, remaining_width);
        spans.push(Span::styled(msg, Style::default().fg(level_color)));

        let line = Line::from(spans);
        let line_area = Rect::new(chunks[1].x, y, chunks[1].width, 1);
        Paragraph::new(line).render(line_area, buf);
    }

    // Search bar (if searching)
    if state.mode() == LogViewMode::Searching && search_height > 0 {
        let search_line = Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                state.search_input(),
                Style::default().fg(Color::White).add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled("_", Style::default().fg(Color::White)),
        ]);
        Paragraph::new(search_line).render(chunks[2], buf);
    }

    // Footer
    let footer = Line::from(vec![
        Span::styled("[Space] ", Style::default().fg(Color::Green)),
        Span::raw("Pause  "),
        Span::styled("[/] ", Style::default().fg(Color::Cyan)),
        Span::raw("Search  "),
        Span::styled("[Esc] ", Style::default().fg(Color::Red)),
        Span::raw("Back  "),
        Span::styled("[?] ", Style::default().fg(Color::Gray)),
        Span::raw("Help"),
    ]);
    Paragraph::new(footer).render(chunks[3], buf);
}

/// Renders saved searches list.
fn render_saved_searches(state: &DockerLogsState, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Header
        Constraint::Min(3),    // List
        Constraint::Length(1), // Footer
    ])
    .split(area);

    let header = Line::from(Span::styled(
        "Saved Searches:",
        Style::default().fg(Color::Cyan),
    ));
    Paragraph::new(header).render(chunks[0], buf);

    let searches = state.search_manager().list();
    let selected = state.saved_search_idx();

    if searches.is_empty() {
        let msg = Paragraph::new("No saved searches. Use Ctrl+S in search mode to save.")
            .style(Style::default().fg(Color::Gray));
        msg.render(chunks[1], buf);
    } else {
        for (i, search) in searches.iter().enumerate() {
            let row = i as u16;
            if row >= chunks[1].height {
                break;
            }

            let is_selected = i == selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { ">" } else { " " };
            let line = Line::from(vec![
                Span::styled(format!("{} ", prefix), style),
                Span::styled(format!("{}: ", search.name), style),
                Span::styled(
                    &search.pattern,
                    if is_selected {
                        style
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
            ]);

            let y = chunks[1].y + row;
            let line_area = Rect::new(chunks[1].x, y, chunks[1].width, 1);
            Paragraph::new(line).render(line_area, buf);
        }
    }

    let footer = Line::from(vec![
        Span::styled("[Enter] ", Style::default().fg(Color::Green)),
        Span::raw("Apply  "),
        Span::styled("[d] ", Style::default().fg(Color::Red)),
        Span::raw("Delete  "),
        Span::styled("[Esc] ", Style::default().fg(Color::Gray)),
        Span::raw("Back"),
    ]);
    Paragraph::new(footer).render(chunks[2], buf);
}

/// Truncates a string to the given maximum width.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_render_no_panic_empty() {
        let state = DockerLogsState::new(
            crate::docker_logs::config::LogStreamConfig::default(),
        );
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_docker_logs(&state, area, &mut buf);
        // Should not panic
    }

    #[test]
    fn test_render_no_panic_small_area() {
        let state = DockerLogsState::new(
            crate::docker_logs::config::LogStreamConfig::default(),
        );
        let area = Rect::new(0, 0, 10, 2);
        let mut buf = Buffer::empty(area);
        render_docker_logs(&state, area, &mut buf);
        // Should not panic even with tiny area
    }
}
