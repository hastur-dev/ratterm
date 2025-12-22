//! Shared state for the REST API server.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, RwLock};

use super::types::ApiEvent;

/// Shared state for the REST API server.
#[derive(Clone)]
pub struct ApiState {
    /// Channel to send requests to main App thread.
    pub request_tx: mpsc::Sender<AppRequest>,
    /// Broadcast channel for events to all SSE clients.
    pub event_tx: broadcast::Sender<ApiEvent>,
    /// Registered commands from extensions.
    pub commands: Arc<RwLock<CommandRegistry>>,
    /// API token for authentication.
    pub auth_token: String,
    /// Notifications queue.
    pub notifications: Arc<RwLock<Vec<String>>>,
}

impl ApiState {
    /// Creates a new API state.
    #[must_use]
    pub fn new(
        request_tx: mpsc::Sender<AppRequest>,
        event_tx: broadcast::Sender<ApiEvent>,
        auth_token: String,
    ) -> Self {
        Self {
            request_tx,
            event_tx,
            commands: Arc::new(RwLock::new(CommandRegistry::new())),
            auth_token,
            notifications: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Broadcasts an event to all SSE subscribers.
    pub fn broadcast_event(&self, event: ApiEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.event_tx.send(event);
    }

    /// Adds a notification to the queue.
    pub async fn add_notification(&self, message: String) {
        let mut notifications = self.notifications.write().await;
        notifications.push(message);
    }

    /// Takes all pending notifications.
    pub async fn take_notifications(&self) -> Vec<String> {
        let mut notifications = self.notifications.write().await;
        std::mem::take(&mut *notifications)
    }
}

/// Request sent from REST API to main App thread.
#[derive(Debug)]
pub enum AppRequest {
    /// Editor operations.
    Editor(EditorRequest),
    /// Terminal operations.
    Terminal(TerminalRequest),
    /// Layout operations.
    Layout(LayoutRequest),
    /// Tab operations.
    Tab(TabRequest),
    /// System operations.
    System(SystemRequest),
}

/// Editor-related requests.
#[derive(Debug)]
pub enum EditorRequest {
    /// Get editor content.
    GetContent(tokio::sync::oneshot::Sender<EditorContentResult>),
    /// Set editor content.
    SetContent {
        content: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Open a file.
    OpenFile {
        path: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Save current file.
    Save {
        path: Option<String>,
        response: tokio::sync::oneshot::Sender<Result<String, String>>,
    },
    /// Get cursor position.
    GetCursor(tokio::sync::oneshot::Sender<(usize, usize)>),
    /// Set cursor position.
    SetCursor {
        line: usize,
        col: usize,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Insert text.
    InsertText {
        text: String,
        line: Option<usize>,
        col: Option<usize>,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Get current file path.
    GetFile(tokio::sync::oneshot::Sender<Option<String>>),
}

/// Result of getting editor content.
#[derive(Debug)]
pub struct EditorContentResult {
    pub content: String,
    pub path: Option<String>,
    pub modified: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

/// Terminal-related requests.
#[derive(Debug)]
pub enum TerminalRequest {
    /// Send keys to terminal.
    SendKeys {
        keys: String,
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Get terminal buffer.
    GetBuffer {
        lines: Option<usize>,
        offset: Option<usize>,
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<TerminalBufferResult>,
    },
    /// Get terminal size.
    GetSize {
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<(u16, u16)>,
    },
    /// Get terminal cursor position.
    GetCursor {
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<TerminalCursorResult>,
    },
    /// Get terminal title.
    GetTitle {
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<String>,
    },
    /// Clear terminal.
    Clear {
        tab: Option<usize>,
        scrollback: bool,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Get scrollback buffer.
    GetScrollback {
        tab: Option<usize>,
        limit: Option<usize>,
        offset: Option<usize>,
        response: tokio::sync::oneshot::Sender<ScrollbackResult>,
    },
    /// Get terminal selection.
    GetSelection {
        tab: Option<usize>,
        response: tokio::sync::oneshot::Sender<SelectionResult>,
    },
    /// Scroll terminal.
    Scroll {
        tab: Option<usize>,
        lines: i32,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
}

/// Result of getting terminal buffer.
#[derive(Debug)]
pub struct TerminalBufferResult {
    pub lines: Vec<String>,
    pub cursor: Option<(usize, usize)>,
    pub cols: u16,
    pub rows: u16,
}

/// Result of getting terminal cursor.
#[derive(Debug)]
pub struct TerminalCursorResult {
    pub line: usize,
    pub col: usize,
    pub visible: bool,
}

/// Result of getting scrollback.
#[derive(Debug)]
pub struct ScrollbackResult {
    pub lines: Vec<String>,
    pub total_lines: usize,
}

/// Result of getting selection.
#[derive(Debug)]
pub struct SelectionResult {
    pub text: Option<String>,
    pub start: Option<(usize, usize)>,
    pub end: Option<(usize, usize)>,
}

/// Layout-related requests.
#[derive(Debug)]
pub enum LayoutRequest {
    /// Get layout state.
    GetState(tokio::sync::oneshot::Sender<LayoutStateResult>),
    /// Set focused pane.
    SetFocus {
        pane: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Toggle IDE visibility.
    ToggleIde(tokio::sync::oneshot::Sender<bool>),
    /// Set split ratio.
    SetSplit {
        ratio: f32,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
}

/// Result of getting layout state.
#[derive(Debug)]
pub struct LayoutStateResult {
    pub focused: String,
    pub ide_visible: bool,
    pub split_ratio: f32,
}

/// Tab-related requests.
#[derive(Debug)]
pub enum TabRequest {
    /// List terminal tabs.
    ListTerminal(tokio::sync::oneshot::Sender<TabListResult>),
    /// List editor tabs.
    ListEditor(tokio::sync::oneshot::Sender<TabListResult>),
    /// Create new terminal tab.
    NewTerminal {
        shell: Option<String>,
        response: tokio::sync::oneshot::Sender<Result<usize, String>>,
    },
    /// Switch terminal tab.
    SwitchTerminal {
        index: usize,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Close terminal tab.
    CloseTerminal {
        index: Option<usize>,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
}

/// Result of listing tabs.
#[derive(Debug)]
pub struct TabListResult {
    pub tabs: Vec<TabInfoResult>,
    pub active: usize,
}

/// Tab info result.
#[derive(Debug)]
pub struct TabInfoResult {
    pub index: usize,
    pub title: String,
    pub modified: Option<bool>,
}

/// System-related requests.
#[derive(Debug)]
pub enum SystemRequest {
    /// Get version.
    GetVersion(tokio::sync::oneshot::Sender<String>),
    /// Get status message.
    GetStatus(tokio::sync::oneshot::Sender<String>),
    /// Set status message.
    SetStatus {
        message: String,
        response: tokio::sync::oneshot::Sender<()>,
    },
    /// Get current working directory.
    GetCwd(tokio::sync::oneshot::Sender<String>),
    /// Get config value.
    GetConfig {
        key: String,
        response: tokio::sync::oneshot::Sender<Option<String>>,
    },
    /// Get current theme name.
    GetTheme(tokio::sync::oneshot::Sender<String>),
    /// Set theme by name.
    SetTheme {
        name: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// List available themes.
    ListThemes(tokio::sync::oneshot::Sender<(Vec<String>, String)>),
    /// Show notification.
    Notify {
        message: String,
        response: tokio::sync::oneshot::Sender<()>,
    },
}

/// Registry of commands from extensions.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: HashMap<String, RegisteredCommand>,
}

/// A registered command.
#[derive(Debug, Clone)]
pub struct RegisteredCommand {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub callback_url: Option<String>,
    pub source: String,
}

impl CommandRegistry {
    /// Creates a new empty command registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a command.
    pub fn register(
        &mut self,
        id: String,
        name: String,
        description: Option<String>,
        callback_url: Option<String>,
        source: String,
    ) {
        let cmd = RegisteredCommand {
            id: id.clone(),
            name,
            description,
            callback_url,
            source,
        };
        self.commands.insert(id, cmd);
    }

    /// Unregisters a command.
    pub fn unregister(&mut self, id: &str) -> bool {
        self.commands.remove(id).is_some()
    }

    /// Gets a command by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&RegisteredCommand> {
        self.commands.get(id)
    }

    /// Lists all commands.
    #[must_use]
    pub fn list(&self) -> Vec<&RegisteredCommand> {
        self.commands.values().collect()
    }
}
