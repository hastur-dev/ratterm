//! Docker container creation workflow rendering.
//!
//! Provides UI rendering for the multi-step container creation workflow:
//! - Docker Hub search form
//! - Search results list
//! - Image download progress
//! - Volume mount configuration
//! - Startup command input
//! - Creation confirmation

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};

use super::selector::DockerManagerSelector;
use super::types::DockerManagerMode;
use super::widget_create_volumes::{
    render_create_confirm, render_creation_error, render_startup_command, render_volume_confirm,
    render_volume_container_path, render_volume_host_path,
};

/// Creates a download status line for display in creation workflow screens.
pub(super) fn download_status_line(selector: &DockerManagerSelector) -> Option<Line<'static>> {
    let state = selector.creation_state();
    let image = state.selected_image.as_deref().unwrap_or("image");

    if selector.is_downloading() {
        Some(Line::from(vec![
            Span::styled("⟳ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("Downloading '{}' in background...", image),
                Style::default().fg(Color::Yellow),
            ),
        ]))
    } else if state.image_exists {
        Some(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("Image '{}' ready", image),
                Style::default().fg(Color::Green),
            ),
        ]))
    } else {
        None
    }
}

/// Renders the creation mode UI.
pub fn render_creation_mode(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    // Clear background
    Clear.render(area, buf);

    match selector.mode() {
        DockerManagerMode::SearchingHub => render_search_hub(selector, area, buf),
        DockerManagerMode::SearchResults => render_search_results(selector, area, buf),
        DockerManagerMode::CheckingImage => render_checking_image(selector, area, buf),
        DockerManagerMode::DownloadingImage => render_downloading_image(selector, area, buf),
        DockerManagerMode::VolumeMountHostPath => render_volume_host_path(selector, area, buf),
        DockerManagerMode::VolumeMountContainerPath => {
            render_volume_container_path(selector, area, buf);
        }
        DockerManagerMode::VolumeMountConfirm => render_volume_confirm(selector, area, buf),
        DockerManagerMode::StartupCommand => render_startup_command(selector, area, buf),
        DockerManagerMode::CreateConfirm => render_create_confirm(selector, area, buf),
        DockerManagerMode::CreationError => render_creation_error(selector, area, buf),
        _ => {}
    }
}

/// Renders the Docker Hub search form.
fn render_search_hub(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    let block = Block::default()
        .title(" Search Docker Hub ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let state = selector.creation_state();

    let lines = vec![
        Line::from(""),
        Line::from("Enter image name to search Docker Hub:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(
                &state.search_term,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("_", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Examples: nginx, ubuntu, postgres, redis",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter] Search    [Esc] Cancel",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let para = Paragraph::new(lines);
    para.render(inner, buf);
}

/// Renders the search results list.
fn render_search_results(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    let state = selector.creation_state();

    let title = format!(" Search Results ({}) ", state.search_results.len());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let chunks = Layout::vertical([
        Constraint::Min(5),    // Results list
        Constraint::Length(2), // Help text
    ])
    .split(inner);

    // Render results
    let list_height = chunks[0].height as usize;
    let selected = state.selected_result_idx;

    for (i, result) in state.search_results.iter().enumerate() {
        if i >= list_height {
            break;
        }

        let y = chunks[0].y + i as u16;
        let is_selected = i == selected;

        // Build line with stars and official badge
        let official_badge = if result.official { "[OK] " } else { "" };
        let stars = format!("{}*{}", official_badge, result.stars);
        let name = &result.name;

        // Truncate description
        let max_desc_len = (chunks[0].width as usize).saturating_sub(name.len() + stars.len() + 10);
        let desc = if result.description.len() > max_desc_len {
            format!(
                "{}...",
                &result.description[..max_desc_len.saturating_sub(3)]
            )
        } else {
            result.description.clone()
        };

        let prefix = if is_selected { ">" } else { " " };
        let line = format!("{} {} - {} ({})", prefix, name, desc, stars);

        let style = if is_selected {
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        // Render line
        let width = chunks[0].width as usize;
        for (j, c) in line.chars().enumerate() {
            if j >= width {
                break;
            }
            if let Some(cell) = buf.cell_mut((chunks[0].x + j as u16, y)) {
                cell.set_char(c).set_style(style);
            }
        }

        // Fill rest if selected
        if is_selected {
            for j in line.len()..width {
                if let Some(cell) = buf.cell_mut((chunks[0].x + j as u16, y)) {
                    cell.set_char(' ').set_style(style);
                }
            }
        }
    }

    // Help text
    let help = Line::from(Span::styled(
        "j/k:navigate  Enter:select  Esc:back",
        Style::default().fg(Color::DarkGray),
    ));
    let help_para = Paragraph::new(help);
    help_para.render(chunks[1], buf);
}

/// Renders the image checking status.
fn render_checking_image(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    let state = selector.creation_state();
    let image = state.selected_image.as_deref().unwrap_or("image");

    let block = Block::default()
        .title(" Checking Image ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let lines = vec![
        Line::from(""),
        Line::from(format!("Checking if '{}' exists on host...", image)),
        Line::from(""),
        Line::from("Please wait..."),
    ];

    let style = Style::default().fg(Color::Yellow);
    let para = Paragraph::new(lines).style(style);
    para.render(inner, buf);
}

/// Renders the image download progress.
fn render_downloading_image(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    let state = selector.creation_state();
    let image = state.selected_image.as_deref().unwrap_or("image");

    let block = Block::default()
        .title(" Downloading Image ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    block.render(area, buf);

    let lines = vec![
        Line::from(""),
        Line::from(format!("Downloading '{}'...", image)),
        Line::from(""),
        Line::from("This may take a few minutes for large images."),
        Line::from(""),
        Line::from(Span::styled(
            "[Esc] Cancel (download continues in background)",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let style = Style::default().fg(Color::Green);
    let para = Paragraph::new(lines).style(style);
    para.render(inner, buf);
}
