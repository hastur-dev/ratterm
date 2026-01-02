//! Docker Manager form rendering.
//!
//! Provides form rendering for Docker run options and
//! utility functions for future UI customization.

#![allow(dead_code)]

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::selector::DockerManagerSelector;
use super::types::{HostCredentialField, RunOptionsField};

/// Renders the run options form.
pub fn render_run_options_form(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    if area.height < 10 || area.width < 30 {
        return;
    }

    // Layout: fields + buttons
    let chunks = Layout::vertical([
        Constraint::Length(3), // Name
        Constraint::Length(3), // Ports
        Constraint::Length(3), // Volumes
        Constraint::Length(3), // Env vars
        Constraint::Length(3), // Shell
        Constraint::Min(2),    // Buttons/help
    ])
    .split(area);

    let current_field = selector.run_options_field();
    let input_buffer = selector.input_buffer();
    let run_options = selector.run_options();

    // Render each field
    render_field(
        RunOptionsField::Name,
        &run_options.name.clone().unwrap_or_default(),
        current_field,
        input_buffer,
        chunks[0],
        buf,
    );

    render_field(
        RunOptionsField::Ports,
        &run_options.port_mappings.join(", "),
        current_field,
        input_buffer,
        chunks[1],
        buf,
    );

    render_field(
        RunOptionsField::Volumes,
        &run_options.volume_mounts.join(", "),
        current_field,
        input_buffer,
        chunks[2],
        buf,
    );

    render_field(
        RunOptionsField::EnvVars,
        &run_options.env_vars.join(", "),
        current_field,
        input_buffer,
        chunks[3],
        buf,
    );

    render_field(
        RunOptionsField::Shell,
        &run_options.shell,
        current_field,
        input_buffer,
        chunks[4],
        buf,
    );

    // Render help text
    render_form_help(chunks[5], buf);
}

/// Renders a single form field.
fn render_field(
    field: RunOptionsField,
    value: &str,
    current_field: RunOptionsField,
    input_buffer: &str,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height < 3 {
        return;
    }

    let is_current = field == current_field;

    let border_style = if is_current {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let label_style = if is_current {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Layout: label + input
    let label = field.label();
    let placeholder = field.placeholder();

    let display_value = if is_current {
        input_buffer.to_string()
    } else if value.is_empty() {
        String::new()
    } else {
        value.to_string()
    };

    // Show placeholder if empty and current
    let show_placeholder = is_current && display_value.is_empty();

    let spans = vec![
        Span::styled(format!("{}: ", label), label_style),
        if show_placeholder {
            Span::styled(placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(&display_value, Style::default().fg(Color::White))
        },
        if is_current {
            Span::styled("█", Style::default().fg(Color::White))
        } else {
            Span::raw("")
        },
    ];

    let line = Line::from(spans);
    let para = Paragraph::new(line);
    para.render(inner, buf);
}

/// Renders form help text.
fn render_form_help(area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }

    let help = "Tab: next field | Shift+Tab: prev | Enter: run | Esc: cancel";
    let style = Style::default().fg(Color::DarkGray);
    let para = Paragraph::new(help).style(style);
    para.render(area, buf);
}

/// Renders a confirmation dialog.
pub fn render_confirm_dialog(
    message: &str,
    yes_label: &str,
    no_label: &str,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height < 5 || area.width < 30 {
        return;
    }

    let lines = vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("[Enter] {} ", yes_label),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!("  [Esc] {}", no_label),
                Style::default().fg(Color::Red),
            ),
        ]),
    ];

    let para = Paragraph::new(lines);
    para.render(area, buf);
}

/// Renders an input field with label.
pub fn render_input_field(
    label: &str,
    value: &str,
    is_focused: bool,
    is_password: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 {
        return;
    }

    let display_value = if is_password {
        "*".repeat(value.len())
    } else {
        value.to_string()
    };

    let label_style = if is_focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let value_style = if is_focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };

    let cursor = if is_focused { "█" } else { "" };

    let spans = vec![
        Span::styled(format!("{}: ", label), label_style),
        Span::styled(display_value, value_style),
        Span::styled(cursor, Style::default().fg(Color::White)),
    ];

    let line = Line::from(spans);
    let para = Paragraph::new(line);
    para.render(area, buf);
}

/// Renders the host selection list.
pub fn render_host_selection_list(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    let hosts = selector.available_hosts();
    let selected_idx = selector.host_selection_index();
    let scroll_offset = selector.host_scroll_offset();

    // Calculate visible range
    let max_visible = area.height.saturating_sub(2) as usize;
    let visible_hosts: Vec<_> = hosts
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
        .collect();

    // Build lines for each visible host
    let mut lines: Vec<Line> = Vec::with_capacity(max_visible);

    for (idx, host_display) in visible_hosts {
        let is_selected = idx == selected_idx;

        let prefix = if is_selected { "▶ " } else { "  " };

        let display_text = if host_display.host_id.is_none() {
            format!("{}{} (local)", prefix, host_display.display_name)
        } else {
            let cred_indicator = if host_display.has_credentials {
                "🔑"
            } else {
                "🔒"
            };
            format!(
                "{}{} {} [{}]",
                prefix, host_display.display_name, cred_indicator, host_display.hostname
            )
        };

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::styled(display_text, style));
    }

    // Add scroll indicators if needed
    if scroll_offset > 0 {
        lines.insert(
            0,
            Line::styled("  ▲ more above", Style::default().fg(Color::DarkGray)),
        );
        if !lines.is_empty() && lines.len() > 1 {
            lines.remove(1);
        }
    }

    if scroll_offset + max_visible < hosts.len() {
        if let Some(last) = lines.last_mut() {
            *last = Line::styled("  ▼ more below", Style::default().fg(Color::DarkGray));
        }
    }

    let para = Paragraph::new(lines);
    para.render(area, buf);
}

/// Renders the host credentials form.
pub fn render_host_credentials_form(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height < 8 || area.width < 30 {
        return;
    }

    // Layout: username + password + save checkbox + help
    let chunks = Layout::vertical([
        Constraint::Length(3), // Username
        Constraint::Length(3), // Password
        Constraint::Length(2), // Save checkbox
        Constraint::Min(2),    // Help
    ])
    .split(area);

    let current_field = selector.cred_field();
    let (username, password, save) = selector.get_entered_credentials();

    // Render username field
    render_credential_field(
        "Username",
        &username,
        HostCredentialField::Username == current_field,
        false,
        chunks[0],
        buf,
    );

    // Render password field
    render_credential_field(
        "Password",
        &password,
        HostCredentialField::Password == current_field,
        true,
        chunks[1],
        buf,
    );

    // Render save checkbox
    render_save_checkbox(
        save,
        HostCredentialField::SaveCheckbox == current_field,
        chunks[2],
        buf,
    );

    // Render help text
    render_credential_help(chunks[3], buf);
}

/// Renders a credential input field.
fn render_credential_field(
    label: &str,
    value: &str,
    is_focused: bool,
    is_password: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height < 3 {
        return;
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    let display_value = if is_password && !value.is_empty() {
        "*".repeat(value.len())
    } else {
        value.to_string()
    };

    let label_style = if is_focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let cursor = if is_focused { "█" } else { "" };

    let spans = vec![
        Span::styled(format!("{}: ", label), label_style),
        Span::styled(display_value, Style::default().fg(Color::White)),
        Span::styled(cursor, Style::default().fg(Color::White)),
    ];

    let line = Line::from(spans);
    let para = Paragraph::new(line);
    para.render(inner, buf);
}

/// Renders the save credentials checkbox.
fn render_save_checkbox(checked: bool, is_focused: bool, area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }

    let checkbox = if checked { "[✓]" } else { "[ ]" };

    let style = if is_focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let line = Line::from(vec![Span::styled(
        format!("{} Save credentials", checkbox),
        style,
    )]);

    let para = Paragraph::new(line);
    para.render(area, buf);
}

/// Renders credential form help text.
fn render_credential_help(area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }

    let help = "Tab: next | Shift+Tab: prev | Space: toggle | Enter: submit | Esc: cancel";
    let style = Style::default().fg(Color::DarkGray);
    let para = Paragraph::new(help).style(style);
    para.render(area, buf);
}
