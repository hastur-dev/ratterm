//! Popup dialog widgets.
//!
//! Provides modal dialogs for search, file creation, command palette, etc.

mod command_palette;
mod extension_approval;
mod mode_switcher;
mod shell_selector;
mod theme_selector;

pub use command_palette::{Command, CommandPalette};
pub use extension_approval::{ExtensionApprovalPrompt, ExtensionApprovalWidget};
pub use mode_switcher::{ModeSwitcher, ModeSwitcherWidget};
pub use shell_selector::{
    ShellInstallPrompt, ShellInstallPromptWidget, ShellSelector, ShellSelectorItem,
    ShellSelectorWidget,
};
pub use theme_selector::{ThemeSelector, ThemeSelectorWidget};

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
    /// Command palette (VSCode-style Ctrl+Shift+P).
    CommandPalette,
    /// Mode switcher (Ctrl+Shift+Tab to cycle through editor modes).
    ModeSwitcher,
    /// Shell selector for choosing terminal shell.
    ShellSelector,
    /// Shell install prompt when selected shell is not available.
    ShellInstallPrompt,
    /// Theme selector for choosing color theme.
    ThemeSelector,
    /// Extension approval prompt for first-time extension load.
    ExtensionApproval,
    /// SSH Manager for managing SSH connections.
    SSHManager,
    /// SSH credential entry dialog.
    SSHCredentialPrompt,
    /// SSH storage mode selection (first-time setup).
    SSHStorageSetup,
    /// SSH master password entry.
    SSHMasterPassword,
    /// SSH network scan subnet entry.
    SSHSubnetEntry,
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
            Self::CommandPalette => "Command Palette",
            Self::ModeSwitcher => "Switch Editor Mode",
            Self::ShellSelector => "Select Shell",
            Self::ShellInstallPrompt => "Shell Not Available",
            Self::ThemeSelector => "Select Theme",
            Self::ExtensionApproval => "Extension Approval Required",
            Self::SSHManager => "SSH Manager",
            Self::SSHCredentialPrompt => "SSH Credentials",
            Self::SSHStorageSetup => "SSH Storage Setup",
            Self::SSHMasterPassword => "Master Password",
            Self::SSHSubnetEntry => "Network Scan",
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
            Self::CommandPalette => "> ",
            Self::ModeSwitcher => "",
            Self::ShellSelector => "",
            Self::ShellInstallPrompt => "",
            Self::ThemeSelector => "",
            Self::ExtensionApproval => "",
            Self::SSHManager => "",
            Self::SSHCredentialPrompt => "Username: ",
            Self::SSHStorageSetup => "",
            Self::SSHMasterPassword => "Password: ",
            Self::SSHSubnetEntry => "Subnet (e.g., 192.168.1.0/24): ",
        }
    }

    /// Returns true if this popup is a confirmation dialog.
    #[must_use]
    pub fn is_confirmation(&self) -> bool {
        matches!(self, Self::ConfirmSaveBeforeExit)
    }

    /// Returns true if this popup is a command palette.
    #[must_use]
    pub fn is_command_palette(&self) -> bool {
        matches!(self, Self::CommandPalette)
    }

    /// Returns true if this popup is a mode switcher.
    #[must_use]
    pub fn is_mode_switcher(&self) -> bool {
        matches!(self, Self::ModeSwitcher)
    }

    /// Returns true if this popup is a shell selector.
    #[must_use]
    pub fn is_shell_selector(&self) -> bool {
        matches!(self, Self::ShellSelector)
    }

    /// Returns true if this popup is a shell install prompt.
    #[must_use]
    pub fn is_shell_install_prompt(&self) -> bool {
        matches!(self, Self::ShellInstallPrompt)
    }

    /// Returns true if this popup is a theme selector.
    #[must_use]
    pub fn is_theme_selector(&self) -> bool {
        matches!(self, Self::ThemeSelector)
    }

    /// Returns true if this popup is an extension approval prompt.
    #[must_use]
    pub fn is_extension_approval(&self) -> bool {
        matches!(self, Self::ExtensionApproval)
    }

    /// Returns true if this popup is the SSH manager.
    #[must_use]
    pub fn is_ssh_manager(&self) -> bool {
        matches!(self, Self::SSHManager)
    }

    /// Returns true if this popup is an SSH credential prompt.
    #[must_use]
    pub fn is_ssh_credential_prompt(&self) -> bool {
        matches!(self, Self::SSHCredentialPrompt)
    }

    /// Returns true if this popup is the SSH storage setup.
    #[must_use]
    pub fn is_ssh_storage_setup(&self) -> bool {
        matches!(self, Self::SSHStorageSetup)
    }

    /// Returns true if this popup is the SSH master password entry.
    #[must_use]
    pub fn is_ssh_master_password(&self) -> bool {
        matches!(self, Self::SSHMasterPassword)
    }

    /// Returns true if this popup is the SSH subnet entry.
    #[must_use]
    pub fn is_ssh_subnet_entry(&self) -> bool {
        matches!(self, Self::SSHSubnetEntry)
    }

    /// Returns true if this is any SSH-related popup.
    #[must_use]
    pub fn is_ssh_popup(&self) -> bool {
        matches!(
            self,
            Self::SSHManager
                | Self::SSHCredentialPrompt
                | Self::SSHStorageSetup
                | Self::SSHMasterPassword
                | Self::SSHSubnetEntry
        )
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

        // Clear suggestion when user types a '.' (they're specifying their own extension)
        if c == '.' && self.suggestion.is_some() {
            self.suggestion = None;
        }
    }

    /// Deletes character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
            self.error = None;
        } else if self.suggestion.is_some() {
            self.suggestion = None;
        }
    }

    /// Deletes character at cursor (delete).
    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.error = None;
        } else if self.suggestion.is_some() {
            self.suggestion = None;
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
        let width = (area.width * 60 / 100).clamp(30, 60);
        let height = if self.popup.results.is_empty() { 5 } else { 12 };

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for PopupWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.popup.visible {
            return;
        }

        let popup_area = self.popup_area(area);
        let bg_color = Color::Rgb(30, 30, 30);

        // Clear background and fill with explicit color
        Clear.render(popup_area, buf);
        for y in popup_area.y..popup_area.bottom() {
            for x in popup_area.x..popup_area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }
        }

        // Draw border with explicit background
        let block = Block::default()
            .title(self.popup.kind.title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan).bg(bg_color));

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

        // Render prompt and input with explicit backgrounds
        let prompt = self.popup.kind.prompt();
        let input_display = if let Some(ref suggestion) = self.popup.suggestion {
            let input_style = Style::default().fg(Color::White).bg(bg_color);
            let suggestion_style = Style::default()
                .fg(Color::DarkGray)
                .bg(bg_color)
                .add_modifier(Modifier::ITALIC);

            Line::from(vec![
                Span::styled(prompt, Style::default().fg(Color::White).bg(bg_color)),
                Span::styled(&self.popup.input, input_style),
                Span::styled(suggestion, suggestion_style),
            ])
        } else {
            Line::from(vec![
                Span::styled(prompt, Style::default().fg(Color::White).bg(bg_color)),
                Span::styled(
                    &self.popup.input,
                    Style::default().fg(Color::White).bg(bg_color),
                ),
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
            let error_para =
                Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red).bg(bg_color));
            error_para.render(chunks[1], buf);
        }

        // Render results with explicit backgrounds and scrolling
        if !self.popup.results.is_empty() {
            let visible_count = chunks[2].height as usize;

            // Calculate scroll offset to keep selected item visible
            let scroll_offset = if self.popup.selected_result >= visible_count {
                self.popup.selected_result.saturating_sub(visible_count - 1)
            } else {
                0
            };

            let visible_results: Vec<Line> = self
                .popup
                .results
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(visible_count)
                .map(|(i, result)| {
                    let style = if i == self.popup.selected_result {
                        Style::default().bg(Color::Blue).fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray).bg(bg_color)
                    };
                    Line::styled(result.as_str(), style)
                })
                .collect();

            let results_para = Paragraph::new(visible_results);
            results_para.render(chunks[2], buf);
        }
    }
}
