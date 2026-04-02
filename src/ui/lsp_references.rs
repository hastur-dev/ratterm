//! LSP references panel widget.
//!
//! Renders find-references results grouped by file.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::lsp::references::ReferenceGroup;

/// Widget for displaying find-references results.
pub struct LspReferencesWidget<'a> {
    /// Reference groups (one per file).
    groups: &'a [ReferenceGroup],
    /// Currently selected flat index.
    selected: usize,
    /// Scroll offset for the visible area.
    scroll: usize,
}

impl<'a> LspReferencesWidget<'a> {
    /// Creates a new references widget.
    #[must_use]
    pub fn new(groups: &'a [ReferenceGroup], selected: usize, scroll: usize) -> Self {
        Self {
            groups,
            selected,
            scroll,
        }
    }

    /// Returns total number of selectable items across all groups.
    #[must_use]
    pub fn total_items(groups: &[ReferenceGroup]) -> usize {
        groups.iter().map(|g| g.locations.len()).sum()
    }

    /// Maps a flat index to `(group_idx, location_idx)` within that group.
    #[must_use]
    pub fn index_to_group_location(
        groups: &[ReferenceGroup],
        flat_idx: usize,
    ) -> Option<(usize, usize)> {
        let mut remaining = flat_idx;
        for (gi, group) in groups.iter().enumerate() {
            if remaining < group.locations.len() {
                return Some((gi, remaining));
            }
            remaining -= group.locations.len();
        }
        None
    }
}

impl<'a> Widget for LspReferencesWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(format!(" References ({}) ", Self::total_items(self.groups)));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line> = Vec::new();
        let mut flat_idx = 0;

        for group in self.groups {
            // File header
            let file_name = group
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            lines.push(Line::from(Span::styled(
                format!("  {} ({} references)", file_name, group.locations.len()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));

            for loc in &group.locations {
                let is_selected = flat_idx == self.selected;
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let prefix = if is_selected { "> " } else { "  " };
                let text = format!("{prefix}  L{}: {}", loc.line + 1, loc.preview);
                lines.push(Line::from(Span::styled(text, style)));
                flat_idx += 1;
            }
        }

        // Apply scroll and take only visible lines
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(self.scroll)
            .take(inner.height as usize)
            .collect();

        Paragraph::new(visible_lines).render(inner, buf);
    }
}
