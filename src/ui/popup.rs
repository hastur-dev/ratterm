//! Popup dialog widgets.
//!
//! Provides modal dialogs for search, file creation, command palette, etc.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::config::{KeybindingMode, ShellDetector, ShellInstaller, ShellType};
use crate::theme::ThemePreset;

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
    /// If at start of input and there's a suggestion, clears the suggestion.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
            self.error = None;
        } else if self.suggestion.is_some() {
            // Clear suggestion when backspacing at start of empty input
            self.suggestion = None;
        }
    }

    /// Deletes character at cursor (delete).
    /// If at end of input and there's a suggestion, clears the suggestion.
    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
            self.error = None;
        } else if self.suggestion.is_some() {
            // Clear suggestion when pressing delete at end of input
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
            let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
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

/// A command that can be executed from the command palette.
#[derive(Debug, Clone)]
pub struct Command {
    /// Unique identifier for the command.
    pub id: &'static str,
    /// Display label for the command.
    pub label: &'static str,
    /// Category for grouping commands.
    pub category: &'static str,
    /// Keyboard shortcut hint.
    pub keybinding: Option<&'static str>,
}

impl Command {
    /// Creates a new command.
    #[must_use]
    pub const fn new(
        id: &'static str,
        label: &'static str,
        category: &'static str,
        keybinding: Option<&'static str>,
    ) -> Self {
        Self {
            id,
            label,
            category,
            keybinding,
        }
    }

    /// Returns formatted display string for command palette.
    #[must_use]
    pub fn display(&self) -> String {
        if let Some(kb) = self.keybinding {
            format!("{}: {}  ({})", self.category, self.label, kb)
        } else {
            format!("{}: {}", self.category, self.label)
        }
    }
}

/// Command palette state and filtering.
pub struct CommandPalette {
    /// All available commands.
    commands: Vec<Command>,
    /// Filtered commands matching current query.
    filtered: Vec<(usize, i64)>, // (index, score)
    /// Fuzzy matcher for filtering.
    matcher: SkimMatcherV2,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Creates a new command palette with all available commands.
    #[must_use]
    pub fn new() -> Self {
        let commands = Self::all_commands();
        let filtered: Vec<(usize, i64)> =
            commands.iter().enumerate().map(|(i, _)| (i, 0)).collect();

        Self {
            commands,
            filtered,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Returns all available commands.
    fn all_commands() -> Vec<Command> {
        vec![
            // File commands
            Command::new("file.new", "New File", "File", Some("Ctrl+N")),
            Command::new("file.newFolder", "New Folder", "File", Some("Ctrl+Shift+N")),
            Command::new("file.open", "Open File Browser", "File", Some("Ctrl+O")),
            Command::new("file.save", "Save", "File", Some("Ctrl+S")),
            Command::new("file.close", "Close File", "File", None),
            // Edit commands
            Command::new("edit.undo", "Undo", "Edit", Some("Ctrl+Z")),
            Command::new("edit.redo", "Redo", "Edit", Some("Ctrl+Y")),
            Command::new("edit.copy", "Copy", "Edit", Some("Ctrl+Shift+C")),
            Command::new("edit.paste", "Paste", "Edit", Some("Ctrl+V")),
            Command::new("edit.selectAll", "Select All", "Edit", Some("Ctrl+A")),
            Command::new("edit.selectLine", "Select Line", "Edit", Some("Ctrl+L")),
            Command::new(
                "edit.duplicateLine",
                "Duplicate Line",
                "Edit",
                Some("Ctrl+D"),
            ),
            Command::new(
                "edit.deleteLine",
                "Delete Line",
                "Edit",
                Some("Ctrl+Shift+K"),
            ),
            Command::new("edit.moveLineUp", "Move Line Up", "Edit", Some("Alt+Up")),
            Command::new(
                "edit.moveLineDown",
                "Move Line Down",
                "Edit",
                Some("Alt+Down"),
            ),
            Command::new(
                "edit.toggleComment",
                "Toggle Comment",
                "Edit",
                Some("Ctrl+/"),
            ),
            Command::new("edit.indent", "Indent", "Edit", Some("Tab")),
            Command::new("edit.outdent", "Outdent", "Edit", Some("Shift+Tab")),
            // Search commands
            Command::new("search.inFile", "Find in File", "Search", Some("Ctrl+F")),
            Command::new(
                "search.inFiles",
                "Find in All Files",
                "Search",
                Some("Ctrl+Shift+F"),
            ),
            Command::new(
                "search.files",
                "Search Files",
                "Search",
                Some("Ctrl+Shift+E"),
            ),
            Command::new(
                "search.directories",
                "Search Directories",
                "Search",
                Some("Ctrl+Shift+D"),
            ),
            // View commands
            Command::new(
                "view.focusTerminal",
                "Focus Terminal",
                "View",
                Some("Alt+Left"),
            ),
            Command::new(
                "view.focusEditor",
                "Focus Editor",
                "View",
                Some("Alt+Right"),
            ),
            Command::new("view.toggleFocus", "Toggle Focus", "View", Some("Alt+Tab")),
            Command::new("view.splitLeft", "Shrink Split", "View", Some("Alt+[")),
            Command::new("view.splitRight", "Expand Split", "View", Some("Alt+]")),
            // Terminal commands
            Command::new("terminal.new", "New Terminal", "Terminal", Some("Ctrl+T")),
            Command::new(
                "terminal.split",
                "Split Terminal",
                "Terminal",
                Some("Ctrl+S"),
            ),
            Command::new(
                "terminal.close",
                "Close Terminal",
                "Terminal",
                Some("Ctrl+W"),
            ),
            Command::new(
                "terminal.nextTab",
                "Next Terminal Tab",
                "Terminal",
                Some("Ctrl+Right"),
            ),
            Command::new(
                "terminal.prevTab",
                "Previous Terminal Tab",
                "Terminal",
                Some("Ctrl+Left"),
            ),
            Command::new("terminal.selectShell", "Select Shell", "Terminal", None),
            // Theme commands
            Command::new("theme.select", "Select Theme", "Theme", None),
            Command::new("theme.dark", "Dark Theme", "Theme", None),
            Command::new("theme.light", "Light Theme", "Theme", None),
            Command::new("theme.dracula", "Dracula Theme", "Theme", None),
            Command::new("theme.gruvbox", "Gruvbox Theme", "Theme", None),
            Command::new("theme.nord", "Nord Theme", "Theme", None),
            // Extension commands
            Command::new("extension.list", "List Installed", "Extension", None),
            Command::new(
                "extension.install",
                "Install from GitHub",
                "Extension",
                None,
            ),
            Command::new("extension.update", "Update All", "Extension", None),
            Command::new("extension.remove", "Remove Extension", "Extension", None),
            // Application commands
            Command::new("app.quit", "Quit", "Application", Some("Ctrl+Q")),
            Command::new(
                "app.commandPalette",
                "Command Palette",
                "Application",
                Some("Ctrl+Shift+P"),
            ),
            Command::new(
                "app.switchEditorMode",
                "Switch Editor Mode",
                "Application",
                Some("Ctrl+Shift+Tab"),
            ),
        ]
    }

    /// Filters commands based on query string.
    pub fn filter(&mut self, query: &str) {
        if query.is_empty() {
            // Show all commands when query is empty
            self.filtered = self
                .commands
                .iter()
                .enumerate()
                .map(|(i, _)| (i, 0))
                .collect();
            return;
        }

        let mut matches: Vec<(usize, i64)> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(idx, cmd)| {
                let search_text = format!("{} {}", cmd.category, cmd.label);
                self.matcher
                    .fuzzy_match(&search_text, query)
                    .map(|score| (idx, score))
            })
            .collect();

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered = matches;
    }

    /// Returns filtered command display strings.
    #[must_use]
    pub fn results(&self) -> Vec<String> {
        self.filtered
            .iter()
            .filter_map(|(idx, _)| self.commands.get(*idx))
            .map(Command::display)
            .collect()
    }

    /// Returns the command at the given filtered index.
    #[must_use]
    pub fn get_command(&self, filtered_index: usize) -> Option<&Command> {
        self.filtered
            .get(filtered_index)
            .and_then(|(idx, _)| self.commands.get(*idx))
    }

    /// Returns number of filtered results.
    #[must_use]
    pub fn len(&self) -> usize {
        self.filtered.len()
    }

    /// Returns true if no filtered results.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filtered.is_empty()
    }
}

/// Mode switcher state for cycling through editor keybinding modes.
pub struct ModeSwitcher {
    /// All available modes in order.
    modes: Vec<KeybindingMode>,
    /// Currently selected mode index.
    selected_index: usize,
    /// Original mode when switcher was opened (for cancel).
    original_mode: KeybindingMode,
}

impl ModeSwitcher {
    /// Creates a new mode switcher starting at the given mode.
    #[must_use]
    pub fn new(current_mode: KeybindingMode) -> Self {
        let modes = vec![
            KeybindingMode::Vim,
            KeybindingMode::Emacs,
            KeybindingMode::Default,
            KeybindingMode::VsCode,
        ];

        let selected_index = modes.iter().position(|m| *m == current_mode).unwrap_or(0);

        Self {
            modes,
            selected_index,
            original_mode: current_mode,
        }
    }

    /// Cycles to the next mode.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.modes.len();
    }

    /// Cycles to the previous mode.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.modes.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected mode.
    #[must_use]
    pub fn selected_mode(&self) -> KeybindingMode {
        self.modes[self.selected_index]
    }

    /// Returns the original mode (for cancellation).
    #[must_use]
    pub fn original_mode(&self) -> KeybindingMode {
        self.original_mode
    }

    /// Returns all modes with their selection state.
    #[must_use]
    pub fn modes_with_selection(&self) -> Vec<(KeybindingMode, bool)> {
        self.modes
            .iter()
            .enumerate()
            .map(|(i, mode)| (*mode, i == self.selected_index))
            .collect()
    }

    /// Returns the display name for a mode.
    #[must_use]
    pub fn mode_name(mode: KeybindingMode) -> &'static str {
        match mode {
            KeybindingMode::Vim => "Vim",
            KeybindingMode::Emacs => "Emacs",
            KeybindingMode::Default => "Default",
            KeybindingMode::VsCode => "VSCode",
        }
    }
}

/// Widget for rendering the mode switcher popup.
pub struct ModeSwitcherWidget<'a> {
    switcher: &'a ModeSwitcher,
}

impl<'a> ModeSwitcherWidget<'a> {
    /// Creates a new mode switcher widget.
    #[must_use]
    pub fn new(switcher: &'a ModeSwitcher) -> Self {
        Self { switcher }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 40_u16.min(area.width.saturating_sub(4));
        let height = 8_u16.min(area.height.saturating_sub(4)); // Title + 4 modes + padding

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ModeSwitcherWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        // Clear background
        Clear.render(popup_area, buf);

        // Draw border
        let block = Block::default()
            .title(" Switch Editor Mode ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout for modes
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        // Render each mode
        for (i, (mode, is_selected)) in self.switcher.modes_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let name = ModeSwitcher::mode_name(*mode);
            let (style, prefix) = if *is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else {
                (Style::default().fg(Color::DarkGray), "  ")
            };

            let text = format!("{}{}", prefix, name);
            let para = Paragraph::new(text)
                .style(style)
                .alignment(Alignment::Center);
            para.render(chunks[i], buf);
        }
    }
}

/// Shell selector state for choosing terminal shell.
pub struct ShellSelector {
    /// Available shells with their info.
    shells: Vec<ShellSelectorItem>,
    /// Currently selected shell index.
    selected_index: usize,
    /// Original shell when selector was opened (for cancel).
    original_shell: ShellType,
}

/// An item in the shell selector list.
#[derive(Debug, Clone)]
pub struct ShellSelectorItem {
    /// Shell type.
    pub shell_type: ShellType,
    /// Whether this shell is available.
    pub available: bool,
    /// Version string if available.
    pub version: Option<String>,
}

impl ShellSelector {
    /// Creates a new shell selector starting at the given shell.
    #[must_use]
    pub fn new(current_shell: ShellType) -> Self {
        let detected = ShellDetector::detect_all();
        let platform_shells = ShellType::available_for_platform();

        let mut shells: Vec<ShellSelectorItem> = platform_shells
            .iter()
            .map(|shell_type| {
                let info = detected.iter().find(|d| d.shell_type == *shell_type);
                ShellSelectorItem {
                    shell_type: *shell_type,
                    available: info.is_some_and(|i| i.available),
                    version: info.and_then(|i| i.version.clone()),
                }
            })
            .collect();

        // Add System default at the end
        shells.push(ShellSelectorItem {
            shell_type: ShellType::System,
            available: true,
            version: None,
        });

        let selected_index = shells
            .iter()
            .position(|s| s.shell_type == current_shell)
            .unwrap_or(shells.len().saturating_sub(1)); // Default to System

        Self {
            shells,
            selected_index,
            original_shell: current_shell,
        }
    }

    /// Cycles to the next shell.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.shells.len();
    }

    /// Cycles to the previous shell.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.shells.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected shell type.
    #[must_use]
    pub fn selected_shell(&self) -> ShellType {
        self.shells[self.selected_index].shell_type
    }

    /// Returns the currently selected shell item.
    #[must_use]
    pub fn selected_item(&self) -> &ShellSelectorItem {
        &self.shells[self.selected_index]
    }

    /// Returns true if the currently selected shell is available.
    #[must_use]
    pub fn is_selected_available(&self) -> bool {
        self.shells[self.selected_index].available
    }

    /// Returns the original shell (for cancellation).
    #[must_use]
    pub fn original_shell(&self) -> ShellType {
        self.original_shell
    }

    /// Returns all shells with their selection state.
    #[must_use]
    pub fn shells_with_selection(&self) -> Vec<(&ShellSelectorItem, bool)> {
        self.shells
            .iter()
            .enumerate()
            .map(|(i, shell)| (shell, i == self.selected_index))
            .collect()
    }
}

/// Widget for rendering the shell selector popup.
pub struct ShellSelectorWidget<'a> {
    selector: &'a ShellSelector,
}

impl<'a> ShellSelectorWidget<'a> {
    /// Creates a new shell selector widget.
    #[must_use]
    pub fn new(selector: &'a ShellSelector) -> Self {
        Self { selector }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let shell_count = self.selector.shells.len();
        let width = 50_u16.min(area.width.saturating_sub(4));
        // Height: 2 border + 1 instructions + shell_count lines + 1 padding
        let height = (4 + shell_count as u16).min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ShellSelectorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        // Clear background
        Clear.render(popup_area, buf);

        // Draw border
        let block = Block::default()
            .title(" Select Terminal Shell ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build constraints for each shell + instructions line
        let shell_count = self.selector.shells.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(shell_count + 1);
        for _ in 0..shell_count {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(1)); // Instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render each shell
        for (i, (shell, is_selected)) in self.selector.shells_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let name = shell.shell_type.display_name();
            let status = if shell.available {
                if let Some(ref ver) = shell.version {
                    format!(" ({})", ver)
                } else {
                    String::new()
                }
            } else {
                " [not installed]".to_string()
            };

            let (style, prefix) = if *is_selected {
                let bg_color = if shell.available {
                    Color::Cyan
                } else {
                    Color::Yellow
                };
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(bg_color)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else if shell.available {
                (Style::default().fg(Color::White), "  ")
            } else {
                (Style::default().fg(Color::DarkGray), "  ")
            };

            let text = format!("{}{}{}", prefix, name, status);
            let para = Paragraph::new(text).style(style);
            para.render(chunks[i], buf);
        }

        // Render instructions at the bottom
        if chunks.len() > shell_count {
            let instructions = Line::from(vec![
                Span::styled("↑↓", Style::default().fg(Color::Cyan)),
                Span::raw(" Select  "),
                Span::styled("Enter", Style::default().fg(Color::Cyan)),
                Span::raw(" Confirm  "),
                Span::styled("Esc", Style::default().fg(Color::Cyan)),
                Span::raw(" Cancel"),
            ]);
            Paragraph::new(instructions)
                .alignment(Alignment::Center)
                .render(chunks[shell_count], buf);
        }
    }
}

/// Shell install prompt state.
pub struct ShellInstallPrompt {
    /// The shell type that needs installation.
    shell_type: ShellType,
    /// Installation instructions.
    instructions: Vec<String>,
    /// Download URL if available.
    download_url: Option<String>,
    /// Whether user confirmed to proceed.
    confirmed: bool,
}

impl ShellInstallPrompt {
    /// Creates a new shell install prompt.
    #[must_use]
    pub fn new(shell_type: ShellType) -> Self {
        let info = ShellInstaller::get_instructions(shell_type);

        Self {
            shell_type,
            instructions: info.manual_steps,
            download_url: info.download_url,
            confirmed: false,
        }
    }

    /// Returns the shell type.
    #[must_use]
    pub fn shell_type(&self) -> ShellType {
        self.shell_type
    }

    /// Returns the installation instructions.
    #[must_use]
    pub fn instructions(&self) -> &[String] {
        &self.instructions
    }

    /// Returns the download URL if available.
    #[must_use]
    pub fn download_url(&self) -> Option<&str> {
        self.download_url.as_deref()
    }

    /// Sets the confirmed state.
    pub fn set_confirmed(&mut self, confirmed: bool) {
        self.confirmed = confirmed;
    }

    /// Returns whether user confirmed.
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

/// Widget for rendering the shell install prompt.
pub struct ShellInstallPromptWidget<'a> {
    prompt: &'a ShellInstallPrompt,
}

impl<'a> ShellInstallPromptWidget<'a> {
    /// Creates a new shell install prompt widget.
    #[must_use]
    pub fn new(prompt: &'a ShellInstallPrompt) -> Self {
        Self { prompt }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let instruction_count = self.prompt.instructions.len();
        let width = 60_u16.min(area.width.saturating_sub(4));
        // Height: 2 border + 2 header + instructions + 2 footer
        let height = (6 + instruction_count as u16).min(area.height.saturating_sub(4));

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ShellInstallPromptWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        // Clear background
        Clear.render(popup_area, buf);

        // Draw border
        let title = format!(" {} Not Installed ", self.prompt.shell_type.display_name());
        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Build content
        let instruction_count = self.prompt.instructions.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(instruction_count + 3);
        constraints.push(Constraint::Length(1)); // Header
        for _ in 0..instruction_count {
            constraints.push(Constraint::Length(1));
        }
        if self.prompt.download_url.is_some() {
            constraints.push(Constraint::Length(1)); // URL
        }
        constraints.push(Constraint::Length(1)); // Footer instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render header
        let header = "To use this shell, please install it:";
        Paragraph::new(header)
            .style(Style::default().fg(Color::White))
            .render(chunks[0], buf);

        // Render instructions
        for (i, instruction) in self.prompt.instructions.iter().enumerate() {
            if i + 1 >= chunks.len() {
                break;
            }
            let text = format!("  {}. {}", i + 1, instruction);
            Paragraph::new(text)
                .style(Style::default().fg(Color::Gray))
                .render(chunks[i + 1], buf);
        }

        // Render download URL if available
        let mut footer_idx = instruction_count + 1;
        if let Some(ref url) = self.prompt.download_url {
            if footer_idx < chunks.len() {
                let url_text = format!("  URL: {}", url);
                Paragraph::new(url_text)
                    .style(Style::default().fg(Color::Cyan))
                    .render(chunks[footer_idx], buf);
                footer_idx += 1;
            }
        }

        // Render footer instructions
        if footer_idx < chunks.len() {
            let footer = Line::from(vec![
                Span::styled("Esc", Style::default().fg(Color::Cyan)),
                Span::raw(" Close  "),
            ]);
            Paragraph::new(footer)
                .alignment(Alignment::Center)
                .render(chunks[footer_idx], buf);
        }
    }
}

/// Theme selector state for choosing color theme.
pub struct ThemeSelector {
    /// All available themes.
    themes: Vec<ThemePreset>,
    /// Currently selected theme index.
    selected_index: usize,
    /// Original theme when selector was opened (for cancel).
    original_theme: ThemePreset,
}

impl ThemeSelector {
    /// Creates a new theme selector starting at the given theme.
    #[must_use]
    pub fn new(current_theme: Option<ThemePreset>) -> Self {
        let themes: Vec<ThemePreset> = ThemePreset::all().to_vec();
        let current = current_theme.unwrap_or(ThemePreset::Dark);

        let selected_index = themes.iter().position(|t| *t == current).unwrap_or(0);

        Self {
            themes,
            selected_index,
            original_theme: current,
        }
    }

    /// Cycles to the next theme.
    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.themes.len();
    }

    /// Cycles to the previous theme.
    pub fn prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.themes.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Returns the currently selected theme.
    #[must_use]
    pub fn selected_theme(&self) -> ThemePreset {
        self.themes[self.selected_index]
    }

    /// Returns the original theme (for cancellation).
    #[must_use]
    pub fn original_theme(&self) -> ThemePreset {
        self.original_theme
    }

    /// Returns all themes with their selection state.
    #[must_use]
    pub fn themes_with_selection(&self) -> Vec<(ThemePreset, bool)> {
        self.themes
            .iter()
            .enumerate()
            .map(|(i, theme)| (*theme, i == self.selected_index))
            .collect()
    }
}

/// Widget for rendering the theme selector popup.
pub struct ThemeSelectorWidget<'a> {
    selector: &'a ThemeSelector,
}

impl<'a> ThemeSelectorWidget<'a> {
    /// Creates a new theme selector widget.
    #[must_use]
    pub fn new(selector: &'a ThemeSelector) -> Self {
        Self { selector }
    }

    /// Calculates the popup area (centered).
    fn popup_area(&self, area: Rect) -> Rect {
        let width = 40_u16.min(area.width.saturating_sub(4));
        let height = 9_u16.min(area.height.saturating_sub(4)); // Title + 5 themes + padding

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        Rect::new(x, y, width, height)
    }
}

impl Widget for ThemeSelectorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = self.popup_area(area);

        // Clear background
        Clear.render(popup_area, buf);

        // Draw border
        let block = Block::default()
            .title(" Select Theme ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Layout for themes
        let theme_count = self.selector.themes.len();
        let mut constraints: Vec<Constraint> = Vec::with_capacity(theme_count + 1);
        for _ in 0..theme_count {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(1)); // Instructions

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(constraints)
            .split(inner);

        // Render each theme
        for (i, (theme, is_selected)) in self.selector.themes_with_selection().iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let name = theme.name();
            let display_name = match name {
                "dark" => "Dark",
                "light" => "Light",
                "dracula" => "Dracula",
                "gruvbox" => "Gruvbox",
                "nord" => "Nord",
                _ => name,
            };

            let (style, prefix) = if *is_selected {
                (
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    "► ",
                )
            } else {
                (Style::default().fg(Color::White), "  ")
            };

            let text = format!("{}{}", prefix, display_name);
            let para = Paragraph::new(text)
                .style(style)
                .alignment(Alignment::Center);
            para.render(chunks[i], buf);
        }

        // Render instructions at the bottom
        if chunks.len() > theme_count {
            let instructions = Line::from(vec![
                Span::styled("↑↓", Style::default().fg(Color::Cyan)),
                Span::raw(" Select  "),
                Span::styled("Enter", Style::default().fg(Color::Cyan)),
                Span::raw(" Apply  "),
                Span::styled("Esc", Style::default().fg(Color::Cyan)),
                Span::raw(" Cancel"),
            ]);
            Paragraph::new(instructions)
                .alignment(Alignment::Center)
                .render(chunks[theme_count], buf);
        }
    }
}
