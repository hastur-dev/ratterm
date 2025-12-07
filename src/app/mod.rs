//! Main application state and event handling.
//!
//! Orchestrates the terminal emulator, code editor, and file browser.

mod input;
mod keymap;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event};

use crate::clipboard::Clipboard;
use crate::config::{Config, KeybindingMode};
use crate::editor::Editor;
use crate::filebrowser::FileBrowser;
use crate::terminal::{pty::PtyError, TerminalMultiplexer};
use crate::ui::{
    editor_widget::EditorWidget,
    file_picker::FilePickerWidget,
    layout::{FocusedPane, SplitLayout},
    popup::{Popup, PopupKind, PopupWidget},
    statusbar::StatusBar,
    terminal_widget::TerminalWidget,
};

/// Event poll timeout in milliseconds.
const POLL_TIMEOUT_MS: u64 = 50;

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Normal editing/terminal mode.
    #[default]
    Normal,
    /// File browser is active.
    FileBrowser,
    /// Popup dialog is active.
    Popup,
}

/// Open file tab.
#[derive(Debug, Clone)]
pub struct OpenFile {
    /// File path.
    pub path: PathBuf,
    /// Display name.
    pub name: String,
}

/// Application state.
pub struct App {
    /// Terminal multiplexer (multiple tabs).
    terminals: Option<TerminalMultiplexer>,
    /// Code editor (right pane).
    editor: Editor,
    /// File browser.
    file_browser: FileBrowser,
    /// Layout manager.
    layout: SplitLayout,
    /// Current app mode.
    mode: AppMode,
    /// Popup dialog.
    popup: Popup,
    /// Open files (tabs).
    open_files: Vec<OpenFile>,
    /// Current file index.
    current_file_idx: usize,
    /// Running flag.
    running: bool,
    /// Status message.
    status: String,
    /// Last error.
    last_error: Option<String>,
    /// Clipboard.
    clipboard: Clipboard,
    /// Configuration (will be used for keybinding modes).
    #[allow(dead_code)]
    config: Config,
}

impl App {
    /// Creates a new application.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        // Load configuration
        let config = Config::load().unwrap_or_default();

        let terminals = match TerminalMultiplexer::new(cols / 2, rows - 2) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!("Failed to create terminal: {}", e);
                None
            }
        };

        let editor = Editor::new(cols / 2, rows - 2);
        let file_browser = FileBrowser::default();

        Ok(Self {
            terminals,
            editor,
            file_browser,
            layout: SplitLayout::new(),
            mode: AppMode::Normal,
            popup: Popup::new(PopupKind::SearchInFile),
            open_files: Vec::new(),
            current_file_idx: 0,
            running: true,
            status: String::new(),
            last_error: None,
            clipboard: Clipboard::new(),
            config,
        })
    }

    /// Returns a reference to the clipboard.
    #[must_use]
    pub fn clipboard(&self) -> &Clipboard {
        &self.clipboard
    }

    /// Returns the current keybinding mode.
    #[must_use]
    pub fn keybinding_mode(&self) -> KeybindingMode {
        self.config.mode
    }

    /// Returns the active terminal (if any).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&crate::terminal::Terminal> {
        self.terminals.as_ref().and_then(|t| t.active_terminal())
    }

    /// Returns mutable reference to the active terminal.
    pub fn active_terminal_mut(&mut self) -> Option<&mut crate::terminal::Terminal> {
        self.terminals.as_mut().and_then(|t| t.active_terminal_mut())
    }

    /// Adds a new terminal tab.
    pub fn add_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            match terminals.add_tab() {
                Ok(idx) => self.set_status(format!("Created terminal tab {}", idx + 1)),
                Err(e) => self.set_status(format!("Cannot create tab: {}", e)),
            }
        }
    }

    /// Closes the current terminal tab.
    pub fn close_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            if terminals.close_tab() {
                self.set_status("Closed terminal tab");
            } else {
                self.set_status("Cannot close last terminal tab");
            }
        }
    }

    /// Switches to next terminal tab.
    pub fn next_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.next_tab();
            let idx = terminals.active_tab_index();
            self.set_status(format!("Terminal {}", idx + 1));
        }
    }

    /// Switches to previous terminal tab.
    pub fn prev_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.prev_tab();
            let idx = terminals.active_tab_index();
            self.set_status(format!("Terminal {}", idx + 1));
        }
    }

    /// Creates a horizontal split in the terminal.
    pub fn split_terminal_horizontal(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            match terminals.split_horizontal() {
                Ok(()) => self.set_status("Split horizontal"),
                Err(e) => self.set_status(format!("Cannot split: {}", e)),
            }
        }
    }

    /// Creates a vertical split in the terminal.
    pub fn split_terminal_vertical(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            match terminals.split_vertical() {
                Ok(()) => self.set_status("Split vertical"),
                Err(e) => self.set_status(format!("Cannot split: {}", e)),
            }
        }
    }

    /// Closes the current terminal split.
    pub fn close_terminal_split(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.close_split();
            self.set_status("Closed split");
        }
    }

    /// Toggles focus between split terminal panes.
    pub fn toggle_terminal_split_focus(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.toggle_split_focus();
            let focus = terminals.current_split_focus();
            let pane = match focus {
                crate::terminal::SplitFocus::First => "first",
                crate::terminal::SplitFocus::Second => "second",
            };
            self.set_status(format!("Focus: {} pane", pane));
        }
    }

    /// Copies text to clipboard.
    pub fn copy_to_clipboard(&mut self, text: &str) {
        if let Err(e) = self.clipboard.copy(text) {
            self.set_status(format!("Copy failed: {}", e));
        } else {
            self.set_status("Copied to clipboard");
        }
    }

    /// Pastes from clipboard.
    pub fn paste_from_clipboard(&mut self) -> Option<String> {
        match self.clipboard.paste() {
            Ok(text) => Some(text),
            Err(e) => {
                self.set_status(format!("Paste failed: {}", e));
                None
            }
        }
    }

    /// Returns true if the app is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Requests to quit the application.
    /// If there are unsaved changes, shows a confirmation dialog.
    pub fn request_quit(&mut self) {
        if self.editor.is_modified() {
            self.show_popup(PopupKind::ConfirmSaveBeforeExit);
        } else {
            self.running = false;
        }
    }

    /// Forces quit without checking for unsaved changes.
    pub fn force_quit(&mut self) {
        self.running = false;
    }

    /// Saves the current file and then quits.
    pub fn save_and_quit(&mut self) {
        if let Err(e) = self.editor.save() {
            self.set_status(format!("Error saving: {}", e));
        } else {
            self.running = false;
        }
    }

    /// Sets the status message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    /// Opens a file in the editor.
    ///
    /// # Errors
    /// Returns error if file cannot be opened.
    pub fn open_file(&mut self, path: impl Into<PathBuf>) -> io::Result<()> {
        let path = path.into();
        self.editor.open(&path)?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        if !self.open_files.iter().any(|f| f.path == path) {
            self.open_files.push(OpenFile {
                path: path.clone(),
                name,
            });
            self.current_file_idx = self.open_files.len() - 1;
        }

        self.set_status(format!("Opened {}", path.display()));
        self.layout.set_focused(FocusedPane::Editor);
        self.mode = AppMode::Normal;
        self.file_browser.hide();
        Ok(())
    }

    /// Shows the file browser.
    pub fn show_file_browser(&mut self) {
        let _ = self.file_browser.refresh();
        self.file_browser.show();
        self.mode = AppMode::FileBrowser;
        self.layout.set_focused(FocusedPane::Editor);
    }

    /// Hides the file browser.
    pub fn hide_file_browser(&mut self) {
        self.file_browser.hide();
        self.mode = AppMode::Normal;
    }

    /// Shows a popup dialog.
    pub fn show_popup(&mut self, kind: PopupKind) {
        self.popup.set_kind(kind);
        self.popup.clear();

        if matches!(kind, PopupKind::CreateFile) {
            if let Some(ext) = self.file_browser.common_extension() {
                self.popup.set_suggestion(Some(format!(".{}", ext)));
            }
        }

        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the popup.
    pub fn hide_popup(&mut self) {
        self.popup.hide();
        self.mode = if self.file_browser.is_visible() {
            AppMode::FileBrowser
        } else {
            AppMode::Normal
        };
    }

    /// Switches to the next open file.
    pub fn next_file(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        self.current_file_idx = (self.current_file_idx + 1) % self.open_files.len();
        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        }
    }

    /// Switches to the previous open file.
    pub fn prev_file(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        self.current_file_idx = if self.current_file_idx == 0 {
            self.open_files.len() - 1
        } else {
            self.current_file_idx - 1
        };
        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        }
    }

    /// Handles terminal resize.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let areas = self
            .layout
            .calculate(ratatui::layout::Rect::new(0, 0, cols, rows));

        if let Some(ref mut terminals) = self.terminals {
            if areas.has_terminal() {
                let _ = terminals.resize(
                    areas.terminal.width.saturating_sub(2),
                    areas.terminal.height.saturating_sub(2),
                );
            }
        }

        if areas.has_editor() {
            self.editor.resize(
                areas.editor.width.saturating_sub(2),
                areas.editor.height.saturating_sub(2),
            );
            self.file_browser
                .set_visible_height(areas.editor.height.saturating_sub(4) as usize);
        }
    }

    /// Processes events and updates state.
    ///
    /// # Errors
    /// Returns error if event processing fails.
    pub fn update(&mut self) -> io::Result<()> {
        if let Some(ref mut terminals) = self.terminals {
            if let Err(e) = terminals.process_all() {
                self.last_error = Some(format!("Terminal error: {}", e));
            }
        }

        if event::poll(Duration::from_millis(POLL_TIMEOUT_MS))? {
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            }
        }

        Ok(())
    }

    /// Updates popup results based on current input.
    pub(crate) fn update_popup_results(&mut self) {
        let input = self.popup.input().to_string();

        let results: Vec<String> = match self.popup.kind() {
            PopupKind::SearchFiles => self
                .file_browser
                .search_files(&input)
                .into_iter()
                .take(10)
                .map(|e| e.name().to_string())
                .collect(),
            PopupKind::SearchDirectories => self
                .file_browser
                .search_directories(&input)
                .into_iter()
                .take(10)
                .map(|e| e.name().to_string())
                .collect(),
            _ => Vec::new(),
        };

        self.popup.set_results(results);
    }

    /// Executes the popup action.
    pub(crate) fn execute_popup_action(&mut self) {
        let input = self.popup.final_input();

        match self.popup.kind() {
            PopupKind::SearchInFile => {
                self.set_status(format!("Searching for: {}", input));
                self.hide_popup();
            }
            PopupKind::SearchInFiles => {
                self.set_status(format!("Searching all files for: {}", input));
                self.hide_popup();
            }
            PopupKind::SearchFiles | PopupKind::SearchDirectories => {
                if let Some(result) = self.popup.selected_result() {
                    let path = self.file_browser.path().join(result);
                    if path.is_file() {
                        let _ = self.open_file(path);
                    } else if path.is_dir() {
                        let _ = self.file_browser.change_dir(&path);
                        self.show_file_browser();
                    }
                }
                self.hide_popup();
            }
            PopupKind::CreateFile => {
                if !input.is_empty() {
                    let path = self.file_browser.path().join(&input);
                    match std::fs::write(&path, "") {
                        Ok(()) => {
                            let _ = self.file_browser.refresh();
                            let _ = self.open_file(path);
                        }
                        Err(e) => {
                            self.popup.set_error(Some(format!("Error: {}", e)));
                            return;
                        }
                    }
                }
                self.hide_popup();
            }
            PopupKind::CreateFolder => {
                if !input.is_empty() {
                    let path = self.file_browser.path().join(&input);
                    match std::fs::create_dir(&path) {
                        Ok(()) => {
                            let _ = self.file_browser.refresh();
                            self.set_status(format!("Created folder: {}", path.display()));
                        }
                        Err(e) => {
                            self.popup.set_error(Some(format!("Error: {}", e)));
                            return;
                        }
                    }
                }
                self.hide_popup();
            }
            PopupKind::ConfirmSaveBeforeExit => {
                // This is handled by handle_confirmation_key, not execute_popup_action
                self.hide_popup();
            }
        }
    }

    /// Renders the application.
    pub fn render(&self, frame: &mut ratatui::Frame) {
        use ratatui::layout::{Constraint, Direction, Layout};

        let area = frame.area();
        let areas = self.layout.calculate(area);

        // Render terminal pane (with split support)
        if areas.has_terminal() {
            if let Some(ref terminals) = self.terminals {
                if let Some(tab) = terminals.active_tab() {
                    let is_focused = self.layout.focused() == FocusedPane::Terminal;

                    match tab.split {
                        crate::terminal::SplitDirection::None => {
                            // Single terminal
                            let widget = TerminalWidget::new(&tab.terminal)
                                .focused(is_focused)
                                .title(tab.terminal.title());
                            frame.render_widget(widget, areas.terminal);
                        }
                        crate::terminal::SplitDirection::Horizontal => {
                            // Top/bottom split
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(areas.terminal);

                            let first_focused =
                                is_focused && tab.split_focus == crate::terminal::SplitFocus::First;
                            let second_focused =
                                is_focused && tab.split_focus == crate::terminal::SplitFocus::Second;

                            let widget1 = TerminalWidget::new(&tab.terminal)
                                .focused(first_focused)
                                .title(tab.terminal.title());
                            frame.render_widget(widget1, chunks[0]);

                            if let Some(ref split_term) = tab.split_terminal {
                                let widget2 = TerminalWidget::new(split_term)
                                    .focused(second_focused)
                                    .title(split_term.title());
                                frame.render_widget(widget2, chunks[1]);
                            }
                        }
                        crate::terminal::SplitDirection::Vertical => {
                            // Left/right split
                            let chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(areas.terminal);

                            let first_focused =
                                is_focused && tab.split_focus == crate::terminal::SplitFocus::First;
                            let second_focused =
                                is_focused && tab.split_focus == crate::terminal::SplitFocus::Second;

                            let widget1 = TerminalWidget::new(&tab.terminal)
                                .focused(first_focused)
                                .title(tab.terminal.title());
                            frame.render_widget(widget1, chunks[0]);

                            if let Some(ref split_term) = tab.split_terminal {
                                let widget2 = TerminalWidget::new(split_term)
                                    .focused(second_focused)
                                    .title(split_term.title());
                                frame.render_widget(widget2, chunks[1]);
                            }
                        }
                    }
                }
            }
        }

        // Render editor or file browser
        if areas.has_editor() {
            if self.file_browser.is_visible() {
                let widget = FilePickerWidget::new(&self.file_browser)
                    .focused(self.layout.focused() == FocusedPane::Editor);
                frame.render_widget(widget, areas.editor);
            } else {
                let widget = EditorWidget::new(&self.editor)
                    .focused(self.layout.focused() == FocusedPane::Editor);
                frame.render_widget(widget, areas.editor);
            }
        }

        // Render status bar
        self.render_status_bar(frame, &areas);

        // Render popup if visible
        if self.popup.is_visible() {
            let popup_widget = PopupWidget::new(&self.popup);
            frame.render_widget(popup_widget, area);
        }
    }

    /// Renders the status bar.
    fn render_status_bar(
        &self,
        frame: &mut ratatui::Frame,
        areas: &crate::ui::layout::LayoutAreas,
    ) {
        let path_string = self.editor.path().map(|p| p.display().to_string());
        let path_ref = path_string.as_deref();
        let terminal_title = self
            .terminals
            .as_ref()
            .and_then(|t| t.active_terminal())
            .map(|t| t.title().to_string());

        let mut status_bar = StatusBar::new()
            .focused_pane(self.layout.focused())
            .keybinding_mode(self.config.mode)
            .message(&self.status);

        if self.layout.focused() == FocusedPane::Editor {
            status_bar = status_bar
                .editor_mode(self.editor.mode())
                .cursor_position(self.editor.cursor_position());

            if let Some(path) = path_ref {
                status_bar = status_bar.file_path(path);
            }
        } else if let Some(ref title) = terminal_title {
            status_bar = status_bar.terminal_title(title);
        }

        // Add tab info to status bar if we have multiple tabs
        let final_message = if let Some(ref terminals) = self.terminals {
            let tab_count = terminals.tab_count();
            if tab_count > 1 {
                let active = terminals.active_tab_index() + 1;
                format!("[Tab {}/{}] {}", active, tab_count, self.status)
            } else {
                self.status.clone()
            }
        } else {
            self.status.clone()
        };

        if !final_message.is_empty() && final_message != self.status {
            status_bar = status_bar.message(&final_message);
        }

        frame.render_widget(status_bar, areas.status_bar);
    }

    /// Shuts down the application.
    pub fn shutdown(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.shutdown();
        }
    }
}
