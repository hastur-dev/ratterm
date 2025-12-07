//! Popup dialog widgets.
//!
//! Provides modal dialogs for search, file creation, etc.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Type of popup dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// Search for text in file.
    SearchInFile,
    /// Search for text in all files.
    SearchInFiles,
    /// Search for directories.
    SearchDirectories,
    /// Search for file names.
    SearchFiles,
    /// Create new file.
    CreateFile,
    /// Create new folder.
    CreateFolder,
    /// Confirm save before exit.
    ConfirmSaveBeforeExit,
}

impl PopupKind {
    /// Returns the title for this popup kind.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::SearchInFile => "Search in File",
            Self::SearchInFiles => "Search in All Files",
            Self::SearchDirectories => "Search for Directories",
            Self::SearchFiles => "Search for Files",
            Self::CreateFile => "Create New File",
            Self::CreateFolder => "Create New Folder",
            Self::ConfirmSaveBeforeExit => "Unsaved Changes",
        }
    }

    /// Returns the prompt for this popup kind.
    #[must_use]
    pub fn prompt(&self) -> &'static str {
        match self {
            Self::SearchInFile => "Find: ",
            Self::SearchInFiles => "Search: ",
            Self::SearchDirectories => "Directory: ",
            Self::SearchFiles => "File: ",
            Self::CreateFile => "Name: ",
            Self::CreateFolder => "Folder: ",
            Self::ConfirmSaveBeforeExit => "Save? (Y)es / (N)o / (C)ancel: ",
        }
    }

    /// Returns true if this popup is a confirmation dialog.
    #[must_use]
    pub fn is_confirmation(&self) -> bool {
        matches!(self, Self::ConfirmSaveBeforeExit)
    }
}

/// Popup dialog state.
pub struct Popup {
    /// Kind of popup.
    kind: PopupKind,
    /// Current input text.
    input: String,
    /// Cursor position in input.
    cursor: usize,
    /// Suggested suffix (for autocomplete).
    suggestion: Option<String>,
    /// Error message.
    error: Option<String>,
    /// Is popup visible.
    visible: bool,
    /// Results list (for search).
    results: Vec<String>,
    /// Selected result index.
    selected_result: usize,
}

impl Popup {
    /// Creates a new popup.
    #[must_use]
    pub fn new(kind: PopupKind) -> Self {
        Self {
            kind,
            input: String::new(),
            cursor: 0,
            suggestion: None,
            error: None,
            visible: false,
            results: Vec::new(),
            selected_result: 0,
        }
    }

    /// Returns the popup kind.
    #[must_use]
    pub fn kind(&self) -> PopupKind {
        self.kind
    }

    /// Sets the popup kind.
    pub fn set_kind(&mut self, kind: PopupKind) {
        self.kind = kind;
        self.clear();
    }

    /// Returns the input text.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Returns true if the popup is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shows the popup.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the popup.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Clears the popup state.
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.error = None;
        self.results.clear();
        self.selected_result = 0;
    }

    /// Sets a suggestion for autocomplete.
    pub fn set_suggestion(&mut self, suggestion: Option<String>) {
        self.suggestion = suggestion;
    }

    /// Sets an error message.
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    /// Sets the results list.
    pub fn set_results(&mut self, results: Vec<String>) {
        self.results = results;
        self.selected_result = 0;
    }

    /// Returns the results.
    #[must_use]
    pub fn results(&self) -> &[String] {
        &self.results
    }

    /// Returns the selected result.
    #[must_use]
    pub fn selected_result(&self) -> Option<&String> {
        self.results.get(self.selected_result)
    }

    /// Inserts a character at cursor.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
        self.error = None;
    }

    /// Deletes character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
            self.error = None;
        }
    }

    /// Deletes character at cursor (delete).
    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.error = None;
        }
    }

    /// Moves cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Moves cursor right.
    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    /// Moves cursor to start.
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Moves cursor to end.
    pub fn move_to_end(&mut self) {
        self.cursor = self.input.len();
    }

    /// Moves result selection up.
    pub fn result_up(&mut self) {
        if self.selected_result > 0 {
            self.selected_result -= 1;
        }
    }

    /// Moves result selection down.
    pub fn result_down(&mut self) {
        if self.selected_result < self.results.len().saturating_sub(1) {
            self.selected_result += 1;
        }
    }

    /// Accepts the suggestion and appends it to input.
    pub fn accept_suggestion(&mut self) {
        if let Some(suggestion) = self.suggestion.take() {
            self.input.push_str(&suggestion);
            self.cursor = self.input.len();
        }
    }

    /// Returns the final input (with suggestion if for create dialogs).
    #[must_use]
    pub fn final_input(&self) -> String {
        match self.kind {
            PopupKind::CreateFile | PopupKind::CreateFolder => {
                if let Some(ref suggestion) = self.suggestion {
                    if !self.input.contains('.') {
                        return format!("{}{}", self.input, suggestion);
                    }
                }
                self.input.clone()
            }
            _ => self.input.clone(),
        }
    }
}

/// Popup widget for rendering.
pub struct PopupWidget<'a> {
    popup: &'a Popup,
}

impl<'a> PopupWidget<'a> {
    /// Creates a new popup widget.
    #[must_use]
    pub fn new(popup: &'a Popup) -> Self {
        Self { popup }
    }

    /// Calculates the popup area.
    fn popup_area(&self, area: Rect) -> Rect {
        let width = (area.width * 60 / 100).min(60).max(30);
        let height = if self.popup.results.is_empty() { 5 } else { 12 };

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl<'a> Widget for PopupWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.popup.visible {
            return;
        }

        let popup_area = self.popup_area(area);

        // Clear background
        Clear.render(popup_area, buf);

        // Draw border
        let block = Block::default()
            .title(self.popup.kind.title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout: prompt + input, then results
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Prompt + input
                Constraint::Length(1), // Error or space
                Constraint::Min(0),    // Results
            ])
            .split(inner);

        // Render prompt and input
        let prompt = self.popup.kind.prompt();
        let input_display = if let Some(ref suggestion) = self.popup.suggestion {
            let input_style = Style::default().fg(Color::White);
            let suggestion_style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);

            Line::from(vec![
                Span::raw(prompt),
                Span::styled(&self.popup.input, input_style),
                Span::styled(suggestion, suggestion_style),
            ])
        } else {
            Line::from(vec![
                Span::raw(prompt),
                Span::styled(&self.popup.input, Style::default().fg(Color::White)),
            ])
        };

        Paragraph::new(input_display).render(chunks[0], buf);

        // Render cursor
        let cursor_x = chunks[0].x + prompt.len() as u16 + self.popup.cursor as u16;
        if cursor_x < chunks[0].right() && chunks[0].y < buf.area.bottom() {
            if let Some(cell) = buf.cell_mut((cursor_x, chunks[0].y)) {
                cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        }

        // Render error if any
        if let Some(ref error) = self.popup.error {
            let error_para = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red));
            error_para.render(chunks[1], buf);
        }

        // Render results
        if !self.popup.results.is_empty() {
            let visible_results: Vec<Line> = self
                .popup
                .results
                .iter()
                .enumerate()
                .take(chunks[2].height as usize)
                .map(|(i, result)| {
                    let style = if i == self.popup.selected_result {
                        Style::default().bg(Color::Blue).fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    Line::styled(result.as_str(), style)
                })
                .collect();

            let results_para = Paragraph::new(visible_results);
            results_para.render(chunks[2], buf);
        }
    }
}
