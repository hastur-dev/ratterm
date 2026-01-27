//! Health Dashboard widget for rendering device metrics.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use super::{DashboardHost, DashboardMode, HealthDashboard};
use crate::ssh::MetricStatus;

/// Progress bar width for metrics.
const PROGRESS_WIDTH: u16 = 10;

/// Widget for rendering the Health Dashboard.
pub struct HealthDashboardWidget<'a> {
    /// Dashboard state reference.
    dashboard: &'a HealthDashboard,
    /// Whether the widget is focused.
    focused: bool,
}

impl<'a> HealthDashboardWidget<'a> {
    /// Creates a new Health Dashboard widget.
    #[must_use]
    pub fn new(dashboard: &'a HealthDashboard) -> Self {
        Self {
            dashboard,
            focused: true,
        }
    }

    /// Sets the focused state.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for HealthDashboardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the background
        Clear.render(area, buf);

        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = match self.dashboard.mode() {
            DashboardMode::Overview => "SSH Health Dashboard",
            DashboardMode::Detail => {
                if self.dashboard.selected_host().is_some() {
                    // We can't return a temporary, so just use a fixed title
                    "SSH Health Dashboard - Detail View"
                } else {
                    "SSH Health Dashboard"
                }
            }
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        // Split into header, content, and footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(3),    // Content
                Constraint::Length(2), // Footer
            ])
            .split(inner);

        render_header(self.dashboard, chunks[0], buf);

        match self.dashboard.mode() {
            DashboardMode::Overview => render_overview(self.dashboard, chunks[1], buf),
            DashboardMode::Detail => render_detail(self.dashboard, chunks[1], buf),
        }

        render_footer(self.dashboard, chunks[2], buf);
    }
}

/// Renders the header with summary stats.
fn render_header(dashboard: &HealthDashboard, area: Rect, buf: &mut Buffer) {
    let online = dashboard.online_count();
    let offline = dashboard.offline_count();
    let total = dashboard.host_count();
    let time_since = dashboard.time_since_refresh();
    let auto_status = if dashboard.auto_refresh() {
        "ON"
    } else {
        "OFF"
    };

    let header = Line::from(vec![
        Span::styled(
            format!(" {} devices ", total),
            Style::default().fg(Color::White),
        ),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" {} online ", online),
            Style::default().fg(Color::Green),
        ),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" {} offline ", offline),
            Style::default().fg(Color::Red),
        ),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" Last update: {}s ago ", time_since),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" Auto: {} ", auto_status),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let para = Paragraph::new(header);
    para.render(area, buf);
}

/// Renders the overview mode showing all hosts.
fn render_overview(dashboard: &HealthDashboard, area: Rect, buf: &mut Buffer) {
    let hosts = dashboard.hosts();
    let selected = dashboard.selected_index();
    let scroll = dashboard.scroll_offset();

    if hosts.is_empty() {
        let msg = Paragraph::new("No hosts with credentials available")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        msg.render(area, buf);
        return;
    }

    // Each host takes 2 lines
    let visible_count = (area.height / 2) as usize;
    let visible_hosts = hosts.iter().skip(scroll).take(visible_count);

    let mut y = area.y;
    for (idx, host) in visible_hosts.enumerate() {
        let actual_idx = scroll + idx;
        let is_selected = actual_idx == selected;

        if y + 1 >= area.bottom() {
            break;
        }

        render_host_row(host, is_selected, Rect::new(area.x, y, area.width, 2), buf);
        y += 2;
    }
}

/// Renders a single host row in overview mode.
fn render_host_row(host: &DashboardHost, selected: bool, area: Rect, buf: &mut Buffer) {
    let bg_color = if selected {
        Color::Rgb(50, 50, 70)
    } else {
        Color::Reset
    };

    // Fill background if selected
    if selected {
        for y in area.y..area.bottom().min(area.y + 2) {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }
    }

    // Line 1: Name (hostname) [STATUS]
    let indicator = if selected { "▸ " } else { "  " };
    let status_str = host.metrics.status.as_str();
    let status_color = match host.metrics.status {
        MetricStatus::Online => Color::Green,
        MetricStatus::Offline => Color::Red,
        MetricStatus::Collecting => Color::Yellow,
        MetricStatus::Error => Color::Magenta,
        MetricStatus::Unknown => Color::DarkGray,
    };

    let line1 = Line::from(vec![
        Span::styled(indicator, Style::default().fg(Color::Cyan)),
        Span::styled(
            &host.display_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({})", host.connection_string()),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", status_str),
            Style::default().fg(status_color),
        ),
    ]);

    let para1 = Paragraph::new(line1);
    para1.render(Rect::new(area.x, area.y, area.width, 1), buf);

    // Line 2: Metrics bars (only if online)
    if host.metrics.status.is_online() {
        render_metrics_line(
            host,
            Rect::new(area.x + 2, area.y + 1, area.width - 2, 1),
            buf,
        );
    } else {
        let empty_line = Line::from(vec![Span::styled(
            "  CPU: --        RAM: --         DISK: --         GPU: --",
            Style::default().fg(Color::DarkGray),
        )]);
        let para2 = Paragraph::new(empty_line);
        para2.render(Rect::new(area.x + 2, area.y + 1, area.width - 2, 1), buf);
    }
}

/// Renders the metrics line with progress bars.
fn render_metrics_line(host: &DashboardHost, area: Rect, buf: &mut Buffer) {
    let m = &host.metrics;

    let cpu_bar = progress_bar(m.cpu_usage_percent, PROGRESS_WIDTH, Color::Green);
    let ram_bar = progress_bar(m.memory_percent(), PROGRESS_WIDTH, Color::Blue);
    let disk_bar = progress_bar(m.disk_percent(), PROGRESS_WIDTH, Color::Yellow);

    let gpu_spans = if let Some(ref gpu) = m.gpu {
        if gpu.is_available() {
            vec![
                Span::styled("GPU: ", Style::default().fg(Color::DarkGray)),
                progress_bar_span(gpu.usage_percent, 5, Color::Magenta),
                Span::styled(
                    format!(" {:3.0}%", gpu.usage_percent),
                    Style::default().fg(Color::White),
                ),
            ]
        } else {
            vec![Span::styled(
                "GPU: N/A",
                Style::default().fg(Color::DarkGray),
            )]
        }
    } else {
        vec![Span::styled(
            "GPU: N/A",
            Style::default().fg(Color::DarkGray),
        )]
    };

    let mut spans = vec![Span::styled("CPU: ", Style::default().fg(Color::DarkGray))];
    spans.extend(cpu_bar);
    spans.push(Span::styled(
        format!(" {:3.0}%  ", m.cpu_usage_percent),
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled("RAM: ", Style::default().fg(Color::DarkGray)));
    spans.extend(ram_bar);
    spans.push(Span::styled(
        format!(" {:3.0}%  ", m.memory_percent()),
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled("DISK: ", Style::default().fg(Color::DarkGray)));
    spans.extend(disk_bar);
    spans.push(Span::styled(
        format!(" {:3.0}%  ", m.disk_percent()),
        Style::default().fg(Color::White),
    ));
    spans.extend(gpu_spans);

    let line = Line::from(spans);
    let para = Paragraph::new(line);
    para.render(area, buf);
}

/// Creates a progress bar as a vector of spans.
fn progress_bar(percent: f32, width: u16, color: Color) -> Vec<Span<'static>> {
    let filled = ((percent / 100.0) * width as f32).round() as u16;
    let empty = width.saturating_sub(filled);

    vec![
        Span::styled("█".repeat(filled as usize), Style::default().fg(color)),
        Span::styled(
            "░".repeat(empty as usize),
            Style::default().fg(Color::DarkGray),
        ),
    ]
}

/// Creates a progress bar as a single span (compact version).
fn progress_bar_span(percent: f32, width: u16, color: Color) -> Span<'static> {
    let filled = ((percent / 100.0) * width as f32).round() as u16;
    let empty = width.saturating_sub(filled);

    let bar = format!(
        "{}{}",
        "█".repeat(filled as usize),
        "░".repeat(empty as usize)
    );

    Span::styled(bar, Style::default().fg(color))
}

/// Renders the detail mode for a single host.
fn render_detail(dashboard: &HealthDashboard, area: Rect, buf: &mut Buffer) {
    let host = match dashboard.selected_host() {
        Some(h) => h,
        None => {
            let msg = Paragraph::new("No host selected")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray));
            msg.render(area, buf);
            return;
        }
    };

    let m = &host.metrics;
    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(
            &host.display_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({}) ", host.connection_string()),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("[{}]", m.status.as_str()),
            Style::default().fg(if m.status.is_online() {
                Color::Green
            } else {
                Color::Red
            }),
        ),
    ]));
    lines.push(Line::from(""));

    if !m.status.is_online() {
        if let Some(ref err) = m.error {
            lines.push(Line::from(Span::styled(
                format!("Error: {}", err),
                Style::default().fg(Color::Red),
            )));
        }
        let para = Paragraph::new(lines);
        para.render(area, buf);
        return;
    }

    // CPU Section
    lines.push(Line::from(Span::styled(
        "CPU",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("├─ Usage:      ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:5.1}%  ", m.cpu_usage_percent),
            Style::default().fg(Color::White),
        ),
        progress_bar_span(m.cpu_usage_percent, 20, Color::Green),
    ]));
    lines.push(Line::from(vec![
        Span::styled("├─ Cores:      ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", m.cpu_cores),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("└─ Load Avg:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "{:.2} / {:.2} / {:.2} (1m/5m/15m)",
                m.load_avg.0, m.load_avg.1, m.load_avg.2
            ),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    // Memory Section
    lines.push(Line::from(Span::styled(
        "Memory",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("├─ Total:      ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} MB", m.mem_total_mb),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("├─ Used:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} MB ({:.1}%)  ", m.mem_used_mb, m.memory_percent()),
            Style::default().fg(Color::White),
        ),
        progress_bar_span(m.memory_percent(), 20, Color::Blue),
    ]));
    lines.push(Line::from(vec![
        Span::styled("├─ Available:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} MB", m.mem_available_mb),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("└─ Swap:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} / {} MB", m.swap_used_mb, m.swap_total_mb),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    // Disk Section
    lines.push(Line::from(Span::styled(
        "Disk (/)",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("├─ Total:      ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} GB", m.disk_total_gb),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("└─ Used:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} GB ({:.1}%)  ", m.disk_used_gb, m.disk_percent()),
            Style::default().fg(Color::White),
        ),
        progress_bar_span(m.disk_percent(), 20, Color::Yellow),
    ]));
    lines.push(Line::from(""));

    // GPU Section
    if let Some(ref gpu) = m.gpu {
        if gpu.is_available() {
            lines.push(Line::from(Span::styled(
                format!("GPU: {}", gpu.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("├─ Utilization: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.1}%  ", gpu.usage_percent),
                    Style::default().fg(Color::White),
                ),
                progress_bar_span(gpu.usage_percent, 20, Color::Magenta),
            ]));
            lines.push(Line::from(vec![
                Span::styled("├─ Memory:      ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} / {} MB", gpu.memory_used_mb, gpu.memory_total_mb),
                    Style::default().fg(Color::White),
                ),
            ]));
            if let Some(temp) = gpu.temperature_celsius {
                lines.push(Line::from(vec![
                    Span::styled("└─ Temperature: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}°C", temp), Style::default().fg(Color::White)),
                ]));
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "GPU: Not detected",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let para = Paragraph::new(lines);
    para.render(area, buf);
}

/// Renders the footer with help text.
fn render_footer(dashboard: &HealthDashboard, area: Rect, buf: &mut Buffer) {
    let help = match dashboard.mode() {
        DashboardMode::Overview => {
            "[↑/↓] Navigate  [Enter] Details  [r] Refresh  [Space] Toggle auto  [q/Esc] Close"
        }
        DashboardMode::Detail => "[Backspace] Back  [r] Refresh  [q/Esc] Close",
    };

    let footer = Line::from(Span::styled(help, Style::default().fg(Color::DarkGray)));
    let para = Paragraph::new(footer).alignment(Alignment::Center);
    para.render(area, buf);
}
