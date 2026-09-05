//! Rendering the volume, startup-command and confirmation steps of the
//! container-creation workflow.
//!
//! Split out of `widget_create.rs`, which had grown past this project's
//! file-size limit.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

use super::selector::DockerManagerSelector;
use super::widget_create::download_status_line;

pub(super) fn render_volume_host_path(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let state = selector.creation_state();

    let block = Block::default()
        .title(" Volume Mount - Host Path ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let mut lines = Vec::new();

    // Show download status at the top
    if let Some(status_line) = download_status_line(selector) {
        lines.push(status_line);
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    // Show existing mounts if any
    if !state.volume_mounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "Configured volume mounts:",
            Style::default().fg(Color::DarkGray),
        )));
        for mount in &state.volume_mounts {
            lines.push(Line::from(format!(
                "  {}:{}",
                mount.host_path, mount.container_path
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(
        "Enter host path to mount (or leave empty to skip):",
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("> "),
        Span::styled(
            &state.current_host_path,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press 'f' to open file browser for directory selection",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Enter] Confirm    [f] Browse    [Esc] Skip volumes",
        Style::default().fg(Color::Cyan),
    )));

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders the volume mount container path input.
pub(super) fn render_volume_container_path(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let state = selector.creation_state();

    let block = Block::default()
        .title(" Volume Mount - Container Path ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let mut lines = Vec::new();

    // Show download status at the top
    if let Some(status_line) = download_status_line(selector) {
        lines.push(status_line);
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(format!(
        "Host path: {}",
        state.current_host_path
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Enter container path (e.g., /app/data):"));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("> "),
        Span::styled(
            &state.current_container_path,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Enter] Confirm    [Esc] Back",
        Style::default().fg(Color::Cyan),
    )));

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders the volume mount confirmation.
pub(super) fn render_volume_confirm(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let state = selector.creation_state();

    let block = Block::default()
        .title(" Volume Mounts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let mut lines = Vec::new();

    // Show download status at the top
    if let Some(status_line) = download_status_line(selector) {
        lines.push(status_line);
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Configured volume mounts:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if state.volume_mounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for mount in &state.volume_mounts {
            lines.push(Line::from(format!(
                "  {}:{}",
                mount.host_path, mount.container_path
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Add another volume mount?"));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Y] Yes    [N/Enter] No, continue    [Esc] Skip",
        Style::default().fg(Color::Cyan),
    )));

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders the startup command input.
pub(super) fn render_startup_command(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let state = selector.creation_state();

    let block = Block::default()
        .title(" Startup Command ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let mut lines = Vec::new();

    // Show download status at the top
    if let Some(status_line) = download_status_line(selector) {
        lines.push(status_line);
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from("Enter additional startup command (optional):"));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("> "),
        Span::styled(
            &state.startup_command,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Example: bash, sh, /bin/bash, python app.py",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Enter] Confirm    [Esc] Back",
        Style::default().fg(Color::Cyan),
    )));

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders the creation confirmation screen.
pub(super) fn render_create_confirm(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let is_downloading = selector.is_downloading();
    let is_ready = selector.is_image_ready();

    // Use different border color based on readiness
    let border_color = if is_downloading {
        Color::Yellow
    } else {
        Color::Green
    };

    let block = Block::default()
        .title(" Confirm Container Creation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    // Build preview of the docker run command
    let state = selector.creation_state();
    let image = state.selected_image.as_deref().unwrap_or("image");

    let mut lines = Vec::new();

    // Show download status prominently at top
    if is_downloading {
        lines.push(Line::from(vec![
            Span::styled("⟳ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("Downloading '{}' - please wait...", image),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    } else if is_ready {
        lines.push(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("Image '{}' ready", image),
                Style::default().fg(Color::Green),
            ),
        ]));
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Docker run command preview:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Build command preview
    let mut cmd_parts = vec!["docker run -it --rm".to_string()];

    for mount in &state.volume_mounts {
        cmd_parts.push(format!("-v {}:{}", mount.host_path, mount.container_path));
    }

    cmd_parts.push(image.to_string());

    if !state.startup_command.is_empty() {
        cmd_parts.push(state.startup_command.clone());
    }

    let cmd = cmd_parts.join(" ");

    // Wrap command if too long
    let max_width = (inner.width as usize).saturating_sub(4);
    for chunk in cmd.as_bytes().chunks(max_width.max(40)) {
        if let Ok(s) = std::str::from_utf8(chunk) {
            lines.push(Line::from(Span::styled(
                format!("  {}", s),
                Style::default().fg(Color::Cyan),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Show appropriate help text based on download status
    if is_downloading {
        lines.push(Line::from(Span::styled(
            "Waiting for download to complete...    [Esc] Back",
            Style::default().fg(Color::Yellow),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "[Enter] Create Container    [Esc] Back",
            Style::default().fg(Color::Green),
        )));
    }

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders creation error message.
pub(super) fn render_creation_error(
    selector: &DockerManagerSelector,
    area: Rect,
    buf: &mut Buffer,
) {
    let state = selector.creation_state();

    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let error_msg = state.error.as_deref().unwrap_or("Unknown error");

    let mut lines = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Container creation failed:",
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Wrap error message
    let max_width = (inner.width as usize).saturating_sub(4);
    for chunk in error_msg.as_bytes().chunks(max_width.max(40)) {
        if let Ok(s) = std::str::from_utf8(chunk) {
            lines.push(Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        }
    }

    lines.push(Line::from(""));

    if state.suggest_log_file {
        lines.push(Line::from(Span::styled(
            "See ~/.ratterm/ratterm.log for details",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "[Enter/Esc] Dismiss",
        Style::default().fg(Color::Cyan),
    )));

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}
