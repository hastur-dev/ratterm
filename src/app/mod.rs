//! Main application state and event handling.
//!
//! Orchestrates the terminal emulator, code editor, and file browser.

mod input;
mod keymap;

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use crossterm::event::{self, Event};
use ratatui::layout::Rect;
use tracing::{debug, info, warn};

use crate::api::{ApiHandler, ApiServer, MAX_REQUESTS_PER_FRAME, RequestReceiver};

use crate::clipboard::Clipboard;
use crate::config::{Config, KeybindingMode, ShellType};
use crate::editor::Editor;
use crate::extension::ExtensionManager;
use crate::filebrowser::FileBrowser;
use crate::terminal::{BackgroundManager, ProcessInfo, TerminalMultiplexer, pty::PtyError};
use crate::theme::ThemePreset;
use crate::ui::{
    editor_tabs::{EditorTabBar, EditorTabInfo},
    editor_widget::EditorWidget,
    file_picker::FilePickerWidget,
    layout::{FocusedPane, SplitLayout},
    popup::{
        CommandPalette, ExtensionApprovalPrompt, ModeSwitcher, ModeSwitcherWidget, Popup,
        PopupKind, PopupWidget, ShellInstallPrompt, ShellInstallPromptWidget, ShellSelector,
        ShellSelectorWidget, ThemeSelector, ThemeSelectorWidget,
    },
    statusbar::StatusBar,
    terminal_tabs::TerminalTabBar,
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
    /// Command palette for VSCode-style command access.
    command_palette: CommandPalette,
    /// Mode switcher for cycling through editor keybinding modes.
    mode_switcher: Option<ModeSwitcher>,
    /// Shell selector for choosing terminal shell.
    shell_selector: Option<ShellSelector>,
    /// Shell install prompt for unavailable shells.
    shell_install_prompt: Option<ShellInstallPrompt>,
    /// Theme selector for choosing color theme.
    theme_selector: Option<ThemeSelector>,
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
    /// Configuration.
    config: Config,
    /// Cached terminal area for mouse coordinate conversion (interior mutability for render).
    last_terminal_area: Cell<Rect>,
    /// Flag to request a full screen redraw (clears ghost artifacts).
    needs_redraw: bool,
    /// Flag to request restart after update (for in-app updates).
    request_restart_after_update: bool,
    /// API server (runs in background thread).
    api_server: Option<ApiServer>,
    /// API request receiver (from background thread).
    api_request_rx: Option<RequestReceiver>,
    /// Background process manager.
    background_manager: BackgroundManager,
    /// Extension manager for handling installed extensions.
    extension_manager: ExtensionManager,
    /// Extension approval prompt (shown when extension needs user consent).
    extension_approval_prompt: Option<ExtensionApprovalPrompt>,
    /// Last known screen size for layout-triggered resizes.
    last_screen_size: (u16, u16),
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

        // Get the shell path from config
        let shell_path = config.shell.get_shell_path();

        // Subtract 4 from height: 1 for status bar + 1 for tab bar + 2 for borders
        let terminals =
            match TerminalMultiplexer::with_shell(cols / 2, rows.saturating_sub(4), shell_path) {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!("Failed to create terminal: {}", e);
                    None
                }
            };

        let editor = Editor::new(cols / 2, rows.saturating_sub(4));
        let file_browser = FileBrowser::default();

        // Create layout based on ide_always config
        let layout = if config.ide_always {
            SplitLayout::with_ide_visible()
        } else {
            SplitLayout::new() // Terminal-first, IDE hidden
        };

        // Start API server
        let (api_server, api_request_rx) = match ApiServer::start(None) {
            Ok((server, rx)) => {
                info!("API server started");
                (Some(server), Some(rx))
            }
            Err(e) => {
                warn!("Failed to start API server: {}", e);
                (None, None)
            }
        };

        Ok(Self {
            terminals,
            editor,
            file_browser,
            layout,
            mode: AppMode::Normal,
            popup: Popup::new(PopupKind::SearchInFile),
            command_palette: CommandPalette::new(),
            mode_switcher: None,
            shell_selector: None,
            shell_install_prompt: None,
            theme_selector: None,
            open_files: Vec::new(),
            current_file_idx: 0,
            running: true,
            status: String::new(),
            last_error: None,
            clipboard: Clipboard::new(),
            config,
            last_terminal_area: Cell::new(Rect::default()),
            needs_redraw: false,
            request_restart_after_update: false,
            api_server,
            api_request_rx,
            background_manager: BackgroundManager::new(),
            extension_manager: ExtensionManager::new(),
            extension_approval_prompt: None,
            last_screen_size: (80, 24), // Default, will be updated on first resize
        })
    }

    /// Takes the redraw request flag, resetting it to false.
    /// Returns true if a full screen redraw was requested.
    pub fn take_redraw_request(&mut self) -> bool {
        std::mem::take(&mut self.needs_redraw)
    }

    /// Requests a full screen redraw on the next frame.
    /// Use this when changing modes or after operations that may leave ghost artifacts.
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Returns true if the app requested a restart after an in-app update.
    #[must_use]
    pub fn needs_restart_after_update(&self) -> bool {
        self.request_restart_after_update
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

    /// Returns a reference to the editor.
    #[must_use]
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Returns a mutable reference to the editor.
    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    /// Returns a reference to the layout manager.
    #[must_use]
    pub fn layout(&self) -> &SplitLayout {
        &self.layout
    }

    /// Returns a mutable reference to the layout manager.
    pub fn layout_mut(&mut self) -> &mut SplitLayout {
        &mut self.layout
    }

    /// Returns the status message.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Returns the current file path (if any).
    #[must_use]
    pub fn current_file_path(&self) -> Option<&Path> {
        self.open_files
            .get(self.current_file_idx)
            .map(|f| f.path.as_path())
    }

    /// Returns true if the current file has unsaved modifications.
    #[must_use]
    pub fn is_file_modified(&self) -> bool {
        self.editor.is_modified()
    }

    /// Saves the file at the given path.
    ///
    /// # Errors
    /// Returns error if save fails.
    pub fn save_file(&mut self, path: &Path) -> io::Result<()> {
        self.editor.save_as(path)?;
        self.set_status(format!("Saved {}", path.display()));
        Ok(())
    }

    /// Returns terminal tab information for the API.
    #[must_use]
    pub fn terminal_tabs(&self) -> Vec<crate::api::protocol::TerminalTabInfo> {
        use crate::api::protocol::TerminalTabInfo;

        if let Some(ref terminals) = self.terminals {
            let tab_info = terminals.tab_info();
            tab_info
                .iter()
                .enumerate()
                .map(|(i, info)| TerminalTabInfo {
                    index: i,
                    name: info.name.clone(),
                    active: info.is_active,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns editor tab information for the API.
    #[must_use]
    pub fn editor_tabs(&self) -> Vec<crate::api::protocol::EditorTabInfo> {
        use crate::api::protocol::EditorTabInfo;

        self.open_files
            .iter()
            .enumerate()
            .map(|(i, file)| EditorTabInfo {
                index: i,
                name: file.name.clone(),
                path: Some(file.path.to_string_lossy().into_owned()),
                modified: i == self.current_file_idx && self.editor.is_modified(),
                active: i == self.current_file_idx,
            })
            .collect()
    }

    /// Switches to the terminal tab at the given index.
    pub fn switch_terminal_tab(&mut self, index: usize) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.switch_to(index);
            self.set_status(format!("Terminal {}", index + 1));
        }
    }

    /// Returns the active terminal (if any).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&crate::terminal::Terminal> {
        self.terminals.as_ref().and_then(|t| t.active_terminal())
    }

    /// Returns mutable reference to the active terminal.
    pub fn active_terminal_mut(&mut self) -> Option<&mut crate::terminal::Terminal> {
        self.terminals
            .as_mut()
            .and_then(|t| t.active_terminal_mut())
    }

    /// Adds a new terminal tab.
    pub fn add_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            let shell_name = self.config.shell.display_name();
            match terminals.add_tab_with_shell(shell_path.clone()) {
                Ok(idx) => {
                    if let Some(ref path) = shell_path {
                        self.set_status(format!(
                            "Created terminal {} with {} ({})",
                            idx + 1,
                            shell_name,
                            path.display()
                        ));
                    } else {
                        self.set_status(format!(
                            "Created terminal {} with system default",
                            idx + 1
                        ));
                    }
                }
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
            let shell_path = self.config.shell.get_shell_path();
            match terminals.split_horizontal_with_shell(shell_path) {
                Ok(()) => self.set_status("Split horizontal"),
                Err(e) => self.set_status(format!("Cannot split: {}", e)),
            }
        }
    }

    /// Creates a vertical split in the terminal.
    pub fn split_terminal_vertical(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            match terminals.split_vertical_with_shell(shell_path) {
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

        // Request full redraw to ensure clean rendering
        self.request_redraw();
        Ok(())
    }

    /// Shows the file browser.
    ///
    /// The file browser will open in the terminal's current working directory
    /// if available, otherwise in its current directory.
    pub fn show_file_browser(&mut self) {
        // Try to get the terminal's current working directory
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                let cwd = terminal.current_working_dir();
                // Change to the terminal's CWD if it's different
                if cwd.is_dir() && cwd != self.file_browser.path() {
                    let _ = self.file_browser.change_dir(&cwd);
                }
            }
        }

        let _ = self.file_browser.refresh();
        self.file_browser.show();
        self.mode = AppMode::FileBrowser;
        self.layout.set_focused(FocusedPane::Editor);
        // Request full redraw when showing file browser
        self.request_redraw();
    }

    /// Hides the file browser.
    pub fn hide_file_browser(&mut self) {
        self.file_browser.hide();
        self.mode = AppMode::Normal;

        // Request full redraw to ensure clean rendering
        self.request_redraw();
    }

    /// Shows the IDE pane (editor).
    pub fn show_ide(&mut self) {
        self.layout.show_ide();
        self.resize_for_current_layout();
        self.request_redraw();
        self.set_status("IDE opened");
    }

    /// Hides the IDE pane.
    pub fn hide_ide(&mut self) {
        self.layout.hide_ide();
        self.resize_for_current_layout();
        self.request_redraw();
        self.set_status("IDE closed");
    }

    /// Toggles the IDE pane visibility.
    pub fn toggle_ide(&mut self) {
        if self.layout.ide_visible() {
            self.hide_ide();
        } else {
            self.show_ide();
        }
    }

    /// Returns true if the IDE pane is visible.
    #[must_use]
    pub fn ide_visible(&self) -> bool {
        self.layout.ide_visible()
    }

    /// Moves split left (increases terminal size, decreases editor size).
    pub fn move_split_left(&mut self) {
        self.layout.move_split_left();
        self.resize_for_current_layout();
        self.request_redraw();
    }

    /// Moves split right (decreases terminal size, increases editor size).
    pub fn move_split_right(&mut self) {
        self.layout.move_split_right();
        self.resize_for_current_layout();
        self.request_redraw();
    }

    /// Checks if IDE should auto-hide (no open files and not ide_always).
    /// Called after closing editor tabs.
    pub fn check_ide_auto_hide(&mut self) {
        // Don't auto-hide if ide_always is set
        if self.config.ide_always {
            return;
        }

        // Hide IDE if no files are open
        if self.open_files.is_empty() && self.layout.ide_visible() {
            self.hide_ide();
        }
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

        // Initialize command palette with all commands
        if matches!(kind, PopupKind::CommandPalette) {
            self.command_palette.filter("");
            self.popup.set_results(self.command_palette.results());
        }

        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the popup.
    pub fn hide_popup(&mut self) {
        self.popup.hide();
        self.mode_switcher = None; // Clear mode switcher when hiding popup
        self.shell_selector = None; // Clear shell selector when hiding popup
        self.shell_install_prompt = None; // Clear shell install prompt when hiding popup
        self.mode = if self.file_browser.is_visible() {
            AppMode::FileBrowser
        } else {
            AppMode::Normal
        };
    }

    /// Shows the mode switcher popup.
    pub fn show_mode_switcher(&mut self) {
        self.mode_switcher = Some(ModeSwitcher::new(self.config.mode));
        self.popup.set_kind(PopupKind::ModeSwitcher);
        self.popup.clear();
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Cycles to the next editor mode in the mode switcher.
    pub fn cycle_mode_next(&mut self) {
        if let Some(ref mut switcher) = self.mode_switcher {
            switcher.next();
        }
    }

    /// Cycles to the previous editor mode in the mode switcher.
    pub fn cycle_mode_prev(&mut self) {
        if let Some(ref mut switcher) = self.mode_switcher {
            switcher.prev();
        }
    }

    /// Applies the selected mode from the mode switcher and closes it.
    pub fn apply_mode_switch(&mut self) {
        if let Some(ref switcher) = self.mode_switcher {
            let new_mode = switcher.selected_mode();
            self.config.mode = new_mode;
            self.set_status(format!(
                "Switched to {} mode",
                crate::ui::popup::ModeSwitcher::mode_name(new_mode)
            ));
        }
        self.hide_popup();
    }

    /// Cancels the mode switch and reverts to the original mode.
    pub fn cancel_mode_switch(&mut self) {
        self.hide_popup();
    }

    /// Returns true if the mode switcher is currently active.
    #[must_use]
    pub fn is_mode_switcher_active(&self) -> bool {
        self.mode_switcher.is_some() && self.popup.kind().is_mode_switcher()
    }

    /// Shows the shell selector popup.
    pub fn show_shell_selector(&mut self) {
        self.shell_selector = Some(ShellSelector::new(self.config.shell));
        self.popup.set_kind(PopupKind::ShellSelector);
        self.popup.clear();
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Cycles to the next shell in the shell selector.
    pub fn cycle_shell_next(&mut self) {
        if let Some(ref mut selector) = self.shell_selector {
            selector.next();
        }
    }

    /// Cycles to the previous shell in the shell selector.
    pub fn cycle_shell_prev(&mut self) {
        if let Some(ref mut selector) = self.shell_selector {
            selector.prev();
        }
    }

    /// Applies the selected shell from the selector.
    /// If the shell is not available, shows the install prompt instead.
    /// Automatically creates a new tab with the selected shell.
    pub fn apply_shell_selection(&mut self) {
        if let Some(ref selector) = self.shell_selector {
            let selected_shell = selector.selected_shell();

            if !selector.is_selected_available() {
                // Shell not available - show install prompt
                self.shell_install_prompt = Some(ShellInstallPrompt::new(selected_shell));
                self.popup.set_kind(PopupKind::ShellInstallPrompt);
                self.shell_selector = None;
                return;
            }

            // Shell is available - apply the selection
            self.config.shell = selected_shell;

            // Close old tabs if configured
            if self.config.auto_close_tabs_on_shell_change {
                self.close_all_terminal_tabs();
            }

            // Hide popup first so we can create new tab
            self.shell_selector = None;
            self.popup.hide();
            self.mode = AppMode::Normal;

            // Create new tab with the selected shell
            self.add_terminal_tab();

            // Focus terminal pane
            self.layout
                .set_focused(crate::ui::layout::FocusedPane::Terminal);

            return;
        }
        self.hide_popup();
    }

    /// Closes all terminal tabs except one, then closes the remaining one's shell.
    fn close_all_terminal_tabs(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            // Close all tabs until only one remains
            while terminals.tab_count() > 1 {
                terminals.close_tab();
            }
        }
    }

    /// Cancels the shell selection.
    pub fn cancel_shell_selection(&mut self) {
        self.hide_popup();
    }

    /// Returns true if the shell selector is currently active.
    #[must_use]
    pub fn is_shell_selector_active(&self) -> bool {
        self.shell_selector.is_some() && self.popup.kind().is_shell_selector()
    }

    /// Returns true if the shell install prompt is currently active.
    #[must_use]
    pub fn is_shell_install_prompt_active(&self) -> bool {
        self.shell_install_prompt.is_some() && self.popup.kind().is_shell_install_prompt()
    }

    /// Returns the current shell configuration.
    #[must_use]
    pub fn current_shell(&self) -> ShellType {
        self.config.shell
    }

    /// Shows the theme selector popup.
    pub fn show_theme_selector(&mut self) {
        let current_name = self.current_theme_name();
        let all_themes = self.available_themes();
        self.theme_selector = Some(ThemeSelector::new_with_themes(&current_name, all_themes));
        self.popup.set_kind(PopupKind::ThemeSelector);
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Applies the selected theme.
    pub fn apply_theme_selection(&mut self) {
        if let Some(ref selector) = self.theme_selector {
            let selected_name = selector.selected_theme_name().to_string();

            // Use set_theme_by_name which supports both presets and custom themes
            if let Err(e) = self.set_theme_by_name(&selected_name) {
                self.set_status(format!("Failed to set theme: {}", e));
            }
            // set_theme_by_name already sets the status on success
        }
        self.theme_selector = None;
        self.hide_popup();
    }

    /// Shows installed extensions in the status bar.
    pub fn show_installed_extensions(&mut self) {
        let mut manager = ExtensionManager::new();
        if let Err(e) = manager.init() {
            self.set_status(format!("Failed to load extensions: {}", e));
            return;
        }

        let extensions = manager.installed();
        if extensions.is_empty() {
            self.set_status(
                "No extensions installed. Use: rat ext install <user/repo>".to_string(),
            );
        } else {
            let names: Vec<_> = extensions
                .values()
                .map(|e| format!("{} v{}", e.name, e.version))
                .collect();
            self.set_status(format!("Extensions: {}", names.join(", ")));
        }
    }

    /// Cancels the theme selection.
    pub fn cancel_theme_selection(&mut self) {
        self.theme_selector = None;
        self.hide_popup();
    }

    /// Returns true if the theme selector is currently active.
    #[must_use]
    pub fn is_theme_selector_active(&self) -> bool {
        self.theme_selector.is_some() && self.popup.kind().is_theme_selector()
    }

    /// Returns the current theme name.
    #[must_use]
    pub fn current_theme_name(&self) -> String {
        self.config.theme_manager.current().name().to_string()
    }

    /// Returns the current theme preset, if using one.
    #[must_use]
    pub fn current_theme_preset(&self) -> Option<ThemePreset> {
        self.config.theme_manager.current_preset()
    }

    /// Returns all available theme names.
    #[must_use]
    pub fn available_themes(&self) -> Vec<String> {
        self.config.theme_manager.all_available_themes()
    }

    /// Sets the theme to a specific preset.
    pub fn set_theme(&mut self, preset: ThemePreset) {
        self.config.theme_manager.set_preset(preset);
        if let Err(e) = self.config.save_theme() {
            self.set_status(format!("Failed to save theme: {}", e));
        } else {
            self.set_status(format!("Theme changed to: {}", preset.name()));
        }
    }

    /// Sets the theme by name, supporting both presets and custom themes.
    /// Returns Ok(()) on success, Err(error_message) on failure.
    pub fn set_theme_by_name(&mut self, name: &str) -> Result<(), String> {
        // First try preset themes
        if let Some(preset) = ThemePreset::from_name(name) {
            self.set_theme(preset);
            return Ok(());
        }

        // Try custom themes
        let custom_themes = crate::theme::list_custom_theme_info();
        for info in custom_themes {
            if info.name == name {
                match self.config.theme_manager.load_custom_theme(&info.path) {
                    Ok(()) => {
                        self.set_status(format!("Theme changed to: {}", name));
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(format!("Failed to load custom theme: {}", e));
                    }
                }
            }
        }

        Err(format!("Unknown theme: {}", name))
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

    /// Returns information about open file tabs.
    #[must_use]
    pub fn editor_tab_info(&self) -> Vec<EditorTabInfo> {
        self.open_files
            .iter()
            .enumerate()
            .map(|(i, file)| EditorTabInfo {
                index: i,
                name: file.name.clone(),
                is_active: i == self.current_file_idx,
                is_modified: i == self.current_file_idx && self.editor.is_modified(),
            })
            .collect()
    }

    /// Creates a new untitled editor tab.
    pub fn new_editor_tab(&mut self) {
        // Count existing untitled buffers
        let untitled_count = self
            .open_files
            .iter()
            .filter(|f| f.name.starts_with("Untitled"))
            .count();

        let name = if untitled_count == 0 {
            "Untitled".to_string()
        } else {
            format!("Untitled-{}", untitled_count + 1)
        };

        // Create new buffer in editor
        self.editor.new_buffer();

        // Add to open files
        self.open_files.push(OpenFile {
            path: PathBuf::from(&name),
            name: name.clone(),
        });
        self.current_file_idx = self.open_files.len() - 1;

        self.set_status(format!("Created {}", name));
    }

    /// Closes the current editor tab.
    pub fn close_editor_tab(&mut self) {
        if self.open_files.is_empty() {
            self.set_status("No tabs to close");
            return;
        }

        // Check for unsaved changes
        if self.editor.is_modified() {
            self.show_popup(PopupKind::ConfirmSaveBeforeExit);
            return;
        }

        // Remove current file
        let closed_name = self.open_files[self.current_file_idx].name.clone();
        self.open_files.remove(self.current_file_idx);

        // Adjust index
        if self.current_file_idx >= self.open_files.len() && !self.open_files.is_empty() {
            self.current_file_idx = self.open_files.len() - 1;
        }

        // Open the now-current file, or clear editor if no files left
        if let Some(file) = self.open_files.get(self.current_file_idx) {
            let _ = self.editor.open(&file.path);
        } else {
            self.editor.new_buffer();
            self.current_file_idx = 0;
        }

        self.set_status(format!("Closed {}", closed_name));

        // Check if IDE should auto-hide now that a tab was closed
        self.check_ide_auto_hide();
    }

    /// Closes the current file (alias for close_editor_tab).
    pub fn close_current_file(&mut self) {
        self.close_editor_tab();
    }

    // ========================================================================
    // Background process methods
    // ========================================================================

    /// Starts a command in the background.
    ///
    /// # Errors
    /// Returns error message if the process cannot be started.
    pub fn start_background_process(&mut self, command: &str) -> Result<u64, String> {
        let id = self.background_manager.start(command)?;
        self.set_status(format!("Started background process {} : {}", id, command));
        Ok(id)
    }

    /// Lists all background processes with counts.
    #[must_use]
    pub fn list_background_processes(&mut self) -> (Vec<ProcessInfo>, usize, usize) {
        self.background_manager.update_counts();
        let processes = self.background_manager.list();
        let running = self.background_manager.running_count();
        let errors = self.background_manager.error_count();
        (processes, running, errors)
    }

    /// Gets information about a specific background process.
    #[must_use]
    pub fn get_background_process_info(&self, id: u64) -> Option<ProcessInfo> {
        self.background_manager.get_info(id)
    }

    /// Gets the output of a specific background process.
    #[must_use]
    pub fn get_background_process_output(&self, id: u64) -> Option<String> {
        self.background_manager.get_output(id)
    }

    /// Kills a background process.
    ///
    /// # Errors
    /// Returns error message if the process cannot be killed.
    pub fn kill_background_process(&mut self, id: u64) -> Result<(), String> {
        self.background_manager.kill(id)?;
        self.set_status(format!("Killed background process {}", id));
        Ok(())
    }

    /// Clears finished background processes.
    pub fn clear_finished_background_processes(&mut self) {
        self.background_manager.clear_finished();
        self.background_manager.clear_errors();
        self.set_status("Cleared finished background processes".to_string());
    }

    /// Returns the number of running background processes.
    #[must_use]
    pub fn background_running_count(&self) -> usize {
        self.background_manager.running_count()
    }

    /// Returns the number of background processes with errors.
    #[must_use]
    pub fn background_error_count(&self) -> usize {
        self.background_manager.error_count()
    }

    /// Handles terminal resize.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        // Store screen size for layout-triggered resizes
        self.last_screen_size = (cols, rows);
        self.resize_for_current_layout();
    }

    /// Resizes terminal and editor based on current layout and stored screen size.
    /// Called after layout changes (IDE show/hide, split resize).
    fn resize_for_current_layout(&mut self) {
        let (cols, rows) = self.last_screen_size;
        let areas = self
            .layout
            .calculate(ratatui::layout::Rect::new(0, 0, cols, rows));

        if let Some(ref mut terminals) = self.terminals {
            if areas.has_terminal() {
                // Subtract 3: 1 for tab bar + 2 for borders
                let _ = terminals.resize(
                    areas.terminal.width.saturating_sub(2),
                    areas.terminal.height.saturating_sub(3),
                );
            }
        }

        if areas.has_editor() {
            // Subtract 3: 1 for tab bar + 2 for borders
            self.editor.resize(
                areas.editor.width.saturating_sub(2),
                areas.editor.height.saturating_sub(3),
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
        // Process API requests (non-blocking, bounded)
        self.process_api_requests();

        // Update background process counts
        self.background_manager.update_counts();

        // Only process PTY output when file browser is NOT visible
        // This prevents the terminal grid from being corrupted by PTY output
        // while the user is interacting with the file browser
        if !self.file_browser.is_visible() {
            if let Some(ref mut terminals) = self.terminals {
                if let Err(e) = terminals.process_all() {
                    self.last_error = Some(format!("Terminal error: {}", e));
                }
            }
        }

        if event::poll(Duration::from_millis(POLL_TIMEOUT_MS))? {
            match event::read()? {
                Event::Key(key) => self.handle_key(key),
                Event::Mouse(mouse) => self.handle_mouse(mouse),
                Event::Resize(width, height) => self.resize(width, height),
                _ => {}
            }
        }

        Ok(())
    }

    /// Processes pending API requests (non-blocking, bounded).
    fn process_api_requests(&mut self) {
        // Take the receiver temporarily to avoid borrow issues
        let Some(rx) = self.api_request_rx.take() else {
            return;
        };

        // Create handler locally to avoid borrow issues
        // (ApiHandler is stateless, so this is cheap)
        let handler = ApiHandler::new();

        // Process up to MAX_REQUESTS_PER_FRAME requests
        for _ in 0..MAX_REQUESTS_PER_FRAME {
            match rx.try_recv() {
                Ok((request, response_tx)) => {
                    debug!("Processing API request: {}", request.method);

                    // Handle the request
                    let response = handler.handle(request, self);

                    // Send response back
                    if let Err(e) = response_tx.send(response) {
                        warn!("Failed to send API response: {:?}", e);
                    }
                }
                Err(TryRecvError::Empty) => {
                    // No more requests
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel closed, don't put it back
                    warn!("API request channel disconnected");
                    return;
                }
            }
        }

        // Put the receiver back
        self.api_request_rx = Some(rx);
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
            PopupKind::CommandPalette => {
                self.command_palette.filter(&input);
                self.command_palette.results()
            }
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
            PopupKind::CommandPalette => {
                let selected_idx = self
                    .popup
                    .results()
                    .iter()
                    .position(|r| self.popup.selected_result() == Some(r))
                    .unwrap_or(0);

                if let Some(cmd) = self.command_palette.get_command(selected_idx) {
                    let cmd_id = cmd.id.to_string();
                    self.hide_popup();
                    self.execute_command(&cmd_id);
                } else {
                    self.hide_popup();
                }
            }
            PopupKind::ModeSwitcher => {
                // Mode switcher is handled by apply_mode_switch, not execute_popup_action
                self.apply_mode_switch();
            }
            PopupKind::ShellSelector => {
                // Shell selector is handled by apply_shell_selection
                self.apply_shell_selection();
            }
            PopupKind::ShellInstallPrompt => {
                // Just close the install prompt
                self.hide_popup();
            }
            PopupKind::ThemeSelector => {
                // Theme selector is handled by apply_theme_selection
                self.apply_theme_selection();
            }
            PopupKind::ExtensionApproval => {
                // Extension approval is handled by handle_extension_approval
                // Enter key approves, this is called on Enter
                self.handle_extension_approval(true);
            }
        }
    }

    /// Executes a command by its ID.
    fn execute_command(&mut self, command_id: &str) {
        match command_id {
            // File commands
            "file.new" => self.show_popup(PopupKind::CreateFile),
            "file.newFolder" => self.show_popup(PopupKind::CreateFolder),
            "file.open" => self.show_file_browser(),
            "file.save" => {
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                } else {
                    self.set_status("File saved".to_string());
                }
            }
            "file.close" => self.close_current_file(),

            // Edit commands
            "edit.undo" => self.editor.undo(),
            "edit.redo" => self.editor.redo(),
            "edit.copy" => {
                if let Some(text) = self.editor.selected_text() {
                    self.copy_to_clipboard(&text);
                }
            }
            "edit.paste" => {
                if let Some(text) = self.paste_from_clipboard() {
                    self.editor.insert_str(&text);
                }
            }
            "edit.selectAll" => self.editor.select_all(),
            "edit.selectLine" => self.editor.select_line(),
            "edit.duplicateLine" => self.editor.duplicate_line(),
            "edit.deleteLine" => self.editor.delete_line(),
            "edit.moveLineUp" => self.editor.move_line_up(),
            "edit.moveLineDown" => self.editor.move_line_down(),
            "edit.toggleComment" => self.editor.toggle_comment(),
            "edit.indent" => self.editor.indent(),
            "edit.outdent" => self.editor.outdent(),

            // Search commands
            "search.inFile" => self.show_popup(PopupKind::SearchInFile),
            "search.inFiles" => self.show_popup(PopupKind::SearchInFiles),
            "search.files" => self.show_popup(PopupKind::SearchFiles),
            "search.directories" => self.show_popup(PopupKind::SearchDirectories),

            // View commands
            "view.focusTerminal" => self.layout.set_focused(FocusedPane::Terminal),
            "view.focusEditor" => self.layout.set_focused(FocusedPane::Editor),
            "view.toggleFocus" => self.layout.toggle_focus(),
            "view.splitLeft" => self.move_split_left(),
            "view.splitRight" => self.move_split_right(),

            // Terminal commands
            "terminal.new" => self.add_terminal_tab(),
            "terminal.split" => self.split_terminal_horizontal(),
            "terminal.close" => self.close_terminal_tab(),
            "terminal.nextTab" => {
                if let Some(ref mut terminals) = self.terminals {
                    terminals.next_tab();
                }
            }
            "terminal.prevTab" => {
                if let Some(ref mut terminals) = self.terminals {
                    terminals.prev_tab();
                }
            }
            "terminal.selectShell" => self.show_shell_selector(),

            // Theme commands
            "theme.select" => self.show_theme_selector(),
            "theme.dark" => self.set_theme(ThemePreset::Dark),
            "theme.light" => self.set_theme(ThemePreset::Light),
            "theme.dracula" => self.set_theme(ThemePreset::Dracula),
            "theme.gruvbox" => self.set_theme(ThemePreset::Gruvbox),
            "theme.nord" => self.set_theme(ThemePreset::Nord),

            // Extension commands
            "extension.list" => self.show_installed_extensions(),
            "extension.install" => {
                self.set_status("Use CLI: rat ext install <user/repo>".to_string());
            }
            "extension.update" => {
                self.set_status("Use CLI: rat ext update [name]".to_string());
            }
            "extension.remove" => {
                self.set_status("Use CLI: rat ext remove <name>".to_string());
            }

            // Application commands
            "app.quit" => self.running = false,
            "app.commandPalette" => self.show_popup(PopupKind::CommandPalette),
            "app.switchEditorMode" => self.show_mode_switcher(),

            _ => self.set_status(format!("Unknown command: {}", command_id)),
        }
    }

    /// Renders the application.
    pub fn render(&self, frame: &mut ratatui::Frame) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::style::{Color, Style};
        use ratatui::widgets::Clear;

        let area = frame.area();

        // Clear the entire frame first to prevent rendering artifacts
        frame.render_widget(Clear, area);

        // Explicitly reset entire buffer to prevent ghost characters
        // This clears character, style, and all modifiers
        let bg_color = self.config.theme_manager.current().editor.background;
        let clear_style = Style::default().bg(bg_color).fg(Color::Reset);
        let buf = frame.buffer_mut();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_char(' ');
                    cell.set_style(clear_style);
                }
            }
        }

        let areas = self.layout.calculate(area);

        // Render terminal pane (with split support)
        if areas.has_terminal() {
            if let Some(ref terminals) = self.terminals {
                let is_focused = self.layout.focused() == FocusedPane::Terminal;
                let tab_info = terminals.tab_info();

                // Split area for tab bar + terminal content
                let terminal_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(1)])
                    .split(areas.terminal);

                // Render tab bar
                let tab_bar = TerminalTabBar::new(&tab_info).focused(is_focused);
                frame.render_widget(tab_bar, terminal_chunks[0]);

                // Render terminal content in remaining area
                let terminal_area = terminal_chunks[1];

                // Cache the terminal area for mouse coordinate conversion
                self.last_terminal_area.set(terminal_area);

                if let Some(tab) = terminals.active_tab() {
                    let terminal_theme = &self.config.theme_manager.current().terminal;
                    let (grid_cols, grid_rows) = tab.grid.dimensions();
                    let focused_idx = tab.grid.focused_index();

                    match (grid_cols, grid_rows) {
                        (1, 1) => {
                            // Single terminal (no grid split)
                            if let Some(terminal) = tab.grid.get(0) {
                                let widget = TerminalWidget::new(terminal)
                                    .focused(is_focused)
                                    .title(terminal.title())
                                    .theme(terminal_theme);
                                frame.render_widget(widget, terminal_area);
                            }
                        }
                        (2, 1) => {
                            // Two terminals side-by-side (vertical split)
                            let chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(terminal_area);

                            for (i, chunk) in chunks.iter().enumerate() {
                                if let Some(terminal) = tab.grid.get(i) {
                                    let pane_focused = is_focused && focused_idx == i;
                                    let widget = TerminalWidget::new(terminal)
                                        .focused(pane_focused)
                                        .title(terminal.title())
                                        .theme(terminal_theme);
                                    frame.render_widget(widget, *chunk);
                                }
                            }
                        }
                        (2, 2) => {
                            // 2x2 grid
                            let row_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(terminal_area);

                            // Top row
                            let top_cols = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(row_chunks[0]);

                            // Bottom row
                            let bottom_cols = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Percentage(50),
                                    Constraint::Percentage(50),
                                ])
                                .split(row_chunks[1]);

                            // Grid layout: 0=top-left, 1=top-right, 2=bottom-left, 3=bottom-right
                            let all_chunks =
                                [top_cols[0], top_cols[1], bottom_cols[0], bottom_cols[1]];

                            for (i, chunk) in all_chunks.iter().enumerate() {
                                if let Some(terminal) = tab.grid.get(i) {
                                    let pane_focused = is_focused && focused_idx == i;
                                    let widget = TerminalWidget::new(terminal)
                                        .focused(pane_focused)
                                        .title(terminal.title())
                                        .theme(terminal_theme);
                                    frame.render_widget(widget, *chunk);
                                }
                            }
                        }
                        _ => {
                            // Fallback: render focused terminal
                            if let Some(terminal) = tab.grid.focused() {
                                let widget = TerminalWidget::new(terminal)
                                    .focused(is_focused)
                                    .title(terminal.title())
                                    .theme(terminal_theme);
                                frame.render_widget(widget, terminal_area);
                            }
                        }
                    }
                }
            }
        }

        // Render editor or file browser
        if areas.has_editor() {
            let is_focused = self.layout.focused() == FocusedPane::Editor;

            if self.file_browser.is_visible() {
                let widget = FilePickerWidget::new(&self.file_browser).focused(is_focused);
                frame.render_widget(widget, areas.editor);
            } else {
                // Split area for tab bar + editor content
                let editor_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(1)])
                    .split(areas.editor);

                // Render editor tab bar
                let editor_tabs = self.editor_tab_info();
                let tab_bar = EditorTabBar::new(&editor_tabs).focused(is_focused);
                frame.render_widget(tab_bar, editor_chunks[0]);

                // Render editor content
                let widget = EditorWidget::new(&self.editor)
                    .focused(is_focused)
                    .theme(&self.config.theme_manager.current().editor);
                frame.render_widget(widget, editor_chunks[1]);
            }
        }

        // Render status bar
        self.render_status_bar(frame, &areas);

        // Render popup if visible
        if self.popup.is_visible() {
            // Use special widget for mode switcher
            if let Some(ref switcher) = self.mode_switcher {
                let widget = ModeSwitcherWidget::new(switcher);
                frame.render_widget(widget, area);
            } else if let Some(ref selector) = self.shell_selector {
                // Use special widget for shell selector
                let widget = ShellSelectorWidget::new(selector);
                frame.render_widget(widget, area);
            } else if let Some(ref prompt) = self.shell_install_prompt {
                // Use special widget for shell install prompt
                let widget = ShellInstallPromptWidget::new(prompt);
                frame.render_widget(widget, area);
            } else if let Some(ref selector) = self.theme_selector {
                // Use special widget for theme selector
                let widget = ThemeSelectorWidget::new(selector);
                frame.render_widget(widget, area);
            } else {
                let popup_widget = PopupWidget::new(&self.popup);
                frame.render_widget(popup_widget, area);
            }
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

        // Add background process indicators
        status_bar = status_bar.background_processes(
            self.background_running_count(),
            self.background_error_count(),
        );

        frame.render_widget(status_bar, areas.status_bar);
    }

    /// Shuts down the application.
    pub fn shutdown(&mut self) {
        // Shutdown API server first
        if let Some(server) = self.api_server.take() {
            info!("Shutting down API server");
            server.shutdown();
        }

        // Then shutdown terminals
        if let Some(ref mut terminals) = self.terminals {
            terminals.shutdown();
        }
    }

    /// Saves the current session state to disk.
    ///
    /// # Errors
    /// Returns error if save fails.
    pub fn save_session(&self) -> std::io::Result<()> {
        use crate::session::{PersistedFile, Session};

        let mut session = Session::default();

        // Save open files with cursor positions
        for (idx, file) in self.open_files.iter().enumerate() {
            let (cursor_line, cursor_col) = if idx == self.current_file_idx {
                let pos = self.editor.cursor_position();
                (pos.line, pos.col)
            } else {
                (0, 0)
            };

            let scroll_offset = if idx == self.current_file_idx {
                self.editor.view().scroll_top()
            } else {
                0
            };

            session.open_files.push(PersistedFile {
                path: file.path.clone(),
                cursor_line,
                cursor_col,
                modified: idx == self.current_file_idx && self.editor.is_modified(),
                scroll_offset,
            });
        }

        session.active_file_idx = self.current_file_idx;
        session.cwd = self.file_browser.path().to_path_buf();

        session.focused_pane = match self.layout.focused() {
            FocusedPane::Terminal => 0,
            FocusedPane::Editor => 1,
        };

        session.keybinding_mode = match self.config.mode {
            KeybindingMode::Vim => "vim".to_string(),
            KeybindingMode::Emacs => "emacs".to_string(),
            KeybindingMode::VsCode => "vscode".to_string(),
            KeybindingMode::Default => "default".to_string(),
        };

        if let Some(ref terminals) = self.terminals {
            session.terminal_tab_count = terminals.tab_count();
            session.active_terminal_idx = terminals.active_tab_index();
        }

        session.save()
    }

    /// Restores session state from disk if available.
    ///
    /// # Errors
    /// Returns error if restore fails.
    pub fn restore_session(&mut self) -> std::io::Result<bool> {
        use crate::editor::edit::Position;
        use crate::session::Session;

        if !Session::exists() {
            return Ok(false);
        }

        let session = Session::load()?;

        // Restore open files
        for persisted_file in &session.open_files {
            if persisted_file.path.exists() {
                if let Ok(()) = self.open_file(persisted_file.path.clone()) {
                    // File opened successfully
                }
            }
        }

        // Restore active file
        if session.active_file_idx < self.open_files.len() {
            self.current_file_idx = session.active_file_idx;
            if let Some(file) = self.open_files.get(self.current_file_idx) {
                let _ = self.editor.open(&file.path);
            }
        }

        // Restore cursor position for active file if we have one
        if let Some(persisted_file) = session.open_files.get(session.active_file_idx) {
            let pos = Position::new(persisted_file.cursor_line, persisted_file.cursor_col);
            self.editor.set_cursor_position(pos);
            self.editor.goto_line(persisted_file.scroll_offset);
        }

        // Restore working directory
        let _ = self.file_browser.change_dir(&session.cwd);

        // Restore focused pane
        let focused = match session.focused_pane {
            0 => FocusedPane::Terminal,
            _ => FocusedPane::Editor,
        };
        self.layout.set_focused(focused);

        self.set_status("Session restored".to_string());

        Ok(true)
    }

    // ========== Extension Methods ==========

    /// Initializes extensions and discovers installed ones.
    ///
    /// Extensions that are approved will be noted for startup.
    /// Extensions that need approval will queue an approval popup.
    pub fn init_extensions(&mut self) {
        if let Err(e) = self.extension_manager.init() {
            warn!("Failed to initialize extension manager: {}", e);
            return;
        }

        let pending = self.extension_manager.pending_approval();
        let approved = self.extension_manager.approved_extensions();

        info!(
            "Extensions: {} installed, {} approved, {} pending approval",
            self.extension_manager.count(),
            approved.len(),
            pending.len()
        );

        // Show approval popup for first pending extension
        self.show_next_extension_approval();
    }

    /// Shows the approval popup for the next pending extension.
    fn show_next_extension_approval(&mut self) {
        let pending = self.extension_manager.pending_approval();

        if let Some(ext) = pending.first() {
            let prompt = ExtensionApprovalPrompt::new(
                ext.name.clone(),
                ext.version.clone(),
                ext.author().map(String::from),
                ext.description().map(String::from),
                ext.command().unwrap_or("unknown").to_string(),
            );

            self.extension_approval_prompt = Some(prompt);
            self.popup.set_kind(PopupKind::ExtensionApproval);
            self.popup.show();
            self.mode = AppMode::Popup;
        }
    }

    /// Returns the current extension approval prompt if any.
    #[must_use]
    pub fn extension_approval_prompt(&self) -> Option<&ExtensionApprovalPrompt> {
        self.extension_approval_prompt.as_ref()
    }

    /// Handles extension approval response from the user.
    ///
    /// If approved, the extension is marked as approved and can run.
    /// If denied, the extension is skipped.
    /// Then shows the next pending approval if any.
    pub fn handle_extension_approval(&mut self, approved: bool) {
        if let Some(ref prompt) = self.extension_approval_prompt {
            let name = prompt.name().to_string();

            if approved {
                match self.extension_manager.approve(&name) {
                    Ok(()) => {
                        info!("Extension approved: {}", name);
                        self.set_status(format!("Extension '{}' approved", name));
                    }
                    Err(e) => {
                        warn!("Failed to approve extension {}: {}", name, e);
                        self.set_status(format!("Failed to approve '{}': {}", name, e));
                    }
                }
            } else {
                info!("Extension denied: {}", name);
                self.set_status(format!("Extension '{}' denied", name));
            }
        }

        // Clear current prompt
        self.extension_approval_prompt = None;
        self.popup.hide();
        self.mode = AppMode::Normal;

        // Show next pending approval if any
        self.show_next_extension_approval();
    }

    /// Returns the extension manager.
    #[must_use]
    pub fn extension_manager(&self) -> &ExtensionManager {
        &self.extension_manager
    }

    /// Returns a mutable reference to the extension manager.
    pub fn extension_manager_mut(&mut self) -> &mut ExtensionManager {
        &mut self.extension_manager
    }
}
