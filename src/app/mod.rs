//! Main application state and event handling.
//!
//! Orchestrates the terminal emulator, code editor, and file browser.

mod commands;
pub mod dashboard_hotkeys;
pub mod dashboard_nav;
mod debugger_ops;
mod docker_connect;
mod docker_fleet_ops;
mod docker_logs_ops;
mod docker_ops;
mod extension_ops;
mod file_ops;
mod git_ops;
mod health_ops;
mod input;
mod input_debugger;
mod input_docker;
mod input_docker_create;
mod input_docker_logs;
mod input_editor;
mod input_git;
mod input_health;
pub mod input_k8s;
mod input_lsp;
mod input_mouse;
mod input_ssh;
mod input_terminal;
pub mod input_traits;
mod k8s_ops;
mod key_filter;
mod keymap;
mod layout_ops;
mod lsp_ops;
pub mod lsp_state;
pub mod panel;
mod popup_ops;
mod render;
mod session_ops;
pub mod side_state;
pub mod snapshot;
mod ssh_connect;
mod ssh_ops;
mod ssh_scan;
mod status_sync;
mod terminal_ops;

use std::cell::Cell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event};
use ratatui::layout::Rect;
use tracing::{debug, info, warn};

use self::lsp_state::LspUiState;
use self::side_state::{DebugUiState, GitUiState};
use crate::api::{ApiHandler, ApiServer, ApiServerConfig, MAX_REQUESTS_PER_FRAME, RequestReceiver};
use crate::clipboard::Clipboard;
use crate::completion::CompletionHandle;
use crate::config::{Config, KeybindingMode};
use crate::daemon::DaemonManager;
use crate::debugger::breakpoints::BreakpointStore;
use crate::docker::{DockerFleetState, DockerItemList, DockerStorage};
use crate::editor::{Editor, EditorState};
use crate::extension::ExtensionManager;
use crate::filebrowser::FileBrowser;
use crate::git::dashboard::GitDashboard;
use crate::hosts::HostRegistry;
use crate::remote::{RemoteFileBrowser, RemoteFileManager};
use crate::ssh::{NetworkScanner, SSHStorage, StatusChecker};
use crate::store::RetentionPolicy;
use crate::telemetry::Telemetry;
use crate::terminal::{BackgroundManager, TerminalMultiplexer, pty::PtyError};
use crate::ui::docker_manager::FleetViewState;
use crate::ui::health_dashboard::HealthDashboard;
use crate::ui::k8s_manager::K8sManager;
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
    /// SSH Health Dashboard is active.
    HealthDashboard,
}

/// Context for file browser operations.
///
/// Tracks what the file browser is being used for so we can
/// route the selection appropriately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileBrowserContext {
    /// Normal file opening (default).
    #[default]
    OpenFile,
    /// Selecting a volume mount path for Docker container creation.
    DockerVolumeMount,
}

/// Result from a background Docker operation.
#[derive(Debug, Clone)]
pub enum DockerBackgroundResult {
    /// Image pull completed.
    ImagePulled {
        /// Image name that was pulled.
        image: String,
        /// Whether the operation succeeded.
        success: bool,
        /// Error message if failed.
        error: Option<String>,
    },
}

/// Open file tab.
///
/// A tab owns its document. `saved_state` holds the parked
/// [`EditorState`](crate::editor::EditorState) for every tab except the active
/// one, whose state lives in [`App::editor`] while it is on screen. Switching
/// tabs swaps states instead of re-reading the file, so unsaved edits and undo
/// history survive.
#[derive(Debug)]
pub struct OpenFile {
    /// File path.
    pub path: PathBuf,
    /// Display name.
    pub name: String,
    /// Parked document state; `None` for the tab that is currently active.
    pub(crate) saved_state: Option<EditorState>,
}

impl OpenFile {
    /// Creates a tab entry whose document is currently loaded in the editor.
    #[must_use]
    pub fn active(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            saved_state: None,
        }
    }

    /// Returns true if this tab has unsaved changes.
    ///
    /// Only meaningful for parked tabs; the active tab reports through the
    /// editor, since that is where its buffer lives.
    #[must_use]
    pub fn is_parked_modified(&self) -> bool {
        self.saved_state
            .as_ref()
            .is_some_and(EditorState::is_modified)
    }
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
    /// Command palette for quick command access.
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
    /// SSH hosts, their reachability and their detected capabilities.
    ///
    /// Derefs to the underlying `SSHHostList`, so this is the single owner of
    /// the host data rather than a second copy beside it.
    pub(crate) ssh_hosts: HostRegistry,
    /// Network scanner for SSH host discovery.
    pub(crate) ssh_scanner: Option<NetworkScanner>,
    /// Background TCP status checker for SSH hosts.
    pub(crate) status_checker: Option<StatusChecker>,
    /// Remote file manager for SFTP operations.
    pub(crate) remote_manager: RemoteFileManager,
    /// Remote file browser for SSH directory navigation (active when browsing remote).
    pub(crate) remote_file_browser: Option<RemoteFileBrowser>,
    /// Docker manager selector state.
    pub(crate) docker_manager: Option<DockerManagerSelector>,
    /// Kubernetes screens, which own the cluster connection while they are open.
    pub(crate) k8s_manager: Option<K8sManager>,
    /// Docker connections, container data and lifecycle events, for every host.
    ///
    /// Held whether or not the fleet view is showing: the connections are what
    /// make reopening it fast, and the event subscriptions keep recording
    /// while it is closed.
    pub(crate) docker_fleet: DockerFleetState,
    /// Where the cursor is in the fleet view.
    pub(crate) docker_fleet_view: FleetViewState,
    /// Whether the fleet view is on screen.
    pub(crate) docker_fleet_open: bool,
    /// Docker storage for quick-connect settings.
    pub(crate) docker_storage: DockerStorage,
    /// Docker items (quick connect slots and settings).
    pub(crate) docker_items: DockerItemList,
    /// Context for file browser operations (what the selection is for).
    pub(crate) file_browser_context: FileBrowserContext,
    /// Receiver for background Docker operation results.
    pub(crate) docker_background_rx: Option<Receiver<DockerBackgroundResult>>,
    /// Whether the Windows 11 keybinding notification has been shown.
    pub(crate) win11_notification_shown: bool,
    /// Pairs key releases with their presses so hotkeys fire once per keystroke.
    pub(crate) key_filter: key_filter::KeyEventFilter,
    /// Completion handle for autocomplete functionality.
    pub(crate) completion_handle: Option<CompletionHandle>,
    /// Current completion suggestion text for rendering.
    pub(crate) completion_suggestion: Option<String>,
    /// SSH health dashboard for monitoring device metrics.
    pub(crate) health_dashboard: Option<HealthDashboard>,
    /// Daemon manager for real-time metrics collection.
    pub(crate) daemon_manager: Option<DaemonManager>,
    /// Cached host connection statuses from daemon/health metrics.
    ///
    /// Persists across SSH manager open/close cycles so statuses
    /// survive navigation between dashboards.
    pub(crate) host_statuses: HashMap<u32, crate::ssh::ConnectionStatus>,
    /// Metric history, alert evaluation and the live per-host view.
    ///
    /// Both collectors — the SSH poller and the push daemon — hand their
    /// samples to this, so the dashboard and the database see one stream
    /// rather than two that disagree.
    pub(crate) telemetry: Telemetry,
    /// Whether --test-keys mode is active (F1/F2/F3 open palette/SSH/Docker).
    pub(crate) test_keys: bool,
    /// The fixture directory, when the run was given one.
    ///
    /// While set, the application neither reads nor writes the user's real
    /// configuration, which is what makes a scripted run repeatable and keeps
    /// it away from real machines. The path is kept rather than only a flag so
    /// a screen can look for its own fixture file in it.
    pub(crate) fixture_dir: Option<PathBuf>,
    /// Hotkey help overlay (shown with `?` in dashboards).
    pub(crate) hotkey_overlay: Option<crate::ui::hotkey_overlay::HotkeyOverlay>,
    /// Active Docker log stream handle.
    pub(crate) docker_log_stream: Option<crate::docker_logs::log_stream::LogStream>,
    /// Receiver for Docker log entries from the streaming task.
    pub(crate) docker_log_rx:
        Option<tokio::sync::mpsc::Receiver<crate::docker_logs::types::LogEntry>>,
    /// Git dashboard state.
    pub(crate) git_dashboard: Option<GitDashboard>,
    /// Gutter marks and blame for the file on screen.
    ///
    /// One value rather than three loose fields: blame used to be a flag and a
    /// vector that could disagree, so a failed load could leave the previous
    /// file's blame beside the current file's text.
    pub(crate) git: GitUiState,
    /// The debug session, its breakpoints and its panel.
    pub(crate) debug: DebugUiState,
    /// Everything the language-server screens keep between key presses.
    ///
    /// This was nineteen fields — five panels' worth of items, indices and
    /// scroll offsets, plus hover, signature help and rename. They are one
    /// value now, and the panels share one tested implementation of selection
    /// and scrolling rather than five hand-written ones.
    pub(crate) lsp: LspUiState,
}

/// Construction options for [`App`].
///
/// The defaults reproduce the interactive application. Tests and headless runs
/// switch pieces off so several instances can exist at once without competing
/// for a shell or for the single IPC endpoint.
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// Do not spawn the PTY terminal multiplexer.
    pub without_terminals: bool,
    /// Do not start the IPC API server.
    pub without_api: bool,
    /// Load configuration from disk (`false` uses built-in defaults).
    pub load_user_config: bool,
    /// Explicit control-API configuration; `None` mints a token and listens on
    /// the platform default endpoint.
    pub api_config: Option<ApiServerConfig>,
    /// Load hosts, Docker entries and metrics from this directory instead of
    /// the user's configuration, and cut off remote access.
    pub fixtures: Option<PathBuf>,
}

impl AppOptions {
    /// Options for the interactive application.
    #[must_use]
    pub fn interactive() -> Self {
        Self {
            without_terminals: false,
            without_api: false,
            load_user_config: true,
            api_config: None,
            fixtures: None,
        }
    }

    /// Options for a self-contained instance with no shell, no IPC endpoint,
    /// and no dependency on the developer's `~/.ratrc`.
    #[must_use]
    pub fn isolated() -> Self {
        Self {
            without_terminals: true,
            without_api: true,
            load_user_config: false,
            api_config: None,
            fixtures: None,
        }
    }

    /// Sets an explicit control-API configuration.
    #[must_use]
    pub fn with_api(mut self, config: ApiServerConfig) -> Self {
        self.without_api = false;
        self.api_config = Some(config);
        self
    }

    /// Loads state from a fixture directory instead of the real configuration.
    #[must_use]
    pub fn with_fixtures(mut self, dir: impl Into<PathBuf>) -> Self {
        self.fixtures = Some(dir.into());
        self
    }
}

impl App {
    /// Creates a new application.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        Self::with_options(cols, rows, AppOptions::interactive())
    }

    /// Creates an application with no shell and no IPC endpoint.
    ///
    /// Used by tests and by headless runs, where spawning a shell and claiming
    /// the single API endpoint would make instances interfere with each other.
    ///
    /// # Errors
    /// Returns error if construction fails.
    pub fn isolated(cols: u16, rows: u16) -> Result<Self, PtyError> {
        Self::with_options(cols, rows, AppOptions::isolated())
    }

    /// Creates a new application with explicit options.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn with_options(cols: u16, rows: u16, options: AppOptions) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let config = if options.load_user_config {
            Config::load().unwrap_or_default()
        } else {
            Config::default()
        };
        let lsp_format_on_save = config.lsp_format_on_save;
        let shell_path = config.shell.get_shell_path();

        let terminals = if options.without_terminals {
            None
        } else {
            match TerminalMultiplexer::with_shell(cols / 2, rows.saturating_sub(4), shell_path) {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!("Failed to create terminal: {}", e);
                    None
                }
            }
        };

        let editor = Editor::new(cols / 2, rows.saturating_sub(4));
        let file_browser = FileBrowser::default();
        let cwd = file_browser.path().to_path_buf();

        let layout = if config.ide_always {
            SplitLayout::with_ide_visible()
        } else {
            SplitLayout::new()
        };

        let mut ssh_storage = SSHStorage::new();
        if let Err(e) = ssh_storage.set_mode(config.ssh_storage_mode) {
            warn!("Falling back to the default secret backend: {}", e);
        }

        let mut ssh_hosts = HostRegistry::new();
        let fixture_dir = options.fixtures.clone();
        let fixture_mode = fixture_dir.is_some();
        if let Some(dir) = options.fixtures.as_ref() {
            match crate::fixtures::Fixtures::load(dir) {
                Ok(fixtures) => {
                    info!(
                        "using fixtures from {} ({} hosts)",
                        dir.display(),
                        fixtures.host_count()
                    );
                    ssh_hosts.set_hosts(fixtures.hosts);
                    // A fixture run must not be able to reach a real machine,
                    // and must not write over the user's host file.
                    crate::fixtures::Fixtures::isolate_remote_access();
                    let scratch = std::env::temp_dir()
                        .join("ratterm-fixture-run")
                        .join("ssh_hosts.toml");
                    ssh_storage = SSHStorage::with_path(scratch);
                }
                Err(e) => warn!("could not load fixtures from {}: {}", dir.display(), e),
            }
        }

        let (api_server, api_request_rx) = if options.without_api {
            (None, None)
        } else {
            // The endpoint is authenticated by default: it can drive the editor
            // and inject keystrokes into the shell, so an open one is a local
            // privilege escalation.
            let api_config = match options.api_config.clone() {
                Some(config) => Ok(config),
                None => ApiServerConfig::secure_default(),
            };

            match api_config.and_then(ApiServer::start_with) {
                Ok((server, rx)) => {
                    info!("API server started on {}", server.endpoint());
                    (Some(server), Some(rx))
                }
                Err(e) => {
                    warn!("Failed to start API server: {}", e);
                    (None, None)
                }
            }
        };

        let telemetry = Self::build_telemetry(&config, fixture_mode);

        // A setting that silently did nothing used to be invisible. Say so
        // where the user is looking, not only in a log file.
        let status = config.issue_summary().unwrap_or_default();

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
            status,
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
            ssh_storage,
            ssh_hosts,
            ssh_scanner: None,
            status_checker: None,
            remote_manager: RemoteFileManager::new(),
            remote_file_browser: None,
            docker_manager: None,
            k8s_manager: None,
            docker_fleet: if fixture_mode {
                // A scripted run must not write to the user's event history.
                DockerFleetState::live_only()
            } else {
                DockerFleetState::with_history()
            },
            docker_fleet_view: FleetViewState::new(),
            docker_fleet_open: false,
            docker_storage: DockerStorage::new(),
            docker_items: DockerItemList::new(),
            file_browser_context: FileBrowserContext::OpenFile,
            docker_background_rx: None,
            win11_notification_shown: false,
            key_filter: key_filter::KeyEventFilter::new(),
            completion_handle: Some(CompletionHandle::new(cwd.clone())),
            completion_suggestion: None,
            health_dashboard: None,
            daemon_manager: None,
            host_statuses: HashMap::new(),
            telemetry,
            test_keys: false,
            fixture_dir,
            hotkey_overlay: None,
            docker_log_stream: None,
            docker_log_rx: None,
            git_dashboard: None,
            git: GitUiState::new(),
            debug: DebugUiState::new(BreakpointStore::with_project_root(cwd)),
            lsp: LspUiState::new(lsp_format_on_save),
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

    /// Returns the current application mode.
    #[must_use]
    pub const fn mode(&self) -> AppMode {
        self.mode
    }

    /// Returns the last known screen size.
    #[must_use]
    pub const fn screen_size(&self) -> (u16, u16) {
        self.last_screen_size
    }

    /// Returns the host registry.
    #[must_use]
    pub const fn host_registry(&self) -> &HostRegistry {
        &self.ssh_hosts
    }

    /// Returns true if this instance is running on fixture state.
    #[must_use]
    pub const fn is_fixture_mode(&self) -> bool {
        self.fixture_dir.is_some()
    }

    /// Returns the fixture directory, when the run was given one.
    #[must_use]
    pub fn fixture_dir(&self) -> Option<&Path> {
        self.fixture_dir.as_deref()
    }

    /// Builds the telemetry layer this instance will use.
    ///
    /// A fixture run is always live-only: a scripted run must not append to the
    /// user's real history, and a test that shares a database with the previous
    /// test is not a test. Otherwise history is opened only when the config
    /// asks for it, and a failure to open degrades to live-only rather than
    /// stopping start-up — a broken database is not a reason to lose the
    /// terminal.
    fn build_telemetry(config: &Config, fixture_mode: bool) -> Telemetry {
        let mut telemetry = if fixture_mode || !config.metrics_history {
            Telemetry::in_memory_only()
        } else {
            Telemetry::open_or_live_only()
        };

        telemetry.set_rules(config.alerts.to_rules());
        telemetry.set_policy(RetentionPolicy {
            raw_for: Duration::from_secs(u64::from(config.metrics_raw_days) * 24 * 60 * 60),
            ..RetentionPolicy::default()
        });
        telemetry
    }

    /// Returns the telemetry layer.
    #[must_use]
    pub const fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// Returns a mutable telemetry layer.
    pub const fn telemetry_mut(&mut self) -> &mut Telemetry {
        &mut self.telemetry
    }

    /// Returns the control-API endpoint, if the server started.
    #[must_use]
    pub fn api_endpoint(&self) -> Option<&str> {
        self.api_server.as_ref().map(ApiServer::endpoint)
    }

    /// Returns a mutable host registry.
    pub fn host_registry_mut(&mut self) -> &mut HostRegistry {
        &mut self.ssh_hosts
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

    /// Triggers a completion request based on current editor state.
    pub fn trigger_completion(&mut self) {
        use crate::completion::CompletionContext;
        use crate::completion::lsp::detect_language;

        let Some(ref handle) = self.completion_handle else {
            return;
        };

        let cursor = self.editor.cursor_position();
        let line_content = self.editor.buffer().line(cursor.line).unwrap_or_default();
        let prefix = if cursor.col <= line_content.len() {
            line_content[..cursor.col].to_string()
        } else {
            line_content.clone()
        };

        let language_id = self
            .editor
            .path()
            .and_then(|p| detect_language(p))
            .unwrap_or_else(|| "text".to_string());

        let context = CompletionContext::new(&language_id, cursor.line, cursor.col)
            .with_file_path(self.editor.path().cloned().unwrap_or_default())
            .with_line_content(&line_content)
            .with_prefix(&prefix)
            .with_word_at_cursor(self.editor.word_at_cursor().unwrap_or_default())
            .with_buffer_content(self.editor.buffer().text());

        handle.trigger(context);
    }

    /// Accepts the current completion suggestion.
    pub fn accept_completion(&mut self) -> bool {
        let Some(ref handle) = self.completion_handle else {
            return false;
        };

        if let Some(text) = handle.accept() {
            // Get the word at cursor to determine how much to replace
            let word = self.editor.word_at_cursor().unwrap_or_default();

            // Extract just the part after the current word (case-insensitive prefix match)
            let insert_text = if !word.is_empty()
                && (text.starts_with(&word)
                    || text.to_lowercase().starts_with(&word.to_lowercase()))
            {
                text[word.len()..].to_string()
            } else {
                text
            };

            if !insert_text.is_empty() {
                self.editor.insert_str(&insert_text);
                self.completion_suggestion = None;
                self.set_status("Accepted completion");
                return true;
            }
        }
        false
    }

    /// Dismisses the current completion suggestion.
    pub fn dismiss_completion(&mut self) {
        if let Some(ref handle) = self.completion_handle {
            handle.dismiss();
        }
        self.completion_suggestion = None;
    }

    /// Updates the completion suggestion from the handle.
    pub fn update_completion_suggestion(&mut self) {
        if let Some(ref handle) = self.completion_handle {
            self.completion_suggestion = handle.suggestion_text();
        }
    }

    /// Returns the current completion suggestion text.
    #[must_use]
    pub fn completion_suggestion(&self) -> Option<&str> {
        self.completion_suggestion.as_deref()
    }

    /// Returns true if the app is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Requests to quit the application.
    ///
    /// Any tab with unsaved changes blocks the exit, not just the visible one.
    pub fn request_quit(&mut self) {
        if self.editor.is_modified() || self.any_tab_modified() {
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

    /// Enables test-keys mode (F1/F2/F3 open palette/SSH/Docker).
    pub fn enable_test_keys(&mut self) {
        self.test_keys = true;
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
                is_modified: self.tab_is_modified(i),
            })
            .collect()
    }

    /// Returns true if the tab at `index` has unsaved changes.
    ///
    /// The active tab's buffer lives in the editor; every other tab carries its
    /// own parked state.
    #[must_use]
    pub fn tab_is_modified(&self, index: usize) -> bool {
        match self.open_files.get(index) {
            None => false,
            Some(_) if index == self.current_file_idx => self.editor.is_modified(),
            Some(file) => file.is_parked_modified(),
        }
    }

    /// Returns true if any open tab has unsaved changes.
    #[must_use]
    pub fn any_tab_modified(&self) -> bool {
        (0..self.open_files.len()).any(|i| self.tab_is_modified(i))
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

        if let Some(ref mut terminals) = self.terminals
            && areas.has_terminal()
        {
            let term_cols = areas.terminal.width.saturating_sub(2);
            let term_rows = areas.terminal.height.saturating_sub(3);
            tracing::debug!(
                "RESIZE_LAYOUT: screen={}x{}, terminal_area=({}, {}, {}x{}), resizing_grid_to={}x{}",
                cols,
                rows,
                areas.terminal.x,
                areas.terminal.y,
                areas.terminal.width,
                areas.terminal.height,
                term_cols,
                term_rows
            );
            let _ = terminals.resize(term_cols, term_rows);
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

    /// Advances every background task by one step.
    ///
    /// Separate from [`App::update`] because a headless run and the scenario
    /// runner need the state machine to move without a terminal to read
    /// events from.
    pub fn tick(&mut self) {
        self.process_api_requests();
        self.background_manager.update_counts();
        self.poll_ssh_scanner();
        self.poll_status_checker();
        self.poll_health_dashboard();
        self.poll_docker_log_stream();
        self.update_completion_suggestion();
        // Cheap: returns immediately unless an hour has passed.
        self.telemetry.maybe_downsample();
        // Drains the container event feed. Never blocks, and caps itself.
        self.docker_fleet.pump_events();

        if !self.file_browser.is_visible()
            && !self.is_health_dashboard_open()
            && let Some(ref mut terminals) = self.terminals
        {
            if let Err(e) = terminals.process_all() {
                self.last_error = Some(format!("Terminal error: {}", e));
            }
            // Check for clipboard content from OSC 52 (e.g., from SSH/vim/tmux)
            if let Some(content) = terminals.take_pending_clipboard() {
                self.copy_to_clipboard(&content);
                self.set_status("Copied from remote");
            }
        }
    }

    /// Processes events and updates state.
    ///
    /// # Errors
    /// Returns error if event processing fails.
    pub fn update(&mut self) -> io::Result<()> {
        self.tick();

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

    /// Marks the Windows 11 keybinding notification as shown.
    pub fn mark_win11_notification_shown(&mut self) {
        self.win11_notification_shown = true;
        // Persist this to a marker file so it's not shown again
        if let Some(data_dir) = dirs::data_local_dir() {
            let marker_path = data_dir.join("ratterm").join(".win11_notification_shown");
            if let Some(parent) = marker_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&marker_path, "1");
        }
    }

    /// Checks if Windows 11 keybinding notification should be shown.
    pub fn should_show_win11_notification(&self) -> bool {
        use crate::config::is_windows_11;

        if !is_windows_11() || self.win11_notification_shown {
            return false;
        }

        // Check if marker file exists
        if let Some(data_dir) = dirs::data_local_dir() {
            let marker_path = data_dir.join("ratterm").join(".win11_notification_shown");
            if marker_path.exists() {
                return false;
            }
        }

        true
    }

    /// Shows the Windows 11 keybinding notification if needed.
    pub fn check_win11_notification(&mut self) {
        if self.should_show_win11_notification() {
            self.show_popup(PopupKind::KeybindingChangeNotification);
            self.win11_notification_shown = true;
        }
    }
}
