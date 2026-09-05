//! Drawing the Kubernetes screens.
//!
//! Presentation only: every decision about what a row says or whether it is
//! healthy is made in [`super::rows`], and every decision about what is
//! selected is made in [`super::K8sManager`]. This turns those into cells.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use super::{K8sManager, K8sView, ResourceKind};

/// Rows reserved for the header, the tab bar and the footer.
const CHROME_ROWS: u16 = 6;

/// Minimum width of the name column.
const MIN_NAME_WIDTH: u16 = 20;

/// Draws a [`K8sManager`].
pub struct K8sManagerWidget<'a> {
    manager: &'a K8sManager,
    focused: bool,
}

impl<'a> K8sManagerWidget<'a> {
    /// Draws `manager`.
    #[must_use]
    pub const fn new(manager: &'a K8sManager) -> Self {
        Self {
            manager,
            focused: true,
        }
    }

    /// Sets whether the panel has focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for K8sManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let border = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = match self.manager.view() {
            K8sView::Contexts => "Kubernetes - Contexts".to_string(),
            K8sView::Resources => self.manager.connected().map_or_else(
                || "Kubernetes".to_string(),
                |cluster| format!("Kubernetes - {}", cluster.context),
            ),
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < CHROME_ROWS {
            // Too small to say anything useful; say that rather than drawing a
            // header over a footer.
            Paragraph::new("Not enough room")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray))
                .render(inner, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(inner);

        match self.manager.view() {
            K8sView::Contexts => {
                render_context_header(self.manager, chunks[0], buf);
                render_contexts(self.manager, chunks[1], buf);
            }
            K8sView::Resources => {
                render_tabs(self.manager, chunks[0], buf);
                render_resources(self.manager, chunks[1], buf);
            }
        }

        render_footer(self.manager, chunks[2], buf);
    }
}

/// The header of the context screen.
fn render_context_header(manager: &K8sManager, area: Rect, buf: &mut Buffer) {
    let count = manager.contexts().len();
    let text = if count == 0 {
        "No contexts".to_string()
    } else {
        format!("{count} context(s) - Enter to connect")
    };

    Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(Color::Gray),
    )))
    .render(area, buf);
}

/// The context list.
fn render_contexts(manager: &K8sManager, area: Rect, buf: &mut Buffer) {
    if let Some(error) = manager.error()
        && !manager.contexts().has_items()
    {
        Paragraph::new(vec![
            Line::from(Span::styled(error, Style::default().fg(Color::Red))),
            Line::from(""),
            Line::from(Span::styled(
                "Set KUBECONFIG, create ~/.kube/config, or read one from a fleet host.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .render(area, buf);
        return;
    }

    let height = area.height as usize;
    let selected = manager.contexts().selected();
    let offset = selected.saturating_sub(height.saturating_sub(1));

    let lines: Vec<Line> = manager
        .contexts()
        .items()
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(index, context)| {
            let marker = if context.is_current { "*" } else { " " };
            let server = context.server.as_deref().unwrap_or("<cluster not defined>");

            let style = if index == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if context.cluster_defined {
                Style::default().fg(Color::White)
            } else {
                // A broken context is listed rather than hidden: seeing it is
                // what tells the user what to fix.
                Style::default().fg(Color::Red)
            };

            Line::from(Span::styled(
                format!("{marker} {:<28} {server}", context.name),
                style,
            ))
        })
        .collect();

    Paragraph::new(lines).render(area, buf);
}

/// The resource-kind tabs and the filter box.
fn render_tabs(manager: &K8sManager, area: Rect, buf: &mut Buffer) {
    let mut spans: Vec<Span> = Vec::new();

    for kind in ResourceKind::all() {
        let style = if kind == manager.kind() {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(format!(" {} ", kind.label()), style));
        spans.push(Span::raw(" "));
    }

    let namespace = manager
        .effective_namespace()
        .map_or_else(|| "all namespaces".to_string(), str::to_string);
    spans.push(Span::styled(
        format!("[{namespace}]"),
        Style::default().fg(Color::DarkGray),
    ));

    let mut lines = vec![Line::from(spans)];

    if manager.is_filtering() || !manager.filter().is_empty() {
        let cursor = if manager.is_filtering() { "_" } else { "" };
        lines.push(Line::from(Span::styled(
            format!("/{}{cursor}", manager.filter()),
            Style::default().fg(Color::Yellow),
        )));
    }

    Paragraph::new(lines).render(area, buf);
}

/// The resource table.
fn render_resources(manager: &K8sManager, area: Rect, buf: &mut Buffer) {
    let rows = manager.rows();

    if rows.is_empty() {
        // "Nothing matched the filter" and "the cluster has none of these" are
        // different answers and the screen should not give one for the other.
        let message = if manager.filter().is_empty() {
            format!("No {} found", manager.kind().label().to_lowercase())
        } else {
            format!(
                "No {} match '{}'",
                manager.kind().label().to_lowercase(),
                manager.filter()
            )
        };
        Paragraph::new(Line::from(Span::styled(
            message,
            Style::default().fg(Color::DarkGray),
        )))
        .render(area, buf);
        return;
    }

    let headings = manager.kind().headings();
    let widths = column_widths(area.width, headings.len());

    let mut lines = vec![Line::from(Span::styled(
        format_cells(
            &headings
                .iter()
                .map(|h| (*h).to_string())
                .collect::<Vec<_>>(),
            &widths,
        ),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    ))];

    let body_height = area.height.saturating_sub(1) as usize;
    let selected = manager.selected_row_index();
    let offset = selected.saturating_sub(body_height.saturating_sub(1));

    for (index, row) in rows.iter().enumerate().skip(offset).take(body_height) {
        let style = if index == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if row.unhealthy {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format_cells(&row.cells, &widths),
            style,
        )));
    }

    Paragraph::new(lines).render(area, buf);
}

/// The key hints, and the last error if there is one.
fn render_footer(manager: &K8sManager, area: Rect, buf: &mut Buffer) {
    let hints = match manager.view() {
        K8sView::Contexts => "[Up/Down] Select  [Enter] Connect  [Esc] Close",
        K8sView::Resources => {
            "[Tab] Kind  [/] Filter  [r] Refresh  [l] Logs  [s] Scale  [Backspace] Contexts"
        }
    };

    let mut lines = vec![Line::from(Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    ))];

    if let Some(error) = manager.error() {
        lines.push(Line::from(Span::styled(
            error.to_string(),
            Style::default().fg(Color::Red),
        )));
    } else if !manager.status().is_empty() {
        lines.push(Line::from(Span::styled(
            manager.status().to_string(),
            Style::default().fg(Color::Green),
        )));
    }

    Paragraph::new(lines).render(area, buf);
}

/// Splits the available width across the columns.
///
/// The first column holds a name and gets what is left after the others take
/// a fixed share, because names are the part that gets cut in practice.
#[must_use]
pub fn column_widths(total: u16, columns: usize) -> Vec<u16> {
    if columns == 0 {
        return Vec::new();
    }
    if columns == 1 {
        return vec![total];
    }

    let others = u16::try_from(columns - 1).unwrap_or(1);
    let per_other = (total / u16::try_from(columns).unwrap_or(1)).max(6);
    let used = per_other.saturating_mul(others);
    let name = total.saturating_sub(used).max(MIN_NAME_WIDTH);

    let mut widths = vec![name];
    widths.extend(std::iter::repeat_n(per_other, columns - 1));
    widths
}

/// Lays cells out in fixed-width columns, cutting any that overflow.
#[must_use]
pub fn format_cells(cells: &[String], widths: &[u16]) -> String {
    let mut out = String::new();

    for (index, cell) in cells.iter().enumerate() {
        let width = widths.get(index).copied().unwrap_or(10) as usize;
        let width = width.max(1);

        let count = cell.chars().count();
        if count >= width {
            // Leave one space between columns, so two full cells do not run
            // together into one unreadable word.
            let cut: String = cell.chars().take(width.saturating_sub(1)).collect();
            out.push_str(&cut);
            out.push(' ');
        } else {
            out.push_str(cell);
            out.push_str(&" ".repeat(width - count));
        }
    }

    out.trim_end().to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn no_columns_means_no_widths() {
        assert!(column_widths(80, 0).is_empty());
    }

    #[test]
    fn one_column_takes_the_whole_width() {
        assert_eq!(column_widths(80, 1), vec![80]);
    }

    #[test]
    fn the_name_column_gets_what_the_others_leave() {
        let widths = column_widths(100, 5);
        assert_eq!(widths.len(), 5);
        assert!(
            widths[0] >= MIN_NAME_WIDTH,
            "the name column was {}",
            widths[0]
        );
        assert!(widths[1..].iter().all(|w| *w >= 6));
    }

    #[test]
    fn a_narrow_terminal_still_leaves_the_name_readable() {
        // Names are what the user is scanning for; a two-character name column
        // is the same as no table at all.
        let widths = column_widths(40, 6);
        assert!(widths[0] >= MIN_NAME_WIDTH, "{widths:?}");
    }

    #[test]
    fn cells_are_padded_to_their_column() {
        let line = format_cells(&["api".to_string(), "1/1".to_string()], &[10, 6]);
        assert!(line.starts_with("api       "), "{line:?}");
        assert_eq!(line, "api       1/1");
    }

    #[test]
    fn an_overlong_cell_is_cut_and_kept_apart_from_the_next() {
        let line = format_cells(
            &["a-very-long-pod-name-indeed".to_string(), "1/1".to_string()],
            &[10, 6],
        );
        // Nine characters of name, one space, then the next column.
        assert_eq!(line, "a-very-lo 1/1");
    }

    #[test]
    fn a_cell_with_no_width_still_renders() {
        let line = format_cells(&["x".to_string(), "y".to_string()], &[]);
        assert!(line.contains('x') && line.contains('y'), "{line:?}");
    }

    #[test]
    fn trailing_padding_is_trimmed() {
        let line = format_cells(&["a".to_string(), "b".to_string()], &[10, 10]);
        assert!(!line.ends_with(' '), "{line:?}");
    }

    #[test]
    fn cutting_counts_characters_rather_than_bytes() {
        let line = format_cells(&["éééééééééé".to_string()], &[5]);
        assert_eq!(line.chars().count(), 4, "{line:?}");
    }
}
