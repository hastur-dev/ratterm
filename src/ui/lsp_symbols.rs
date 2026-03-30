//! LSP symbols panel widget.
//!
//! Renders document and workspace symbols with hierarchy and icons.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::lsp::symbols::{
    flatten_symbols, DocumentSymbolResult, SymbolInfoResult, SymbolKind,
};

/// Widget for displaying document symbols (outline).
pub struct LspDocumentSymbolsWidget<'a> {
    /// Hierarchical symbol list.
    symbols: &'a [DocumentSymbolResult],
    /// Currently selected flat index.
    selected: usize,
    /// Scroll offset for the visible area.
    scroll: usize,
    /// Title shown in the border.
    title: &'a str,
}

impl<'a> LspDocumentSymbolsWidget<'a> {
    /// Creates a new document symbols widget.
    #[must_use]
    pub fn new(
        symbols: &'a [DocumentSymbolResult],
        selected: usize,
        scroll: usize,
    ) -> Self {
        Self {
            symbols,
            selected,
            scroll,
            title: " Document Symbols ",
        }
    }

    /// Sets a custom title for the widget border.
    #[must_use]
    pub fn with_title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Returns the display color for a symbol kind.
    #[must_use]
    pub fn kind_color(kind: SymbolKind) -> Color {
        match kind {
            SymbolKind::Function | SymbolKind::Method => Color::Yellow,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface => {
                Color::Cyan
            }
            SymbolKind::Enum | SymbolKind::EnumMember => Color::Green,
            SymbolKind::Variable | SymbolKind::Field | SymbolKind::Property => {
                Color::White
            }
            SymbolKind::Constant => Color::Magenta,
            SymbolKind::Module | SymbolKind::Namespace => Color::Blue,
            _ => Color::Gray,
        }
    }
}

impl<'a> Widget for LspDocumentSymbolsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let flat = flatten_symbols(self.symbols, 0);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!("{} ({}) ", self.title, flat.len()));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines: Vec<Line> = flat
            .iter()
            .enumerate()
            .map(|(idx, (depth, symbol))| {
                let is_selected = idx == self.selected;
                let indent = "  ".repeat(*depth + 1);
                let icon = symbol.kind.icon();
                let color = Self::kind_color(symbol.kind);

                let detail = symbol.detail.as_deref().unwrap_or("");
                let detail_str = if detail.is_empty() {
                    String::new()
                } else {
                    format!(" - {detail}")
                };

                let prefix = if is_selected { ">" } else { " " };
                let text = format!(
                    "{prefix}{indent}[{icon}] {}{detail_str}",
                    symbol.name
                );

                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(color)
                } else {
                    Style::default().fg(color)
                };

                Line::from(Span::styled(text, style))
            })
            .collect();

        let visible: Vec<Line> = lines
            .into_iter()
            .skip(self.scroll)
            .take(inner.height as usize)
            .collect();
        Paragraph::new(visible).render(inner, buf);
    }
}

/// Widget for displaying workspace symbols.
pub struct LspWorkspaceSymbolsWidget<'a> {
    /// Flat list of workspace symbols.
    symbols: &'a [SymbolInfoResult],
    /// Currently selected index.
    selected: usize,
    /// Scroll offset for the visible area.
    scroll: usize,
    /// Current search query.
    query: &'a str,
}

impl<'a> LspWorkspaceSymbolsWidget<'a> {
    /// Creates a new workspace symbols widget.
    #[must_use]
    pub fn new(
        symbols: &'a [SymbolInfoResult],
        selected: usize,
        scroll: usize,
        query: &'a str,
    ) -> Self {
        Self {
            symbols,
            selected,
            scroll,
            query,
        }
    }
}

impl<'a> Widget for LspWorkspaceSymbolsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        assert!(area.width > 0, "render area width must be positive");
        assert!(area.height > 0, "render area height must be positive");

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(format!(
                " Workspace Symbols ({}) - \"{}\" ",
                self.symbols.len(),
                self.query
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines: Vec<Line> = self
            .symbols
            .iter()
            .enumerate()
            .map(|(idx, symbol)| {
                let is_selected = idx == self.selected;
                let icon = symbol.kind.icon();
                let color = LspDocumentSymbolsWidget::kind_color(symbol.kind);

                let file_name = symbol
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                let container = symbol.container_name.as_deref().unwrap_or("");
                let container_str = if container.is_empty() {
                    String::new()
                } else {
                    format!(" ({container})")
                };

                let prefix = if is_selected { "> " } else { "  " };
                let text = format!(
                    "{prefix}[{icon}] {}{container_str}  {file_name}:{}",
                    symbol.name,
                    symbol.line + 1
                );

                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(color)
                } else {
                    Style::default().fg(color)
                };

                Line::from(Span::styled(text, style))
            })
            .collect();

        let visible: Vec<Line> = lines
            .into_iter()
            .skip(self.scroll)
            .take(inner.height as usize)
            .collect();
        Paragraph::new(visible).render(inner, buf);
    }
}
