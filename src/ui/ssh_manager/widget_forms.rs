//! SSH Manager form rendering functions.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::selector::SSHManagerSelector;
use super::types::{AddHostField, ScanCredentialField};

/// Renders the add host mode.
pub fn render_add_host(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
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

    let current_field = selector.add_host_field();

    let title = Paragraph::new("Add New SSH Host")
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    title.render(chunks[0], buf);

    let desc = Paragraph::new("Enter host details and credentials (Tab to switch fields)")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    desc.render(chunks[1], buf);

    render_add_host_field(
        chunks[3],
        chunks[4],
        buf,
        "Hostname/IP:",
        selector.hostname_input(),
        current_field == AddHostField::Hostname,
    );
    render_add_host_field(
        chunks[5],
        chunks[6],
        buf,
        "Port:",
        selector.port_input(),
        current_field == AddHostField::Port,
    );
    render_add_host_field(
        chunks[7],
        chunks[8],
        buf,
        "Display Name (optional):",
        selector.add_host_display_name(),
        current_field == AddHostField::DisplayName,
    );
    render_add_host_field(
        chunks[9],
        chunks[10],
        buf,
        "Username:",
        selector.add_host_username(),
        current_field == AddHostField::Username,
    );

    let password = selector.add_host_password();
    let masked = "*".repeat(password.len());
    render_add_host_field(
        chunks[11],
        chunks[12],
        buf,
        "Password:",
        &masked,
        current_field == AddHostField::Password,
    );

    let footer = Line::from(vec![
        Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
        Span::raw(" Next Field "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Add Host "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]);
    let footer_para = Paragraph::new(footer).alignment(Alignment::Center);
    footer_para.render(chunks[14], buf);
}

/// Helper to render an add host field.
fn render_add_host_field(
    label_area: Rect,
    input_area: Rect,
    buf: &mut Buffer,
    label: &str,
    value: &str,
    is_active: bool,
) {
    let label_style = if is_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    Paragraph::new(label)
        .style(label_style)
        .render(label_area, buf);

    let input_text = if is_active {
        format!("{}_", value)
    } else {
        value.to_string()
    };
    let input_style = if is_active {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };
    Paragraph::new(input_text)
        .style(input_style)
        .render(input_area, buf);
}

/// Renders the scan credential entry mode.
pub fn render_scan_credential_entry(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Scan Network with Credentials")
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    title.render(chunks[0], buf);

    let desc = Paragraph::new("Only hosts that accept these credentials will be added")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    desc.render(chunks[1], buf);

    let username_style = if selector.scan_credential_field() == ScanCredentialField::Username {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Paragraph::new("Username:")
        .style(username_style)
        .render(chunks[3], buf);

    let username_input = if selector.scan_credential_field() == ScanCredentialField::Username {
        format!("{}_", selector.scan_username())
    } else {
        selector.scan_username().to_string()
    };
    Paragraph::new(username_input)
        .style(Style::default().bg(Color::DarkGray))
        .render(chunks[4], buf);

    let password_style = if selector.scan_credential_field() == ScanCredentialField::Password {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Paragraph::new("Password:")
        .style(password_style)
        .render(chunks[5], buf);

    let masked_len = selector.scan_password().len();
    let password_display = if selector.scan_credential_field() == ScanCredentialField::Password {
        format!("{}_", "*".repeat(masked_len))
    } else {
        "*".repeat(masked_len)
    };
    Paragraph::new(password_display)
        .style(Style::default().bg(Color::DarkGray))
        .render(chunks[6], buf);

    let subnet_style = if selector.scan_credential_field() == ScanCredentialField::Subnet {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    Paragraph::new("Subnet (e.g. 10.0.0.0/24, blank=auto):")
        .style(subnet_style)
        .render(chunks[7], buf);

    let subnet_input = if selector.scan_credential_field() == ScanCredentialField::Subnet {
        format!("{}_", selector.scan_subnet())
    } else {
        selector.scan_subnet().to_string()
    };
    Paragraph::new(subnet_input)
        .style(Style::default().bg(Color::DarkGray))
        .render(chunks[8], buf);

    let footer = Line::from(vec![
        Span::styled("[Tab]", Style::default().fg(Color::Yellow)),
        Span::raw(" Next "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Start Scan "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]);
    Paragraph::new(footer)
        .alignment(Alignment::Center)
        .render(chunks[10], buf);
}

/// Renders the edit name mode.
pub fn render_edit_name(selector: &SSHManagerSelector, area: Rect, buf: &mut Buffer) {
    let host_info = selector
        .edit_name_target()
        .and_then(|id| {
            selector
                .hosts
                .iter()
                .find(|h| h.host.id == id)
                .map(|h| h.host.hostname.clone())
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
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    Paragraph::new("Edit Display Name")
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .render(chunks[0], buf);

    Paragraph::new(format!("Host: {}", host_info))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray))
        .render(chunks[1], buf);

    Paragraph::new("Display Name:")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[3], buf);

    let name_text = format!("{}_", selector.edit_name_input());
    Paragraph::new(name_text)
        .style(Style::default().bg(Color::DarkGray))
        .render(chunks[4], buf);

    let footer = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" Save "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Cancel"),
    ]);
    Paragraph::new(footer)
        .alignment(Alignment::Center)
        .render(chunks[6], buf);
}
