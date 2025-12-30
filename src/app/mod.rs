//! Main application state and event handling.
//!
//! Orchestrates the terminal emulator, code editor, and file browser.

mod commands;
mod docker_connect;
mod docker_ops;
mod extension_ops;
mod file_ops;
mod input;
mod input_docker;
mod input_editor;
mod input_mouse;
mod input_ssh;
mod input_terminal;
mod keymap;
mod layout_ops;
mod popup_ops;
mod render;
mod session_ops;
mod ssh_connect;
mod ssh_ops;
mod ssh_scan;
mod terminal_ops;

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
use crate::config::{Config, KeybindingMode};
use crate::docker::{DockerItemList, DockerStorage};
use crate::editor::Editor;
use crate::extension::ExtensionManager;
use crate::filebrowser::FileBrowser;
use crate::remote::{RemoteFileBrowser, RemoteFileManager};
use crate::ssh::{NetworkScanner, SSHHostList, SSHStorage};
use crate::terminal::{BackgroundManager, TerminalMultiplexer, pty::PtyError};
use crate::ui::{
    docker_manager::DockerManagerSelector,
    editor_tabs::EditorTabInfo,
    layout::SplitLayout,
    popup::{
        CommandPalette, ExtensionApprovalPrompt, ModeSwitcher, Popup, PopupKind,
        ShellInstallPrompt, ShellSelector, ThemeSelector,
    },
    ssh_manager::SSHManagerSelector,
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
    pub(crate) terminals: Option<TerminalMultiplexer>,
    /// Code editor (right pane).
    pub(crate) editor: Editor,
    /// File browser.
    pub(crate) file_browser: FileBrowser,
    /// Layout manager.
    pub(crate) layout: SplitLayout,
    /// Current app mode.
    pub(crate) mode: AppMode,
    /// Popup dialog.
    pub(crate) popup: Popup,
    /// Command palette for VSCode-style command access.
    pub(crate) command_palette: CommandPalette,
    /// Mode switcher for cycling through editor keybinding modes.
    pub(crate) mode_switcher: Option<ModeSwitcher>,
    /// Shell selector for choosing terminal shell.
    pub(crate) shell_selector: Option<ShellSelector>,
    /// Shell install prompt for unavailable shells.
    pub(crate) shell_install_prompt: Option<ShellInstallPrompt>,
    /// Theme selector for choosing color theme.
    pub(crate) theme_selector: Option<ThemeSelector>,
    /// Open files (tabs).
    pub(crate) open_files: Vec<OpenFile>,
    /// Current file index.
    pub(crate) current_file_idx: usize,
    /// Running flag.
    pub(crate) running: bool,
    /// Status message.
    pub(crate) status: String,
    /// Last error.
    pub(crate) last_error: Option<String>,
    /// Clipboard.
    pub(crate) clipboard: Clipboard,
    /// Configuration.
    pub(crate) config: Config,
    /// Cached terminal area for mouse coordinate conversion.
    pub(crate) last_terminal_area: Cell<Rect>,
    /// Flag to request a full screen redraw.
    pub(crate) needs_redraw: bool,
    /// Flag to request restart after update.
    pub(crate) request_restart_after_update: bool,
    /// API server (runs in background thread).
    pub(crate) api_server: Option<ApiServer>,
    /// API request receiver.
    pub(crate) api_request_rx: Option<RequestReceiver>,
    /// Background process manager.
    pub(crate) background_manager: BackgroundManager,
    /// Extension manager.
    pub(crate) extension_manager: ExtensionManager,
    /// Extension approval prompt.
    pub(crate) extension_approval_prompt: Option<ExtensionApprovalPrompt>,
    /// Last known screen size.
    pub(crate) last_screen_size: (u16, u16),
    /// SSH manager selector state.
    pub(crate) ssh_manager: Option<SSHManagerSelector>,
    /// SSH host storage.
    pub(crate) ssh_storage: SSHStorage,
    /// SSH host list.
    pub(crate) ssh_hosts: SSHHostList,
    /// Network scanner for SSH host discovery.
    pub(crate) ssh_scanner: Option<NetworkScanner>,
    /// Remote file manager for SFTP operations.
    pub(crate) remote_manager: RemoteFileManager,
    /// Remote file browser for SSH directory navigation (active when browsing remote).
    pub(crate) remote_file_browser: Option<RemoteFileBrowser>,
    /// Docker manager selector state.
    pub(crate) docker_manager: Option<DockerManagerSelector>,
    /// Docker storage for quick-connect settings.
    pub(crate) docker_storage: DockerStorage,
    /// Docker items (quick connect slots and settings).
    pub(crate) docker_items: DockerItemList,
}

impl App {
    /// Creates a new application.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let config = Config::load().unwrap_or_default();
        let shell_path = config.shell.get_shell_path();

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

        let layout = if config.ide_always {
            SplitLayout::with_ide_visible()
        } else {
            SplitLayout::new()
        };

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
            last_screen_size: (80, 24),
            ssh_manager: None,
            ssh_storage: SSHStorage::new(),
            ssh_hosts: SSHHostList::new(),
            ssh_scanner: None,
            remote_manager: RemoteFileManager::new(),
            remote_file_browser: None,
            docker_manager: None,
            docker_storage: DockerStorage::new(),
            docker_items: DockerItemList::new(),
        })
    }

    /// Takes the redraw request flag, resetting it to false.
    pub fn take_redraw_request(&mut self) -> bool {
        std::mem::take(&mut self.needs_redraw)
    }

    /// Requests a full screen redraw on the next frame.
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Returns true if the app requested a restart after an update.
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
        self.save_current_file();
        if !self.editor.is_modified() {
            self.running = false;
        }
    }

    /// Sets the status message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
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

    /// Handles terminal resize.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.last_screen_size = (cols, rows);
        self.resize_for_current_layout();
    }

    /// Resizes terminal and editor based on current layout.
    fn resize_for_current_layout(&mut self) {
        let (cols, rows) = self.last_screen_size;
        let areas = self
            .layout
            .calculate(ratatui::layout::Rect::new(0, 0, cols, rows));

        if let Some(ref mut terminals) = self.terminals {
            if areas.has_terminal() {
                let _ = terminals.resize(
                    areas.terminal.width.saturating_sub(2),
                    areas.terminal.height.saturating_sub(3),
                );
            }
        }

        if areas.has_editor() {
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
        self.process_api_requests();
        self.background_manager.update_counts();
        self.poll_ssh_scanner();

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

    /// Processes pending API requests.
    fn process_api_requests(&mut self) {
        let Some(rx) = self.api_request_rx.take() else {
            return;
        };

        let handler = ApiHandler::new();

        for _ in 0..MAX_REQUESTS_PER_FRAME {
            match rx.try_recv() {
                Ok((request, response_tx)) => {
                    debug!("Processing API request: {}", request.method);
                    let response = handler.handle(request, self);
                    if let Err(e) = response_tx.send(response) {
                        warn!("Failed to send API response: {:?}", e);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("API request channel disconnected");
                    return;
                }
            }
        }

        self.api_request_rx = Some(rx);
    }

    /// Shuts down the application.
    pub fn shutdown(&mut self) {
        if let Some(server) = self.api_server.take() {
            info!("Shutting down API server");
            server.shutdown();
        }

        if let Some(ref mut terminals) = self.terminals {
            terminals.shutdown();
        }
    }
}
