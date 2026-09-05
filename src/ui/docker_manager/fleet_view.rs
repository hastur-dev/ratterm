//! The fleet view: every container on every host, on one screen.
//!
//! One list, with the host name on each row, so "what is running on my fleet"
//! is a single screen rather than one screen per machine. Hosts that failed to
//! connect appear in the header with the reason, and the recent container
//! events sit under the list so a container that died overnight is visible
//! without opening anything.
//!
//! The navigation and layout arithmetic are free functions with no terminal
//! involved, so they are tested directly.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::docker::events::DockerEvents;
use crate::docker::fleet::DockerFleet;

use super::fleet_nav::{FleetViewState, fit, visible_window};

pub use super::fleet_nav::EVENT_PANE_ROWS;

/// Rows the events pane needs before it is worth drawing.
const MIN_EVENT_PANE_HEIGHT: u16 = 3;

/// The filter part of the header.
///
/// While the filter line is open it shows the buffer being typed with a
/// cursor, so an empty filter still looks like something is happening. Once
/// it is closed only an actual filter is shown.
#[must_use]
pub fn filter_hint(state: &FleetViewState, applied: &str) -> String {
    if state.editing_filter() {
        format!("  filter: {}_", state.filter())
    } else if applied.is_empty() {
        String::new()
    } else {
        format!("  filter: {applied}")
    }
}

/// The header lines: the fleet counts, then one line per host.
#[must_use]
pub fn header_lines(fleet: &DockerFleet, state: &FleetViewState) -> Vec<String> {
    let mut lines = vec![format!(
        "{}  sort: {}{}",
        fleet.counts().headline(),
        fleet.sort().as_str(),
        filter_hint(state, fleet.filter())
    )];

    for host in fleet.hosts() {
        let mut line = host.header();
        if let Some(detail) = host.connection.detail() {
            line.push_str("  ");
            line.push_str(&detail);
        }
        lines.push(line);
    }

    lines
}

/// The most recent events, newest last, as display lines.
#[must_use]
pub fn event_lines(events: &DockerEvents, limit: usize) -> Vec<String> {
    let recent = events.recent(limit);
    if recent.is_empty() {
        let note = if events.is_durable() {
            "no container events yet"
        } else {
            "no container events yet (history is not being saved)"
        };
        return vec![note.to_string()];
    }
    recent.iter().map(|event| event.summary()).collect()
}

/// The fleet list, ready to render.
pub struct FleetViewWidget<'a> {
    fleet: &'a DockerFleet,
    events: &'a DockerEvents,
    state: &'a FleetViewState,
}

impl<'a> FleetViewWidget<'a> {
    /// Wraps the fleet and its events for one frame.
    #[must_use]
    pub const fn new(
        fleet: &'a DockerFleet,
        events: &'a DockerEvents,
        state: &'a FleetViewState,
    ) -> Self {
        Self {
            fleet,
            events,
            state,
        }
    }

    /// Splits the area into header, list and events pane.
    fn split(&self, inner: Rect, header_height: u16) -> (Rect, Rect, Option<Rect>) {
        let header = Rect {
            height: header_height.min(inner.height),
            ..inner
        };
        let rest_y = inner.y + header.height;
        let rest_height = inner.height.saturating_sub(header.height);

        let events_height = if self.state.events_shown() && rest_height > MIN_EVENT_PANE_HEIGHT * 2
        {
            (EVENT_PANE_ROWS as u16 + 1).min(rest_height / 2)
        } else {
            0
        };

        let list = Rect {
            y: rest_y,
            height: rest_height.saturating_sub(events_height),
            ..inner
        };
        let events = (events_height > 0).then(|| Rect {
            y: rest_y + list.height,
            height: events_height,
            ..inner
        });

        (header, list, events)
    }
}

impl Widget for FleetViewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Docker fleet ");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 8 || inner.height < 3 {
            return;
        }

        let headers = header_lines(self.fleet, self.state);
        let header_height = (headers.len() as u16)
            .min(inner.height.saturating_sub(2))
            .max(1);
        let (header_area, list_area, events_area) = self.split(inner, header_height);

        let width = inner.width as usize;
        let header_text: Vec<Line> = headers
            .iter()
            .take(header_area.height as usize)
            .enumerate()
            .map(|(index, line)| {
                let style = if index == 0 {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if line.contains("[failed]") {
                    Style::default().fg(Color::Red)
                } else if line.contains("[connected]") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::from(Span::styled(fit(line, width), style))
            })
            .collect();
        Paragraph::new(header_text).render(header_area, buf);

        let rows = self.fleet.rows();
        if rows.is_empty() {
            let message = if self.fleet.is_empty() {
                "No Docker hosts are being watched. Add one in the SSH manager."
            } else if self.fleet.counts().connected == 0 {
                "No host is connected. Press r to retry."
            } else {
                "No container matches the filter."
            };
            Paragraph::new(fit(message, width))
                .style(Style::default().fg(Color::Gray))
                .render(list_area, buf);
        } else {
            let (start, end) =
                visible_window(rows.len(), self.state.selected(), list_area.height as usize);
            let lines: Vec<Line> = rows[start..end]
                .iter()
                .enumerate()
                .map(|(offset, row)| {
                    let index = start + offset;
                    let selected = index == self.state.selected();
                    let style = if selected {
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD)
                    } else if row.container.is_running() {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    Line::from(Span::styled(fit(&row.line(), width), style))
                })
                .collect();
            Paragraph::new(lines).render(list_area, buf);
        }

        if let Some(events_area) = events_area {
            let rows = (events_area.height as usize).saturating_sub(1).max(1);
            let mut lines = vec![Line::from(Span::styled(
                fit("Recent container events", width),
                Style::default().fg(Color::Cyan),
            ))];
            lines.extend(event_lines(self.events, rows).iter().map(|line| {
                Line::from(Span::styled(
                    fit(line, width),
                    Style::default().fg(Color::DarkGray),
                ))
            }));
            Paragraph::new(lines).render(events_area, buf);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::docker::events::FleetEvent;
    use crate::docker::host::DockerHost;

    fn fleet_with(hosts: &[(&DockerHost, &str)]) -> DockerFleet {
        let mut fleet = DockerFleet::new();
        for (host, label) in hosts {
            fleet.track(host, label);
        }
        fleet
    }

    fn event(action: &str) -> FleetEvent {
        FleetEvent {
            host_key: Some(1),
            host_label: "rock5c".to_string(),
            container_id: "abc".to_string(),
            container_name: Some("web".to_string()),
            ts: 1_700_000_000,
            action: action.to_string(),
            detail: None,
        }
    }

    #[test]
    fn the_filter_hint_shows_a_cursor_while_the_line_is_open() {
        let mut state = FleetViewState::new();
        assert_eq!(filter_hint(&state, ""), "", "nothing to say when idle");
        assert_eq!(filter_hint(&state, "web"), "  filter: web");

        // Opening the filter line is what `/` does.
        super::super::fleet_nav::handle_key(
            &mut state,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('/'),
                crossterm::event::KeyModifiers::NONE,
            ),
            0,
        );
        assert_eq!(
            filter_hint(&state, "web"),
            "  filter: _",
            "the buffer being typed wins over the applied filter"
        );
    }

    #[test]
    fn an_empty_fleet_still_has_a_header() {
        let lines = header_lines(&DockerFleet::new(), &FleetViewState::new());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("0 hosts"), "{}", lines[0]);
        assert!(lines[0].contains("sort: host"), "{}", lines[0]);
    }

    #[test]
    fn the_header_lists_one_line_per_host_and_shows_the_filter() {
        let mut fleet = fleet_with(&[
            (&DockerHost::Local, "Local"),
            (&DockerHost::remote(1), "rock5c"),
        ]);
        fleet.set_filter("web");

        let lines = header_lines(&fleet, &FleetViewState::new());
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("filter: web"), "{}", lines[0]);
        assert!(lines[1].starts_with("Local"), "{}", lines[1]);
        assert!(lines[2].starts_with("rock5c"), "{}", lines[2]);
    }

    #[test]
    fn a_failed_host_shows_its_reason_in_the_header() {
        let mut fleet = fleet_with(&[(&DockerHost::remote(1), "rock5c")]);
        fleet.mark_failed(
            Some(1),
            "no route to host; check the SSH manager".to_string(),
            1,
        );
        let lines = header_lines(&fleet, &FleetViewState::new());
        assert!(lines[1].contains("[failed]"), "{}", lines[1]);
        assert!(lines[1].contains("no route to host"), "{}", lines[1]);
    }

    #[test]
    fn an_empty_event_log_says_so_and_says_whether_it_is_being_saved() {
        let events = DockerEvents::in_memory_only();
        let lines = event_lines(&events, 5);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("not being saved"), "{}", lines[0]);
    }

    #[test]
    fn events_are_listed_newest_last_and_capped_at_the_limit() {
        let mut events = DockerEvents::in_memory_only();
        for action in ["create", "start", "die"] {
            events.ingest(event(action));
        }
        let lines = event_lines(&events, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("die"), "{}", lines[1]);
        assert!(lines[0].contains("start"), "{}", lines[0]);
        assert!(lines[1].contains("rock5c"), "{}", lines[1]);
    }

    #[test]
    fn the_widget_draws_without_panicking_at_awkward_sizes() {
        let mut fleet = fleet_with(&[(&DockerHost::Local, "Local")]);
        fleet.mark_failed(None, "no local endpoint; start Docker".to_string(), 1);
        let mut events = DockerEvents::in_memory_only();
        events.ingest(event("die"));
        let state = FleetViewState::new();

        for (width, height) in [(1, 1), (8, 3), (40, 6), (120, 30)] {
            let area = Rect::new(0, 0, width, height);
            let mut buffer = Buffer::empty(area);
            FleetViewWidget::new(&fleet, &events, &state).render(area, &mut buffer);
        }
    }

    #[test]
    fn the_widget_says_why_the_list_is_empty() {
        let empty = DockerFleet::new();
        let events = DockerEvents::in_memory_only();
        let state = FleetViewState::new();
        let area = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(area);
        FleetViewWidget::new(&empty, &events, &state).render(area, &mut buffer);

        let rendered: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("No Docker hosts"), "{rendered}");
    }

    #[test]
    fn a_tracked_but_unconnected_fleet_offers_a_retry() {
        let fleet = fleet_with(&[(&DockerHost::remote(1), "rock5c")]);
        let events = DockerEvents::in_memory_only();
        let state = FleetViewState::new();
        let area = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(area);
        FleetViewWidget::new(&fleet, &events, &state).render(area, &mut buffer);

        let rendered: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("No host is connected"), "{rendered}");
    }

    #[test]
    fn hiding_the_events_pane_leaves_the_list_taller() {
        let fleet = fleet_with(&[(&DockerHost::Local, "Local")]);
        let events = DockerEvents::in_memory_only();
        let inner = Rect::new(0, 0, 80, 20);

        let shown = FleetViewState::new();
        let mut hidden = FleetViewState::new();
        hidden.toggle_events();

        let with_pane = FleetViewWidget::new(&fleet, &events, &shown).split(inner, 2);
        let without = FleetViewWidget::new(&fleet, &events, &hidden).split(inner, 2);

        assert!(with_pane.2.is_some());
        assert!(without.2.is_none());
        assert!(without.1.height > with_pane.1.height);
    }

    #[test]
    fn a_short_area_drops_the_events_pane_rather_than_squashing_the_list() {
        let fleet = fleet_with(&[(&DockerHost::Local, "Local")]);
        let events = DockerEvents::in_memory_only();
        let state = FleetViewState::new();
        let inner = Rect::new(0, 0, 80, 6);
        let (_, list, pane) = FleetViewWidget::new(&fleet, &events, &state).split(inner, 2);
        assert!(pane.is_none());
        assert_eq!(list.height, 4);
    }
}
