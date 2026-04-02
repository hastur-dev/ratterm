//! Git Dashboard widget rendering.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use crate::git::dashboard::{
    GitDashboard, GitDashboardMode, GitDashboardView, StatusSection,
};
use crate::git::api::StatusKind;

/// Widget for rendering the Git Dashboard popup.
pub struct GitDashboardWidget<'a> {
    dashboard: &'a GitDashboard,
    position: Option<crate::ui::window_position::WindowPosition>,
}

impl<'a> GitDashboardWidget<'a> {
    /// Creates a new Git Dashboard widget.
    #[must_use]
    pub fn new(dashboard: &'a GitDashboard) -> Self {
        Self {
            dashboard,
            position: None,
        }
    }

    /// Sets the window position from config.
    #[must_use]
    pub fn position(mut self, pos: crate::ui::window_position::WindowPosition) -> Self {
        self.position = Some(pos);
        self
    }

    /// Calculates the popup area.
    fn popup_area(&self, area: Rect) -> Rect {
        let width = area.width.saturating_sub(8).min(90);
        let height = area.height.saturating_sub(4).min(30);

        match &self.position {
            Some(pos) => pos.resolve(width, height, area.width, area.height),
            None => {
                let x = (area.width.saturating_sub(width)) / 2;
                let y = (area.height.saturating_sub(height)) / 2;
                Rect::new(x, y, width, height)
            }
        }
    }
}

impl Widget for GitDashboardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);
        Clear.render(popup_area, buf);

        let title = format!(
            " Git: {} [{}] ",
            self.dashboard.current_branch,
            match self.dashboard.view {
                GitDashboardView::Status => "Status",
                GitDashboardView::Log => "Log",
                GitDashboardView::Branches => "Branches",
                GitDashboardView::Diff => "Diff",
            }
        );

        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        match self.dashboard.mode {
            GitDashboardMode::CommitMessage => {
                render_commit_form(self.dashboard, inner, buf);
            }
            GitDashboardMode::List => match self.dashboard.view {
                GitDashboardView::Status => {
                    render_status_view(self.dashboard, inner, buf);
                }
                GitDashboardView::Log => {
                    render_log_view(self.dashboard, inner, buf);
                }
                GitDashboardView::Branches => {
                    render_branch_view(self.dashboard, inner, buf);
                }
                GitDashboardView::Diff => {
                    render_diff_placeholder(self.dashboard, inner, buf);
                }
            },
        }

        // Render error if any
        if let Some(ref error) = self.dashboard.error {
            let err_line = Line::from(Span::styled(
                error.clone(),
                Style::default().fg(Color::Red),
            ));
            let err_y = popup_area.y + popup_area.height.saturating_sub(2);
            if err_y < popup_area.y + popup_area.height {
                buf.set_line(popup_area.x + 2, err_y, &err_line, popup_area.width - 4);
            }
        }

        // Render footer with key hints
        render_footer(self.dashboard, popup_area, buf);
    }
}

/// Renders the status view with staged/unstaged/untracked sections.
fn render_status_view(dashboard: &GitDashboard, area: Rect, buf: &mut Buffer) {
    if area.height < 3 || area.width < 10 {
        return;
    }

    // Section tabs at the top
    let tab_line = Line::from(vec![
        section_tab("Staged", dashboard.staged_files.len(), dashboard.section == StatusSection::Staged),
        Span::raw(" | "),
        section_tab("Unstaged", dashboard.unstaged_files.len(), dashboard.section == StatusSection::Unstaged),
        Span::raw(" | "),
        section_tab("Untracked", dashboard.untracked_files.len(), dashboard.section == StatusSection::Untracked),
    ]);
    buf.set_line(area.x, area.y, &tab_line, area.width);

    // File list
    let list_area = Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(1));
    let files = match dashboard.section {
        StatusSection::Staged => &dashboard.staged_files,
        StatusSection::Unstaged => &dashboard.unstaged_files,
        StatusSection::Untracked => &dashboard.untracked_files,
    };

    for (i, file) in files.iter().enumerate() {
        let y = list_area.y + i as u16;
        if y >= list_area.y + list_area.height {
            break;
        }

        let is_selected = i == dashboard.selected_index;
        let icon = match file.kind {
            StatusKind::New => "+",
            StatusKind::Modified => "~",
            StatusKind::Deleted => "-",
            StatusKind::Renamed => "R",
            StatusKind::Conflicted => "!",
            StatusKind::TypeChange => "T",
        };
        let icon_color = match file.kind {
            StatusKind::New => Color::Green,
            StatusKind::Modified => Color::Yellow,
            StatusKind::Deleted => Color::Red,
            StatusKind::Renamed => Color::Cyan,
            StatusKind::Conflicted => Color::Magenta,
            StatusKind::TypeChange => Color::Blue,
        };

        let line = Line::from(vec![
            Span::styled(
                if is_selected { "> " } else { "  " },
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{} ", icon),
                Style::default().fg(icon_color),
            ),
            Span::styled(
                file.path.clone(),
                if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]);

        buf.set_line(list_area.x, y, &line, list_area.width);
    }

    if files.is_empty() {
        let empty = Line::from(Span::styled(
            "  (empty)",
            Style::default().fg(Color::DarkGray),
        ));
        buf.set_line(list_area.x, list_area.y, &empty, list_area.width);
    }
}

/// Renders a section tab label.
fn section_tab(name: &str, count: usize, active: bool) -> Span<'_> {
    let label = format!("{} ({})", name, count);
    if active {
        Span::styled(label, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
    } else {
        Span::styled(label, Style::default().fg(Color::DarkGray))
    }
}

/// Renders the commit log view.
fn render_log_view(dashboard: &GitDashboard, area: Rect, buf: &mut Buffer) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let header = Line::from(Span::styled(
        "Commit Log",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    buf.set_line(area.x, area.y, &header, area.width);

    for (i, commit) in dashboard.commit_log.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.y + area.height {
            break;
        }

        let is_selected = i == dashboard.selected_index;
        let line = Line::from(vec![
            Span::styled(
                if is_selected { "> " } else { "  " },
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{} ", commit.short_hash),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                truncate_str(&commit.message, (area.width as usize).saturating_sub(20)),
                if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::styled(
                format!(" ({})", commit.author),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        buf.set_line(area.x, y, &line, area.width);
    }
}

/// Renders the branch list view.
fn render_branch_view(dashboard: &GitDashboard, area: Rect, buf: &mut Buffer) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let header = Line::from(Span::styled(
        "Branches",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    buf.set_line(area.x, area.y, &header, area.width);

    for (i, branch) in dashboard.branch_list.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.y + area.height {
            break;
        }

        let is_selected = i == dashboard.selected_index;
        let marker = if branch.is_current { "* " } else { "  " };
        let remote_tag = if branch.is_remote { " [remote]" } else { "" };

        let line = Line::from(vec![
            Span::styled(
                if is_selected { "> " } else { "  " },
                Style::default().fg(Color::White),
            ),
            Span::styled(
                marker.to_string(),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                branch.name.clone(),
                if branch.is_current {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::styled(
                format!(" {}{}", branch.last_commit, remote_tag),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        buf.set_line(area.x, y, &line, area.width);
    }
}

/// Renders a placeholder for the diff view.
fn render_diff_placeholder(dashboard: &GitDashboard, area: Rect, buf: &mut Buffer) {
    if area.height < 2 {
        return;
    }

    let selected = dashboard.selected_file_path().unwrap_or("(no file selected)");
    let header = Line::from(vec![
        Span::styled("Diff: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(selected.to_string(), Style::default().fg(Color::Yellow)),
    ]);
    buf.set_line(area.x, area.y, &header, area.width);

    let hint = Line::from(Span::styled(
        "Press Backspace to return to status view",
        Style::default().fg(Color::DarkGray),
    ));
    buf.set_line(area.x, area.y + 1, &hint, area.width);
}

/// Renders the commit message form.
fn render_commit_form(dashboard: &GitDashboard, area: Rect, buf: &mut Buffer) {
    if area.height < 4 || area.width < 20 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Amend toggle
            Constraint::Min(3),   // Message input
            Constraint::Length(1), // Help
        ])
        .split(area);

    // Title
    let title = Line::from(Span::styled(
        "Commit Message",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    buf.set_line(chunks[0].x, chunks[0].y, &title, chunks[0].width);

    // Amend toggle
    let amend_text = if dashboard.amend {
        "[x] Amend (Ctrl+A to toggle)"
    } else {
        "[ ] Amend (Ctrl+A to toggle)"
    };
    let amend_line = Line::from(Span::styled(
        amend_text,
        Style::default().fg(Color::DarkGray),
    ));
    buf.set_line(chunks[1].x, chunks[1].y, &amend_line, chunks[1].width);

    // Message input
    let msg_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Message ");
    let msg_inner = msg_block.inner(chunks[2]);
    msg_block.render(chunks[2], buf);

    let display_msg = if dashboard.commit_message.is_empty() {
        "Type your commit message...".to_string()
    } else {
        format!("{}_", dashboard.commit_message)
    };
    let msg_style = if dashboard.commit_message.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let msg = Paragraph::new(display_msg).style(msg_style);
    msg.render(msg_inner, buf);

    // Help
    let help = Line::from(Span::styled(
        "Enter=commit | Esc=cancel | Ctrl+A=toggle amend",
        Style::default().fg(Color::DarkGray),
    ));
    buf.set_line(chunks[3].x, chunks[3].y, &help, chunks[3].width);
}

/// Renders the footer with key hints.
fn render_footer(dashboard: &GitDashboard, popup_area: Rect, buf: &mut Buffer) {
    let footer_y = popup_area.y + popup_area.height - 1;
    if footer_y <= popup_area.y {
        return;
    }

    let hints = match dashboard.view {
        GitDashboardView::Status => "s=stage u=unstage c=commit Tab=section b=branch l=log ?=help",
        GitDashboardView::Log => "Backspace=back ?=help",
        GitDashboardView::Branches => "Enter=checkout Backspace=back ?=help",
        GitDashboardView::Diff => "Backspace=back ?=help",
    };

    let footer = Line::from(Span::styled(
        format!(" {} ", hints),
        Style::default().fg(Color::DarkGray),
    ));
    buf.set_line(popup_area.x + 1, footer_y, &footer, popup_area.width - 2);
}

/// Truncates a string to fit within max_len.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::dashboard::GitDashboard;

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn test_widget_creation() {
        let dash = GitDashboard::new("/tmp/test");
        let widget = GitDashboardWidget::new(&dash);
        // Should not panic
        let area = Rect::new(0, 0, 80, 24);
        let popup = widget.popup_area(area);
        assert!(popup.width > 0);
        assert!(popup.height > 0);
    }
}
