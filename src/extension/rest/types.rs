//! Request and response types for the REST API.

use serde::{Deserialize, Serialize};

// ============================================================================
// Editor Types
// ============================================================================

/// Response for GET /editor/content
#[derive(Debug, Clone, Serialize)]
pub struct EditorContentResponse {
    pub content: String,
    pub path: Option<String>,
    pub modified: bool,
    pub cursor: CursorPosition,
}

/// Request for PUT /editor/content
#[derive(Debug, Clone, Deserialize)]
pub struct SetContentRequest {
    pub content: String,
}

/// Request for POST /editor/open
#[derive(Debug, Clone, Deserialize)]
pub struct OpenFileRequest {
    pub path: String,
}

/// Request for POST /editor/save
#[derive(Debug, Clone, Deserialize)]
pub struct SaveFileRequest {
    pub path: Option<String>,
}

/// Response for POST /editor/save
#[derive(Debug, Clone, Serialize)]
pub struct SaveFileResponse {
    pub path: String,
}

/// Cursor position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub line: usize,
    pub col: usize,
}

/// Request for PUT /editor/cursor
#[derive(Debug, Clone, Deserialize)]
pub struct SetCursorRequest {
    pub line: usize,
    pub col: usize,
}

/// Request for POST /editor/insert
#[derive(Debug, Clone, Deserialize)]
pub struct InsertTextRequest {
    pub text: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
}

/// Response for GET /editor/file
#[derive(Debug, Clone, Serialize)]
pub struct CurrentFileResponse {
    pub path: Option<String>,
}

// ============================================================================
// Terminal Types
// ============================================================================

/// Request for POST /terminal/send_keys
#[derive(Debug, Clone, Deserialize)]
pub struct SendKeysRequest {
    pub keys: String,
    pub tab: Option<usize>,
}

/// Query params for GET /terminal/buffer
#[derive(Debug, Clone, Deserialize)]
pub struct BufferQuery {
    pub lines: Option<usize>,
    pub offset: Option<usize>,
    pub tab: Option<usize>,
}

/// Response for GET /terminal/buffer
#[derive(Debug, Clone, Serialize)]
pub struct TerminalBufferResponse {
    pub lines: Vec<String>,
    pub cursor: Option<CursorPosition>,
    pub size: TerminalSize,
}

/// Query params for GET /terminal/size
#[derive(Debug, Clone, Deserialize)]
pub struct SizeQuery {
    pub tab: Option<usize>,
}

/// Terminal size
#[derive(Debug, Clone, Serialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

/// Request for POST /terminal/clear
#[derive(Debug, Clone, Deserialize)]
pub struct ClearTerminalRequest {
    pub tab: Option<usize>,
    /// If true, also clear scrollback history
    pub scrollback: Option<bool>,
}

/// Response for GET /terminal/cursor
#[derive(Debug, Clone, Serialize)]
pub struct TerminalCursorResponse {
    pub line: usize,
    pub col: usize,
    pub visible: bool,
}

/// Response for GET /terminal/title
#[derive(Debug, Clone, Serialize)]
pub struct TerminalTitleResponse {
    pub title: String,
}

/// Response for GET /terminal/scrollback
#[derive(Debug, Clone, Serialize)]
pub struct ScrollbackResponse {
    pub lines: Vec<String>,
    pub total_lines: usize,
}

/// Query for GET /terminal/scrollback
#[derive(Debug, Clone, Deserialize)]
pub struct ScrollbackQuery {
    pub tab: Option<usize>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Response for GET /terminal/selection
#[derive(Debug, Clone, Serialize)]
pub struct SelectionResponse {
    pub text: Option<String>,
    pub start: Option<CursorPosition>,
    pub end: Option<CursorPosition>,
}

/// Request for PUT /terminal/selection
#[derive(Debug, Clone, Deserialize)]
pub struct SetSelectionRequest {
    pub tab: Option<usize>,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// Request for POST /terminal/scroll
#[derive(Debug, Clone, Deserialize)]
pub struct ScrollRequest {
    pub tab: Option<usize>,
    /// Number of lines to scroll (positive = down, negative = up)
    pub lines: i32,
}

// ============================================================================
// Filesystem Types
// ============================================================================

/// Query params for filesystem read operations
#[derive(Debug, Clone, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

/// Response for GET /fs/read
#[derive(Debug, Clone, Serialize)]
pub struct ReadFileResponse {
    pub content: String,
}

/// Request for POST /fs/write
#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

/// Response for GET /fs/exists, /fs/is_dir, /fs/is_file
#[derive(Debug, Clone, Serialize)]
pub struct BoolResponse {
    pub result: bool,
}

/// Response for GET /fs/list_dir
#[derive(Debug, Clone, Serialize)]
pub struct ListDirResponse {
    pub entries: Vec<DirEntry>,
}

/// Directory entry
#[derive(Debug, Clone, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: Option<u64>,
}

/// Request for POST /fs/mkdir
#[derive(Debug, Clone, Deserialize)]
pub struct MkdirRequest {
    pub path: String,
}

/// Request for POST /fs/rename
#[derive(Debug, Clone, Deserialize)]
pub struct RenameRequest {
    pub from: String,
    pub to: String,
}

/// Request for POST /fs/copy
#[derive(Debug, Clone, Deserialize)]
pub struct CopyRequest {
    pub from: String,
    pub to: String,
}

/// Generic success response
#[derive(Debug, Clone, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
}

// ============================================================================
// Command Types
// ============================================================================

/// Request for POST /commands/register
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterCommandRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// URL to call when command is executed (extension's HTTP callback)
    pub callback_url: Option<String>,
}

/// Query for DELETE /commands/unregister
#[derive(Debug, Clone, Deserialize)]
pub struct CommandIdQuery {
    pub id: String,
}

/// Response for GET /commands/list
#[derive(Debug, Clone, Serialize)]
pub struct CommandListResponse {
    pub commands: Vec<CommandInfo>,
}

/// Command information
#[derive(Debug, Clone, Serialize)]
pub struct CommandInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
}

/// Request for POST /commands/execute
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteCommandRequest {
    pub id: String,
    pub args: Option<Vec<String>>,
}

/// Response for POST /commands/execute
#[derive(Debug, Clone, Serialize)]
pub struct ExecuteCommandResponse {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

// ============================================================================
// Event Types
// ============================================================================

/// Request for POST /events/subscribe
#[derive(Debug, Clone, Deserialize)]
pub struct SubscribeRequest {
    pub events: Vec<String>,
}

/// Response for POST /events/subscribe
#[derive(Debug, Clone, Serialize)]
pub struct SubscribeResponse {
    pub subscription_id: String,
}

/// Query for DELETE /events/unsubscribe
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionIdQuery {
    pub id: String,
}

/// API Event for SSE streaming
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ApiEvent {
    FileOpen { path: String },
    FileSave { path: String },
    FileClose { path: String },
    FocusChanged { pane: String },
    ThemeChanged { theme: String },
    TerminalOutput { content: String },
    KeyPress { key: String, modifiers: Vec<String> },
    ExtensionLoaded { name: String },
    ExtensionUnloaded { name: String },
}

// ============================================================================
// Layout Types
// ============================================================================

/// Response for GET /layout/state
#[derive(Debug, Clone, Serialize)]
pub struct LayoutStateResponse {
    pub focused: String,
    pub ide_visible: bool,
    pub split_ratio: f32,
}

/// Request for POST /layout/focus
#[derive(Debug, Clone, Deserialize)]
pub struct FocusPaneRequest {
    pub pane: String,
}

/// Response for POST /layout/toggle_ide
#[derive(Debug, Clone, Serialize)]
pub struct ToggleIdeResponse {
    pub visible: bool,
}

/// Request for PUT /layout/split
#[derive(Debug, Clone, Deserialize)]
pub struct SetSplitRequest {
    pub ratio: f32,
}

// ============================================================================
// Tab Types
// ============================================================================

/// Response for GET /tabs/terminal and /tabs/editor
#[derive(Debug, Clone, Serialize)]
pub struct TabListResponse {
    pub tabs: Vec<TabInfo>,
    pub active: usize,
}

/// Tab information
#[derive(Debug, Clone, Serialize)]
pub struct TabInfo {
    pub index: usize,
    pub title: String,
    pub modified: Option<bool>,
}

/// Request for POST /tabs/terminal/new
#[derive(Debug, Clone, Deserialize)]
pub struct NewTerminalRequest {
    pub shell: Option<String>,
}

/// Response for POST /tabs/terminal/new
#[derive(Debug, Clone, Serialize)]
pub struct NewTabResponse {
    pub index: usize,
}

/// Request for POST /tabs/terminal/switch
#[derive(Debug, Clone, Deserialize)]
pub struct SwitchTabRequest {
    pub index: usize,
}

/// Query for DELETE /tabs/terminal/close
#[derive(Debug, Clone, Deserialize)]
pub struct TabIndexQuery {
    pub index: Option<usize>,
}

// ============================================================================
// System Types
// ============================================================================

/// Response for GET /system/version
#[derive(Debug, Clone, Serialize)]
pub struct VersionResponse {
    pub version: String,
}

/// Response for GET /system/status
#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub message: String,
}

/// Request for PUT /system/status
#[derive(Debug, Clone, Deserialize)]
pub struct SetStatusRequest {
    pub message: String,
}

/// Response for GET /system/cwd
#[derive(Debug, Clone, Serialize)]
pub struct CwdResponse {
    pub path: String,
}

/// Query for GET /system/config
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigQuery {
    pub key: String,
}

/// Response for GET /system/config
#[derive(Debug, Clone, Serialize)]
pub struct ConfigResponse {
    pub value: Option<String>,
}

/// Response for GET /system/theme
#[derive(Debug, Clone, Serialize)]
pub struct ThemeResponse {
    pub name: String,
}

/// Request for PUT /system/theme
#[derive(Debug, Clone, Deserialize)]
pub struct SetThemeRequest {
    pub name: String,
}

/// Response for GET /system/themes
#[derive(Debug, Clone, Serialize)]
pub struct ThemeListResponse {
    pub themes: Vec<String>,
    pub current: String,
}

/// Request for POST /system/notify
#[derive(Debug, Clone, Deserialize)]
pub struct NotifyRequest {
    pub message: String,
}

// ============================================================================
// Extension Types
// ============================================================================

/// Response for GET /extensions/list
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionListResponse {
    pub extensions: Vec<ExtensionInfo>,
}

/// Extension information
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    pub status: String,
    pub extension_type: String,
}

/// Request for POST /extensions/reload
#[derive(Debug, Clone, Deserialize)]
pub struct ReloadExtensionRequest {
    pub name: String,
}

/// Response for GET /extensions/health
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

// ============================================================================
// Error Types
// ============================================================================

/// API error response
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: Option<String>,
}

impl ApiError {
    #[must_use]
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: None,
        }
    }

    #[must_use]
    pub fn with_code(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: Some(code.into()),
        }
    }
}
