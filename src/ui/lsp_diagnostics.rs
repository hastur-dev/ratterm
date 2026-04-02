//! LSP diagnostics panel widget.
//!
//! Renders diagnostics grouped by file with severity indicators.

use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::lsp::diagnostics::{DiagnosticInfo, DiagnosticSeverity};

/// Widget for displaying diagnostics grouped by file.
pub struct LspDiagnosticsWidget<'a> {
    /// Diagnostics keyed by file path.
    diagnostics: &'a HashMap<PathBuf, Vec<DiagnosticInfo>>,
    /// Currently selected flat index.
    selected: usize,
    /// Scroll offset for the visible area.
    scroll: usize,
}

impl<'a> LspDiagnosticsWidget<'a> {
    /// Creates a new diagnostics widget.
    #[must_use]
    pub fn new(
        diagnostics: &'a HashMap<PathBuf, Vec<DiagnosticInfo>>,
        selected: usize,
        scroll: usize,
    ) -> Self {
        Self {
            diagnostics,
            selected,
            scroll,
        }
    }

    /// Total count of diagnostics across all files.
    #[must_use]
    pub fn total_items(diagnostics: &HashMap<PathBuf, Vec<DiagnosticInfo>>) -> usize {
        diagnostics.values().map(Vec::len).sum()
    }

    /// Returns the display color for a diagnostic severity.
    #[must_use]
    pub fn severity_color(severity: DiagnosticSeverity) -> Color {
        match severity {
            DiagnosticSeverity::Error => Color::Red,
            DiagnosticSeverity::Warning => Color::Yellow,
            DiagnosticSeverity::Information => Color::Blue,
            DiagnosticSeverity::Hint => Color::Green,
        }
    }
}

impl<'a> Widget for LspDiagnosticsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let total = Self::total_items(self.diagnostics);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(format!(" Diagnostics ({total}) "));

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines: Vec<Line> = Vec::new();
        let mut flat_idx = 0;

        // Sort files for consistent ordering
        let mut sorted_files: Vec<_> = self.diagnostics.iter().collect();
        sorted_files.sort_by_key(|(path, _)| (*path).clone());

        for (path, diags) in &sorted_files {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            lines.push(Line::from(Span::styled(
                format!("  {file_name}"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )));

            for diag in *diags {
                let is_selected = flat_idx == self.selected;
                let sev_color = Self::severity_color(diag.severity);

                let bg = if is_selected {
                    Color::DarkGray
                } else {
                    Color::Reset
                };
                let prefix = if is_selected { "> " } else { "  " };

                let severity_label = diag.severity.label();
                let line_num = diag.range.start_line + 1;
                let msg = &diag.message;
                let text = format!("{prefix}  [{severity_label}] L{line_num}: {msg}");

                lines.push(Line::from(Span::styled(
                    text,
                    Style::default().fg(sev_color).bg(bg),
                )));
                flat_idx += 1;
            }
        }

        let visible: Vec<Line> = lines
            .into_iter()
            .skip(self.scroll)
            .take(inner.height as usize)
            .collect();
        Paragraph::new(visible).render(inner, buf);
    }
}

/// Widget for rendering inline diagnostic indicators in the editor gutter.
pub struct DiagnosticGutterWidget;

impl DiagnosticGutterWidget {
    /// Returns the gutter character and color for a line with diagnostics.
    ///
    /// Returns the indicator for the most severe diagnostic on that line,
    /// or `None` if no diagnostics touch the given line.
    #[must_use]
    pub fn gutter_indicator(diagnostics: &[DiagnosticInfo], line: u32) -> Option<(char, Color)> {
        let line_diags: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.range.start_line <= line && d.range.end_line >= line)
            .collect();

        if line_diags.is_empty() {
            return None;
        }

        // Return the most severe diagnostic's indicator
        line_diags
            .iter()
            .min_by_key(|d| match d.severity {
                DiagnosticSeverity::Error => 0,
                DiagnosticSeverity::Warning => 1,
                DiagnosticSeverity::Information => 2,
                DiagnosticSeverity::Hint => 3,
            })
            .map(|d| {
                (
                    d.severity.gutter_char(),
                    LspDiagnosticsWidget::severity_color(d.severity),
                )
            })
    }
}
