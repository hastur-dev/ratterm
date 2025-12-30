//! API protocol definitions.
//!
//! JSON-RPC style request/response protocol for AI control.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// API request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// Request ID for correlation.
    pub id: String,
    /// Method name (e.g., "terminal.send_keys").
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: Value,
}

/// API response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Request ID for correlation.
    pub id: String,
    /// Result on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorResponse>,
}

/// Error response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
}

impl ApiResponse {
    /// Creates a success response.
    #[must_use]
    pub fn success(id: String, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error response.
    #[must_use]
    pub fn error(id: String, code: i32, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(ApiErrorResponse {
                code,
                message: message.into(),
            }),
        }
    }
}

// ============================================================================
// Terminal operation parameters
// ============================================================================

/// Parameters for terminal.send_keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendKeysParams {
    /// Keys/text to send (can include escape sequences).
    pub keys: String,
    /// Optional tab index (default: active tab).
    #[serde(default)]
    pub tab_index: Option<usize>,
}

/// Parameters for terminal.read_buffer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadBufferParams {
    /// Optional tab index (default: active tab).
    #[serde(default)]
    pub tab_index: Option<usize>,
    /// Number of lines to read (default: all visible).
    #[serde(default)]
    pub lines: Option<usize>,
    /// Line offset from top (default: 0).
    #[serde(default)]
    pub offset: Option<usize>,
}

/// Result for terminal.read_buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBufferResult {
    /// Terminal content lines.
    pub lines: Vec<String>,
    /// Cursor position.
    pub cursor: CursorPosition,
    /// Terminal size.
    pub size: TerminalSize,
}

/// Parameters for terminal.execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteParams {
    /// Command to execute.
    pub command: String,
    /// Optional tab index (default: active tab).
    #[serde(default)]
    pub tab_index: Option<usize>,
    /// Timeout in milliseconds (default: 5000).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

/// Result for terminal.get_size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSize {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

/// Cursor position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    /// Column (0-indexed).
    pub col: u16,
    /// Row (0-indexed).
    pub row: u16,
}

// ============================================================================
// Editor operation parameters
// ============================================================================

/// Parameters for editor.open_file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFileParams {
    /// File path to open.
    pub path: String,
}

/// Result for editor.read_content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorContentResult {
    /// File content.
    pub content: String,
    /// File path (if saved).
    pub path: Option<String>,
    /// Whether file has unsaved changes.
    pub modified: bool,
    /// Cursor position.
    pub cursor: CursorPosition,
}

/// Parameters for editor.write_content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteContentParams {
    /// Content to write.
    pub content: String,
}

/// Parameters for editor.insert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertParams {
    /// Text to insert.
    pub text: String,
    /// Position to insert at (default: cursor position).
    #[serde(default)]
    pub position: Option<CursorPosition>,
}

/// Parameters for editor.save.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveParams {
    /// Optional path to save as.
    #[serde(default)]
    pub path: Option<String>,
}

/// Parameters for editor.set_cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCursorParams {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub col: usize,
}

// ============================================================================
// Layout operation parameters
// ============================================================================

/// Parameters for layout.focus_pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusPaneParams {
    /// Pane to focus: "terminal" or "editor".
    pub pane: String,
}

/// Result for layout.get_state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutStateResult {
    /// Currently focused pane.
    pub focused: String,
    /// Whether IDE pane is visible.
    pub ide_visible: bool,
    /// Split ratio (0.0-1.0, terminal portion).
    pub split_ratio: f32,
}

/// Parameters for layout.resize_split.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeSplitParams {
    /// Split ratio (0.0-1.0, terminal portion).
    pub ratio: f32,
}

// ============================================================================
// Tab operation parameters
// ============================================================================

/// Result for tabs.list_terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalTabsResult {
    /// List of terminal tabs.
    pub tabs: Vec<TerminalTabInfo>,
}

/// Terminal tab information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalTabInfo {
    /// Tab index.
    pub index: usize,
    /// Tab name/title.
    pub name: String,
    /// Whether this tab is active.
    pub active: bool,
}

/// Result for tabs.list_editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTabsResult {
    /// List of editor tabs.
    pub tabs: Vec<EditorTabInfo>,
}

/// Editor tab information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTabInfo {
    /// Tab index.
    pub index: usize,
    /// File name.
    pub name: String,
    /// File path (if saved).
    pub path: Option<String>,
    /// Whether file has unsaved changes.
    pub modified: bool,
    /// Whether this tab is active.
    pub active: bool,
}

/// Parameters for tabs.new_terminal.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewTerminalParams {
    /// Optional shell to use.
    #[serde(default)]
    pub shell: Option<String>,
}

/// Parameters for tabs.switch_terminal or tabs.switch_editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchTabParams {
    /// Tab index to switch to.
    pub index: usize,
}

/// Parameters for tabs.close_terminal or tabs.close_editor.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloseTabParams {
    /// Optional tab index (default: active tab).
    #[serde(default)]
    pub index: Option<usize>,
}

// ============================================================================
// System operation parameters
// ============================================================================

/// Parameters for system.set_cwd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCwdParams {
    /// New working directory path.
    pub path: String,
}

/// Result for system.get_cwd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CwdResult {
    /// Current working directory.
    pub path: String,
}

/// Parameters for system.set_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStatusParams {
    /// Status message to display.
    pub message: String,
}

/// Result for system.get_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResult {
    /// Current status message.
    pub message: String,
}

/// Result for system.get_version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionResult {
    /// Application version.
    pub version: String,
}

/// Parameters for system.quit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuitParams {
    /// Force quit without save prompts.
    #[serde(default)]
    pub force: bool,
}

// ============================================================================
// Theme operation parameters
// ============================================================================

/// Parameters for theme.set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetThemeParams {
    /// Theme name to apply.
    pub name: String,
}

// ============================================================================
// Background process operation parameters
// ============================================================================

/// Parameters for background.start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStartParams {
    /// Command to execute in background.
    pub command: String,
}

/// Result for background.start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStartResult {
    /// Process ID.
    pub id: u64,
}

/// Parameters for background.status or background.output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundProcessParams {
    /// Process ID.
    pub id: u64,
}

/// Background process status values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundStatusValue {
    Running,
    Completed,
    Error,
    Killed,
}

/// Result for background.status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStatusResult {
    /// Process ID.
    pub id: u64,
    /// Command that was executed.
    pub command: String,
    /// Current status.
    pub status: BackgroundStatusValue,
    /// Exit code (if finished).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Error message (if error occurred).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Duration in milliseconds (if finished).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Result for background.output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundOutputResult {
    /// Process ID.
    pub id: u64,
    /// Output content (stdout + stderr combined).
    pub output: String,
    /// Current status.
    pub status: BackgroundStatusValue,
}

/// Result for background.list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundListResult {
    /// List of background processes.
    pub processes: Vec<BackgroundStatusResult>,
    /// Number of currently running processes.
    pub running_count: usize,
    /// Number of processes with errors.
    pub error_count: usize,
}

/// Result for background.kill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundKillResult {
    /// Process ID that was killed.
    pub id: u64,
}

// ============================================================================
// Docker operation parameters
// ============================================================================

/// Result for docker.list_containers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainersResult {
    /// Running containers.
    pub running: Vec<DockerContainerInfo>,
    /// Stopped containers.
    pub stopped: Vec<DockerContainerInfo>,
}

/// Docker container information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainerInfo {
    /// Container ID (short form).
    pub id: String,
    /// Container name.
    pub name: String,
    /// Image name.
    pub image: String,
    /// Container status (running/stopped).
    pub status: String,
    /// Port mappings.
    pub ports: Vec<String>,
}

/// Result for docker.list_images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImagesResult {
    /// Available images.
    pub images: Vec<DockerImageInfo>,
}

/// Docker image information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImageInfo {
    /// Image ID.
    pub id: String,
    /// Repository name.
    pub repository: String,
    /// Image tag.
    pub tag: String,
    /// Image size.
    pub size: String,
}

/// Parameters for docker.exec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerExecParams {
    /// Container ID or name.
    pub container: String,
    /// Shell to use (default: /bin/sh).
    #[serde(default)]
    pub shell: Option<String>,
}

/// Parameters for docker.run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRunParams {
    /// Image name.
    pub image: String,
    /// Optional container name.
    #[serde(default)]
    pub name: Option<String>,
    /// Port mappings (e.g., "8080:80").
    #[serde(default)]
    pub ports: Vec<String>,
    /// Volume mounts (e.g., "/host:/container").
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Environment variables (e.g., "KEY=VALUE").
    #[serde(default)]
    pub env: Vec<String>,
    /// Shell to use (default: /bin/sh).
    #[serde(default)]
    pub shell: Option<String>,
}

/// Parameters for docker.quick_connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerQuickConnectParams {
    /// Slot number (1-9).
    pub slot: usize,
}

/// Parameters for docker.assign_quick_connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerAssignQuickConnectParams {
    /// Slot number (1-9).
    pub slot: usize,
    /// Container ID or image name.
    pub target: String,
    /// Type: "container" or "image".
    pub target_type: String,
}

/// Result for docker.get_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStatusResult {
    /// Whether Docker is available.
    pub available: bool,
    /// Docker version (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Error message (if unavailable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result for docker.quick_connect_slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerQuickConnectSlotsResult {
    /// Assigned quick connect slots.
    pub slots: Vec<DockerQuickConnectSlot>,
}

/// Quick connect slot information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerQuickConnectSlot {
    /// Slot number (1-9).
    pub slot: usize,
    /// Target container ID or image name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Target type: "container" or "image".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = ApiRequest {
            id: "1".to_string(),
            method: "terminal.send_keys".to_string(),
            params: serde_json::json!({"keys": "ls\n"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("terminal.send_keys"));
    }

    #[test]
    fn test_response_success() {
        let resp = ApiResponse::success("1".to_string(), serde_json::json!({"ok": true}));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let resp = ApiResponse::error("1".to_string(), -32601, "Method not found");
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }
}
