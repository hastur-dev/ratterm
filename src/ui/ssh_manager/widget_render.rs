//! SSH Manager widget rendering functions.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Row, Table, Widget},
};

use crate::ssh::ConnectionStatus;
use crate::ui::key_hint_bar::{KeyHint, KeyHintStyle};
use crate::ui::manager_footer::ManagerFooter;

use super::selector::SSHManagerSelector;
use super::types::CredentialField;

/// Renders the host list mode.
pub fn render_list(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let inner = area;

    // Separator line
    let sep_height: u16 = 1;
    // Footer: 2 rows for primary + secondary hints
    let footer_height: u16 = 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(sep_height),
            Constraint::Min(3),
            Constraint::Length(footer_height),
        ])
        .split(inner);

    let sep = "\u{2500}".repeat(chunks[0].width as usize);
    buf.set_string(
        chunks[0].x,
        chunks[0].y,
        &sep,
        Style::default().fg(Color::DarkGray),
    );

    if selector.is_empty() {
        let empty = Paragraph::new("No SSH hosts saved. Press [s] to scan or [a] to add.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        empty.render(chunks[1], buf);
    } else {
        render_host_table(selector, chunks[1], buf);
    }

    // Render ManagerFooter with all SSH hotkeys
    let primary = vec![
        KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
        KeyHint::new("a", "Add Host"),
        KeyHint::new("e", "Edit"),
        KeyHint::styled("d", "Delete", KeyHintStyle::Danger),
        KeyHint::new("s", "Scan Network"),
    ];
    let secondary = vec![
        KeyHint::new("c", "Credential Scan"),
        KeyHint::new("Shift+S", "Scan Subnet"),
        KeyHint::new("Ctrl+1-9", "Quick Connect"),
        KeyHint::styled("Esc", "Close", KeyHintStyle::Danger),
    ];

    let footer = ManagerFooter::new(primary).secondary(secondary);
    footer.render(chunks[2], buf);
}

/// Renders the host table.
pub fn render_host_table(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let header = Row::new(vec!["#", "Name", "Host", "Status"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .height(1);

    let selected_index = selector.selected_index;

    let rows: Vec<Row> = selector
        .visible_hosts()
        .map(|(idx, host)| {
            let is_selected = idx == selected_index;
            let style = if is_selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let _status_style = match host.status {
                ConnectionStatus::Unknown => Style::default().fg(Color::DarkGray),
                ConnectionStatus::Reachable => Style::default().fg(Color::Green),
                ConnectionStatus::Unreachable => Style::default().fg(Color::Red),
                ConnectionStatus::Authenticated => Style::default().fg(Color::Cyan),
            };

            let creds_indicator = if host.has_credentials { "*" } else { "" };

            Row::new(vec![
                format!("{}", idx + 1),
                format!("{}{}", host.host.display(), creds_indicator),
                host.host.connection_string(),
                host.status.as_str().to_string(),
            ])
            .style(style)
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Percentage(35),
        Constraint::Percentage(40),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    Widget::render(table, area, buf);
}

/// Renders the credential entry mode.
pub fn render_credential_entry(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let target_name = selector
        .credential_target()
        .and_then(|id| {
            selector
                .hosts
                .iter()
                .find(|h| h.host.id == id)
                .map(|h| h.host.display().to_string())
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let target = Paragraph::new(format!("Connect to: {}", target_name))
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    target.render(chunks[0], buf);

    let username_style = if selector.credential_field() == CredentialField::Username {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let username_label = Paragraph::new("Username:").style(username_style);
    username_label.render(chunks[2], buf);

    let username_input =
        Paragraph::new(selector.username()).style(Style::default().bg(Color::DarkGray));
    username_input.render(chunks[3], buf);

    let password_style = if selector.credential_field() == CredentialField::Password {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let password_label = Paragraph::new("Password:").style(password_style);
    password_label.render(chunks[4], buf);

    let masked_password = "*".repeat(selector.password().len());
    let password_input =
        Paragraph::new(masked_password).style(Style::default().bg(Color::DarkGray));
    password_input.render(chunks[5], buf);

    let checkbox = if selector.save_credentials() {
        "[x] Save credentials"
    } else {
        "[ ] Save credentials"
    };
    let save_para = Paragraph::new(checkbox).style(Style::default().fg(Color::Cyan));
    save_para.render(chunks[6], buf);

    // Credential entry footer with ManagerFooter
    let cred_hints = vec![
        KeyHint::new("Tab", "Next Field"),
        KeyHint::new("Space", "Toggle Save"),
        KeyHint::styled("Enter", "Connect", KeyHintStyle::Success),
        KeyHint::styled("Esc", "Cancel", KeyHintStyle::Danger),
    ];
    let cred_footer = ManagerFooter::new(cred_hints);
    cred_footer.render(chunks[8], buf);
}

/// Renders the scanning mode.
pub fn render_scanning(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let (scanned, total) = selector.scan_progress().unwrap_or((0, 0));
    let percentage = if total > 0 {
        (scanned * 100) / total
    } else {
        0
    };

    let subnet_info = selector
        .scanning_subnet()
        .map(|s| format!("Subnet: {}", s))
        .unwrap_or_else(|| "Detecting network...".to_string());

    let progress_text = format!(
        "Scanning for SSH hosts... {}/{} ({}%)",
        scanned, total, percentage
    );

    let bar_width = area.width.saturating_sub(4) as usize;
    let filled = if total > 0 {
        (bar_width * scanned) / total
    } else {
        0
    };
    let empty = bar_width.saturating_sub(filled);
    let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Percentage(30),
        ])
        .split(area);

    let subnet_para = Paragraph::new(subnet_info)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));
    subnet_para.render(chunks[1], buf);

    let progress_para = Paragraph::new(progress_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow));
    progress_para.render(chunks[2], buf);

    let bar_para = Paragraph::new(progress_bar)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Green));
    bar_para.render(chunks[3], buf);

    let cancel_hint = Paragraph::new("Press [Esc] to cancel")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    cancel_hint.render(chunks[4], buf);
}

/// Renders the authenticated scanning mode.
pub fn render_authenticated_scanning(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let (scanned, total) = selector.scan_progress().unwrap_or((0, 0));
    let percentage = if total > 0 {
        (scanned * 100) / total
    } else {
        0
    };

    let subnet_info = selector
        .scanning_subnet()
        .map(|s| format!("Subnet: {}", s))
        .unwrap_or_else(|| "Detecting network...".to_string());

    let progress_text = format!(
        "Scanning and authenticating... {}/{} ({}%)",
        scanned, total, percentage
    );

    let auth_stats = format!(
        "Authenticated: {} | Failed: {}",
        selector.auth_success_count(),
        selector.auth_fail_count()
    );

    let bar_width = area.width.saturating_sub(4) as usize;
    let filled = if total > 0 {
        (bar_width * scanned) / total
    } else {
        0
    };
    let empty = bar_width.saturating_sub(filled);
    let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Percentage(25),
        ])
        .split(area);

    Paragraph::new(subnet_info)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan))
        .render(chunks[1], buf);

    Paragraph::new(progress_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[2], buf);

    Paragraph::new(progress_bar)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Green))
        .render(chunks[3], buf);

    Paragraph::new(auth_stats)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Magenta))
        .render(chunks[4], buf);

    Paragraph::new("Press [Esc] to cancel")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray))
        .render(chunks[5], buf);
}
