//! Docker Manager widget rendering helpers.
//!
//! These helper functions provide alternative rendering approaches
//! and can be used for customized Docker manager UIs.

#![allow(dead_code)]

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::selector::DockerManagerSelector;
use super::types::{DockerItemDisplay, DockerListSection};

/// Renders a single list item.
pub fn render_list_item(
    item: &DockerItemDisplay,
    index: usize,
    selected_index: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    let is_selected = index == selected_index;

    // Build display text
    let prefix = if is_selected { "▶" } else { " " };
    let type_label = item.item_type().label();
    let name = item.display();

    // Get status/info based on item type
    let info = match item {
        DockerItemDisplay::Container(c) => {
            if c.ports.is_empty() {
                c.image.clone()
            } else {
                format!("{} [{}]", c.image, c.ports.join(", "))
            }
        }
        DockerItemDisplay::Image(i) => i.size.clone(),
    };

    // Calculate available width
    let max_name_width = (area.width as usize).saturating_sub(20).max(10);
    let display_name = if name.len() > max_name_width {
        format!("{}...", &name[..max_name_width - 3])
    } else {
        name
    };

    let max_info_width = (area.width as usize).saturating_sub(max_name_width + 10);
    let display_info = if info.len() > max_info_width {
        format!("{}...", &info[..max_info_width.saturating_sub(3)])
    } else {
        info
    };

    // Build styled spans
    let base_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let type_style = if is_selected {
        base_style
    } else {
        match item {
            DockerItemDisplay::Container(c) if c.is_running() => Style::default().fg(Color::Green),
            DockerItemDisplay::Container(_) => Style::default().fg(Color::Yellow),
            DockerItemDisplay::Image(_) => Style::default().fg(Color::Cyan),
        }
    };

    let info_style = if is_selected {
        base_style
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let spans = vec![
        Span::styled(format!("{} ", prefix), base_style),
        Span::styled(format!("{} ", type_label), type_style),
        Span::styled(display_name, base_style),
        Span::styled(format!("  {}", display_info), info_style),
    ];

    let line = Line::from(spans);

    // Render the line
    let y = area.y;
    let x = area.x;
    let width = area.width;

    // Clear line first if selected
    if is_selected {
        for i in 0..width {
            if let Some(cell) = buf.cell_mut((x + i, y)) {
                cell.set_char(' ').set_style(base_style);
            }
        }
    }

    // Render spans
    Paragraph::new(line).render(Rect::new(x, y, width, 1), buf);
}

/// Renders a section header.
pub fn render_section_header(
    section: DockerListSection,
    count: usize,
    is_current: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 {
        return;
    }

    let label = format!("{} ({})", section.title(), count);
    let style = if is_current {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Gray)
    };

    let para = Paragraph::new(label).style(style);
    para.render(area, buf);
}

/// Renders status/error message.
pub fn render_status_message(selector: &DockerManagerSelector, area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }

    if let Some(error) = selector.error() {
        let style = Style::default().fg(Color::Red);
        let para = Paragraph::new(error).style(style);
        para.render(area, buf);
    } else if let Some(status) = selector.status() {
        let style = Style::default().fg(Color::Yellow);
        let para = Paragraph::new(status).style(style);
        para.render(area, buf);
    }
}

/// Renders quick connect assignments indicator.
pub fn render_quick_connect_hint(slot: Option<usize>, area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width < 3 {
        return;
    }

    if let Some(slot) = slot {
        let label = format!("[{}]", slot + 1);
        let style = Style::default().fg(Color::Magenta);
        let para = Paragraph::new(label).style(style);
        para.render(area, buf);
    }
}

/// Renders empty state message.
pub fn render_empty_message(message: &str, area: Rect, buf: &mut Buffer) {
    if area.height < 3 {
        return;
    }

    let lines = vec![Line::from(""), Line::from(message), Line::from("")];

    let style = Style::default().fg(Color::Gray);
    let para = Paragraph::new(lines).style(style);
    para.render(area, buf);
}
