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
use super::types::RunOptionsField;

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
