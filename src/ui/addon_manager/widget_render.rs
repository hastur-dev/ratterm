//! Add-on Manager render helpers.
//!
//! Rendering functions for list and loading views.

use super::selector::AddonManagerSelector;
use super::types::{AddonListSection, MAX_DISPLAY_ADDONS};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

/// Renders the list mode view.
pub fn render_list_mode(
    selector: &AddonManagerSelector,
    area: Rect,
    buf: &mut Buffer,
    bg_color: Color,
) {
    // Layout: header (tabs), list, footer (tips)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Section tabs
            Constraint::Min(3),    // List
            Constraint::Length(2), // Footer tips
        ])
        .split(area);

    // Render section tabs
    render_section_tabs(selector, chunks[0], buf, bg_color);

    // Render addon list
    render_addon_list(selector, chunks[1], buf, bg_color);

    // Render footer tips
    render_footer_tips(selector, chunks[2], buf, bg_color);
}

/// Renders the section tab bar.
fn render_section_tabs(
    selector: &AddonManagerSelector,
    area: Rect,
    buf: &mut Buffer,
    bg_color: Color,
) {
    let current_section = selector.section();

    let available_style = if current_section == AddonListSection::Available {
        Style::default()
            .fg(Color::Cyan)
            .bg(bg_color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Gray).bg(bg_color)
    };

    let installed_style = if current_section == AddonListSection::Installed {
        Style::default()
            .fg(Color::Cyan)
            .bg(bg_color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Gray).bg(bg_color)
    };

    let available_count = selector.available_count();
    let installed_count = selector.installed_count();

    let tabs = Line::from(vec![
        Span::styled(format!(" [A]vailable ({}) ", available_count), available_style),
        Span::styled(" | ", Style::default().fg(Color::DarkGray).bg(bg_color)),
        Span::styled(format!(" [I]nstalled ({}) ", installed_count), installed_style),
    ]);

    Paragraph::new(tabs).render(area, buf);
}

/// Renders the addon list.
fn render_addon_list(
    selector: &AddonManagerSelector,
    area: Rect,
    buf: &mut Buffer,
    bg_color: Color,
) {
    let items = selector.current_items();

    if items.is_empty() {
        let empty_msg = match selector.section() {
            AddonListSection::Available => "No add-ons available. Press F5 to refresh.",
            AddonListSection::Installed => "No add-ons installed yet.",
        };

        let para = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray).bg(bg_color));
        para.render(area, buf);
        return;
    }

    let visible_items: Vec<Line> = selector
        .visible_items()
        .map(|(idx, addon_display)| {
            let is_selected = selector.is_selected(idx);

            let style = if is_selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White).bg(bg_color)
            };

            let indicator = addon_display.status_indicator();
            let name = addon_display.display_name();
            let summary = addon_display.summary();

            // Truncate summary to fit
            let max_summary_len = area.width.saturating_sub(20) as usize;
            let summary_truncated: String = summary.chars().take(max_summary_len).collect();

            Line::from(vec![
                Span::styled(format!(" {} ", indicator), style),
                Span::styled(format!("{:<15}", name), style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {}", summary_truncated), style),
            ])
        })
        .collect();

    let para = Paragraph::new(visible_items);
    para.render(area, buf);

    // Render scrollbar if needed
    if items.len() > MAX_DISPLAY_ADDONS {
        render_scrollbar(selector, area, buf);
    }
}

/// Renders a simple scrollbar indicator.
fn render_scrollbar(selector: &AddonManagerSelector, area: Rect, buf: &mut Buffer) {
    let total = selector.current_count();
    let visible = MAX_DISPLAY_ADDONS;
    let selected = selector.selected_index();

    if total <= visible {
        return;
    }

    let scroll_height = area.height.saturating_sub(1) as usize;
    let thumb_pos = (selected * scroll_height) / total;
    let thumb_pos = thumb_pos.min(scroll_height.saturating_sub(1));

    let x = area.right().saturating_sub(1);

    for (i, y) in (area.y..area.bottom()).enumerate() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            let symbol = if i == thumb_pos { "█" } else { "│" };
            cell.set_symbol(symbol);
            cell.set_fg(Color::DarkGray);
        }
    }
}

/// Renders footer tips.
fn render_footer_tips(
    selector: &AddonManagerSelector,
    area: Rect,
    buf: &mut Buffer,
    bg_color: Color,
) {
    let tips = match selector.section() {
        AddonListSection::Available => {
            "Enter: Install | Tab: Switch section | F5: Refresh | Esc: Close"
        }
        AddonListSection::Installed => {
            "Enter: Run | d: Uninstall | Tab: Switch section | Esc: Close"
        }
    };

    let para = Paragraph::new(tips)
        .style(Style::default().fg(Color::DarkGray).bg(bg_color));
    para.render(area, buf);
}

/// Renders the loading view.
pub fn render_loading_view(
    selector: &AddonManagerSelector,
    area: Rect,
    buf: &mut Buffer,
    bg_color: Color,
) {
    let message = if let Some(progress) = selector.install_progress() {
        format!(
            "{}\n\nInstalling: {}\nProgress: {}%",
            progress.phase.display(),
            progress.addon_id,
            progress.progress
        )
    } else {
        "Loading add-ons from GitHub...".to_string()
    };

    // Simple spinner animation based on frame count (static for now)
    let spinner = "⠋";

    let content = format!("{} {}", spinner, message);

    let para = Paragraph::new(content)
        .style(Style::default().fg(Color::Yellow).bg(bg_color));

    // Center vertically
    let y_offset = area.height / 2;
    let centered_area = Rect::new(
        area.x + 2,
        area.y + y_offset.saturating_sub(1),
        area.width.saturating_sub(4),
        3,
    );

    para.render(centered_area, buf);
}

/// Renders an error view with word wrapping.
pub fn render_error_view(message: &str, area: Rect, buf: &mut Buffer, bg_color: Color) {
    use ratatui::text::{Line, Span};

    // Create header
    let header = Line::from(vec![
        Span::styled("⚠ Error", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
    ]);

    // Word-wrap the error message to fit the available width
    let max_width = area.width.saturating_sub(4) as usize;
    let wrapped_lines = wrap_text(message, max_width);

    // Build content lines
    let mut lines: Vec<Line> = vec![header, Line::from("")];

    for line in wrapped_lines {
        lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(Color::White).bg(bg_color),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Esc] Close",
        Style::default().fg(Color::DarkGray).bg(bg_color),
    )));

    let para = Paragraph::new(lines)
        .style(Style::default().bg(bg_color));

    // Use the full inner area for error display
    let error_area = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );

    para.render(error_area, buf);
}

/// Wraps text to fit within a maximum width.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_display() {
        assert_eq!(MAX_DISPLAY_ADDONS, 12);
    }

    #[test]
    fn test_wrap_text() {
        let text = "This is a long error message that needs to be wrapped";
        let wrapped = wrap_text(text, 20);
        assert!(!wrapped.is_empty());
        for line in &wrapped {
            assert!(line.len() <= 20 || !line.contains(' '));
        }
    }

    #[test]
    fn test_wrap_text_short() {
        let text = "Short";
        let wrapped = wrap_text(text, 20);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], "Short");
    }
}
