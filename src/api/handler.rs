//! API request handler.
//!
//! Dispatches API requests to App operations.

use crate::api::ApiError;
use crate::api::protocol::*;
use crate::app::App;
use crate::docker::DockerDiscovery;
use crate::editor::edit::Position;
use crate::terminal::ProcessStatus;
use crate::ui::layout::FocusedPane;
use serde_json::{Value, json};
use tracing::debug;

/// API request handler.
pub struct ApiHandler;

impl ApiHandler {
    /// Creates a new API handler.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Handles an API request and returns a response.
    pub fn handle(&self, request: ApiRequest, app: &mut App) -> ApiResponse {
        debug!("Handling API request: {}", request.method);

        let result = match request.method.as_str() {
            // Terminal operations
            "terminal.send_keys" => self.handle_terminal_send_keys(&request, app),
            "terminal.read_buffer" => self.handle_terminal_read_buffer(&request, app),
            "terminal.get_size" => self.handle_terminal_get_size(&request, app),

            // Editor operations
            "editor.open_file" => self.handle_editor_open_file(&request, app),
            "editor.read_content" => self.handle_editor_read_content(&request, app),
            "editor.write_content" => self.handle_editor_write_content(&request, app),
            "editor.save" => self.handle_editor_save(&request, app),
            "editor.close" => self.handle_editor_close(&request, app),
            "editor.get_cursor" => self.handle_editor_get_cursor(&request, app),
            "editor.set_cursor" => self.handle_editor_set_cursor(&request, app),

            // Layout operations
            "layout.focus_pane" => self.handle_layout_focus_pane(&request, app),
            "layout.toggle_ide" => self.handle_layout_toggle_ide(&request, app),
            "layout.get_state" => self.handle_layout_get_state(&request, app),
            "layout.resize_split" => self.handle_layout_resize_split(&request, app),

            // Tab operations
            "tabs.list_terminal" => self.handle_tabs_list_terminal(&request, app),
            "tabs.list_editor" => self.handle_tabs_list_editor(&request, app),
            "tabs.new_terminal" => self.handle_tabs_new_terminal(&request, app),
            "tabs.close_terminal" => self.handle_tabs_close_terminal(&request, app),
            "tabs.switch_terminal" => self.handle_tabs_switch_terminal(&request, app),

            // System operations
            "system.get_cwd" => self.handle_system_get_cwd(&request, app),
            "system.set_status" => self.handle_system_set_status(&request, app),
            "system.get_status" => self.handle_system_get_status(&request, app),
            "system.get_version" => self.handle_system_get_version(&request, app),
            "system.quit" => self.handle_system_quit(&request, app),

            // Theme operations
            "theme.get" => self.handle_theme_get(&request, app),
            "theme.set" => self.handle_theme_set(&request, app),
            "theme.list" => self.handle_theme_list(&request, app),

            // Background process operations
            "background.start" => self.handle_background_start(&request, app),
            "background.list" => self.handle_background_list(&request, app),
            "background.status" => self.handle_background_status(&request, app),
            "background.output" => self.handle_background_output(&request, app),
            "background.kill" => self.handle_background_kill(&request, app),
            "background.clear" => self.handle_background_clear(&request, app),

            // Docker operations
            "docker.status" => self.handle_docker_status(&request, app),
            "docker.list_containers" => self.handle_docker_list_containers(&request, app),
            "docker.list_images" => self.handle_docker_list_images(&request, app),
            "docker.open_manager" => self.handle_docker_open_manager(&request, app),
            "docker.close_manager" => self.handle_docker_close_manager(&request, app),
            "docker.refresh" => self.handle_docker_refresh(&request, app),
            "docker.exec" => self.handle_docker_exec(&request, app),
            "docker.run" => self.handle_docker_run(&request, app),
            "docker.quick_connect" => self.handle_docker_quick_connect(&request, app),
            "docker.quick_connect_slots" => self.handle_docker_quick_connect_slots(&request, app),
            "docker.stats" => self.handle_docker_stats(&request, app),
            "docker.logs" => self.handle_docker_logs(&request, app),

            _ => Err(ApiError::MethodNotFound(request.method.clone())),
        };

        match result {
            Ok(value) => ApiResponse::success(request.id, value),
            Err(e) => ApiResponse::error(request.id, e.to_error_code(), e.to_string()),
        }
    }

    // ========================================================================
    // Terminal operations
    // ========================================================================

    fn handle_terminal_send_keys(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: SendKeysParams = serde_json::from_value(request.params.clone())?;

        let terminal = app
            .active_terminal_mut()
            .ok_or(ApiError::NoActiveTerminal)?;

        // Send keys as bytes to the PTY
        terminal
            .write(params.keys.as_bytes())
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(json!({}))
    }

    fn handle_terminal_read_buffer(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: ReadBufferParams = serde_json::from_value(request.params.clone())?;

        let terminal = app.active_terminal().ok_or(ApiError::NoActiveTerminal)?;

        let grid = terminal.grid();
        let (cols, rows) = grid.size();
        let (cursor_col, cursor_row) = grid.cursor_pos();

        // Get lines from the grid
        let offset = params.offset.unwrap_or(0);
        let limit = params.lines.unwrap_or(rows as usize);

        let lines: Vec<String> = (offset..offset + limit)
            .filter_map(|row_idx| {
                if row_idx < rows as usize {
                    Some(grid.row_text(row_idx))
                } else {
                    None
                }
            })
            .collect();

        let result = ReadBufferResult {
            lines,
            cursor: CursorPosition {
                col: cursor_col,
                row: cursor_row,
            },
            size: TerminalSize { cols, rows },
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_terminal_get_size(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let terminal = app.active_terminal().ok_or(ApiError::NoActiveTerminal)?;

        let grid = terminal.grid();
        let (cols, rows) = grid.size();

        Ok(json!({
            "cols": cols,
            "rows": rows
        }))
    }

    // ========================================================================
    // Editor operations
    // ========================================================================

    fn handle_editor_open_file(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: OpenFileParams = serde_json::from_value(request.params.clone())?;

        let path = std::path::PathBuf::from(&params.path);
        if !path.exists() {
            return Err(ApiError::FileNotFound(params.path));
        }

        app.open_file(&path)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Show IDE if not visible
        if !app.layout().ide_visible() {
            app.layout_mut().show_ide();
        }

        Ok(json!({}))
    }

    fn handle_editor_read_content(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let editor = app.editor();
        let content = editor.buffer().text();
        let path = app
            .current_file_path()
            .map(|p| p.to_string_lossy().into_owned());
        let modified = app.is_file_modified();
        let pos = editor.cursor_position();

        let result = EditorContentResult {
            content,
            path,
            modified,
            cursor: CursorPosition {
                col: pos.col as u16,
                row: pos.line as u16,
            },
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_editor_write_content(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: WriteContentParams = serde_json::from_value(request.params.clone())?;

        // Select all text and replace it with new content
        app.editor_mut().select_all();
        app.editor_mut().delete_selection();
        app.editor_mut().insert_str(&params.content);

        Ok(json!({}))
    }

    fn handle_editor_save(&self, request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let params: SaveParams = serde_json::from_value(request.params.clone()).unwrap_or_default();

        let path = if let Some(p) = params.path {
            Some(std::path::PathBuf::from(p))
        } else {
            app.current_file_path().map(|p| p.to_path_buf())
        };

        if let Some(path) = path {
            app.save_file(&path)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            Ok(json!({ "path": path.to_string_lossy() }))
        } else {
            Err(ApiError::InvalidParams("No file path specified".into()))
        }
    }

    fn handle_editor_close(&self, _request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        app.close_current_file();
        Ok(json!({}))
    }

    fn handle_editor_get_cursor(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let pos = app.editor().cursor_position();
        Ok(json!({
            "line": pos.line,
            "col": pos.col
        }))
    }

    fn handle_editor_set_cursor(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: SetCursorParams = serde_json::from_value(request.params.clone())?;
        let pos = Position::new(params.line, params.col);
        app.editor_mut().set_cursor_position(pos);
        Ok(json!({}))
    }

    // ========================================================================
    // Layout operations
    // ========================================================================

    fn handle_layout_focus_pane(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: FocusPaneParams = serde_json::from_value(request.params.clone())?;

        match params.pane.to_lowercase().as_str() {
            "terminal" => app.layout_mut().set_focused(FocusedPane::Terminal),
            "editor" => {
                if !app.layout().ide_visible() {
                    app.layout_mut().show_ide();
                }
                app.layout_mut().set_focused(FocusedPane::Editor);
            }
            _ => {
                return Err(ApiError::InvalidParams(format!(
                    "Invalid pane: {}",
                    params.pane
                )));
            }
        }

        Ok(json!({}))
    }

    fn handle_layout_toggle_ide(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.layout_mut().toggle_ide();
        let visible = app.layout().ide_visible();
        Ok(json!({ "visible": visible }))
    }

    fn handle_layout_get_state(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let layout = app.layout();
        let focused = match layout.focused() {
            FocusedPane::Terminal => "terminal",
            FocusedPane::Editor => "editor",
        };

        let result = LayoutStateResult {
            focused: focused.to_string(),
            ide_visible: layout.ide_visible(),
            split_ratio: layout.split_percent() as f32 / 100.0,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_layout_resize_split(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: ResizeSplitParams = serde_json::from_value(request.params.clone())?;

        if !(0.0..=1.0).contains(&params.ratio) {
            return Err(ApiError::InvalidParams(
                "Ratio must be between 0.0 and 1.0".into(),
            ));
        }

        let percent = (params.ratio * 100.0) as u16;
        app.layout_mut().set_split(percent);

        Ok(json!({}))
    }

    // ========================================================================
    // Tab operations
    // ========================================================================

    fn handle_tabs_list_terminal(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let tabs = app.terminal_tabs();

        let result = TerminalTabsResult {
            tabs: tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| TerminalTabInfo {
                    index: i,
                    name: tab.name.clone(),
                    active: tab.active,
                })
                .collect(),
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_tabs_list_editor(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let tabs = app.editor_tabs();

        let result = EditorTabsResult {
            tabs: tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| EditorTabInfo {
                    index: i,
                    name: tab.name.clone(),
                    path: tab.path.clone(),
                    modified: tab.modified,
                    active: tab.active,
                })
                .collect(),
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_tabs_new_terminal(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.add_terminal_tab();

        let tabs = app.terminal_tabs();
        let index = tabs.len().saturating_sub(1);

        Ok(json!({ "index": index }))
    }

    fn handle_tabs_close_terminal(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.close_terminal_tab();
        Ok(json!({}))
    }

    fn handle_tabs_switch_terminal(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: SwitchTabParams = serde_json::from_value(request.params.clone())?;
        app.switch_terminal_tab(params.index);
        Ok(json!({}))
    }

    // ========================================================================
    // System operations
    // ========================================================================

    fn handle_system_get_cwd(
        &self,
        _request: &ApiRequest,
        _app: &mut App,
    ) -> Result<Value, ApiError> {
        let cwd = std::env::current_dir().map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(json!({ "path": cwd.to_string_lossy() }))
    }

    fn handle_system_set_status(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: SetStatusParams = serde_json::from_value(request.params.clone())?;
        app.set_status(&params.message);
        Ok(json!({}))
    }

    fn handle_system_get_status(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let message = app.status().to_string();
        Ok(json!({ "message": message }))
    }

    fn handle_system_get_version(
        &self,
        _request: &ApiRequest,
        _app: &mut App,
    ) -> Result<Value, ApiError> {
        let version = env!("CARGO_PKG_VERSION");
        Ok(json!({ "version": version }))
    }

    fn handle_system_quit(&self, request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let params: QuitParams = serde_json::from_value(request.params.clone()).unwrap_or_default();

        if params.force {
            app.force_quit();
        } else {
            app.request_quit();
        }

        Ok(json!({}))
    }

    // ========================================================================
    // Theme operations
    // ========================================================================

    fn handle_theme_get(&self, _request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let name = app.current_theme_name();
        let preset = app.current_theme_preset().map(|p| p.name().to_string());

        Ok(json!({
            "name": name,
            "preset": preset
        }))
    }

    fn handle_theme_set(&self, request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let params: SetThemeParams = serde_json::from_value(request.params.clone())?;

        // Use the new set_theme_by_name method which supports both presets and custom themes
        match app.set_theme_by_name(&params.name) {
            Ok(()) => Ok(json!({ "success": true, "name": params.name })),
            Err(e) => {
                let available = app.available_themes().join(", ");
                Err(ApiError::InvalidParams(format!(
                    "{}. Available themes: {}",
                    e, available
                )))
            }
        }
    }

    fn handle_theme_list(&self, _request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let available = app.available_themes();
        let current = app.current_theme_name();

        Ok(json!({
            "themes": available,
            "current": current
        }))
    }

    // ========================================================================
    // Background process operations
    // ========================================================================

    fn handle_background_start(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: BackgroundStartParams = serde_json::from_value(request.params.clone())?;

        let id = app
            .start_background_process(&params.command)
            .map_err(ApiError::Internal)?;

        let result = BackgroundStartResult { id };
        Ok(serde_json::to_value(result)?)
    }

    fn handle_background_list(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let (processes, running_count, error_count) = app.list_background_processes();

        let process_results: Vec<BackgroundStatusResult> = processes
            .into_iter()
            .map(|info| {
                let duration_ms = info
                    .finished_at
                    .map(|end| end.duration_since(info.started_at).as_millis() as u64);

                BackgroundStatusResult {
                    id: info.id,
                    command: info.command,
                    status: Self::convert_status(info.status),
                    exit_code: info.exit_code,
                    error_message: info.error_message,
                    duration_ms,
                }
            })
            .collect();

        let result = BackgroundListResult {
            processes: process_results,
            running_count,
            error_count,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_background_status(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: BackgroundProcessParams = serde_json::from_value(request.params.clone())?;

        let info = app
            .get_background_process_info(params.id)
            .ok_or_else(|| ApiError::Internal(format!("Process {} not found", params.id)))?;

        let duration_ms = info
            .finished_at
            .map(|end| end.duration_since(info.started_at).as_millis() as u64);

        let result = BackgroundStatusResult {
            id: info.id,
            command: info.command,
            status: Self::convert_status(info.status),
            exit_code: info.exit_code,
            error_message: info.error_message,
            duration_ms,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_background_output(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: BackgroundProcessParams = serde_json::from_value(request.params.clone())?;

        let info = app
            .get_background_process_info(params.id)
            .ok_or_else(|| ApiError::Internal(format!("Process {} not found", params.id)))?;

        let output = app
            .get_background_process_output(params.id)
            .unwrap_or_default();

        let result = BackgroundOutputResult {
            id: info.id,
            output,
            status: Self::convert_status(info.status),
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_background_kill(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: BackgroundProcessParams = serde_json::from_value(request.params.clone())?;

        app.kill_background_process(params.id)
            .map_err(ApiError::Internal)?;

        let result = BackgroundKillResult { id: params.id };
        Ok(serde_json::to_value(result)?)
    }

    fn handle_background_clear(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.clear_finished_background_processes();
        Ok(json!({}))
    }

    /// Converts internal ProcessStatus to API BackgroundStatusValue.
    fn convert_status(status: ProcessStatus) -> BackgroundStatusValue {
        match status {
            ProcessStatus::Running => BackgroundStatusValue::Running,
            ProcessStatus::Completed => BackgroundStatusValue::Completed,
            ProcessStatus::Error => BackgroundStatusValue::Error,
            ProcessStatus::Killed => BackgroundStatusValue::Killed,
        }
    }

    // ========================================================================
    // Docker operations
    // ========================================================================

    fn handle_docker_status(
        &self,
        _request: &ApiRequest,
        _app: &mut App,
    ) -> Result<Value, ApiError> {
        let available = DockerDiscovery::is_docker_available();
        let version = if available {
            DockerDiscovery::docker_version()
        } else {
            None
        };

        let error = if !available {
            Some("Docker is not available. Ensure Docker is installed and running.".to_string())
        } else {
            None
        };

        let result = DockerStatusResult {
            available,
            version,
            error,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_docker_list_containers(
        &self,
        _request: &ApiRequest,
        _app: &mut App,
    ) -> Result<Value, ApiError> {
        if !DockerDiscovery::is_docker_available() {
            return Err(ApiError::Internal("Docker is not available".into()));
        }

        let (running, stopped) =
            DockerDiscovery::discover_all_containers().map_err(ApiError::Internal)?;

        let running_info: Vec<DockerContainerInfo> = running
            .iter()
            .map(|c| DockerContainerInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                image: c.image.clone(),
                status: c.status.as_str().to_string(),
                ports: c.ports.clone(),
            })
            .collect();

        let stopped_info: Vec<DockerContainerInfo> = stopped
            .iter()
            .map(|c| DockerContainerInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                image: c.image.clone(),
                status: c.status.as_str().to_string(),
                ports: c.ports.clone(),
            })
            .collect();

        let result = DockerContainersResult {
            running: running_info,
            stopped: stopped_info,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_docker_list_images(
        &self,
        _request: &ApiRequest,
        _app: &mut App,
    ) -> Result<Value, ApiError> {
        if !DockerDiscovery::is_docker_available() {
            return Err(ApiError::Internal("Docker is not available".into()));
        }

        let images = DockerDiscovery::discover_images().map_err(ApiError::Internal)?;

        let images_info: Vec<DockerImageInfo> = images
            .iter()
            .map(|i| DockerImageInfo {
                id: i.id.clone(),
                repository: i.repository.clone(),
                tag: i.tag.clone(),
                size: i.size.clone(),
            })
            .collect();

        let result = DockerImagesResult {
            images: images_info,
        };

        Ok(serde_json::to_value(result)?)
    }

    fn handle_docker_open_manager(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.show_docker_manager();
        Ok(json!({}))
    }

    fn handle_docker_close_manager(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.hide_docker_manager();
        Ok(json!({}))
    }

    fn handle_docker_refresh(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        app.refresh_docker_discovery();
        Ok(json!({}))
    }

    fn handle_docker_exec(&self, request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let params: DockerExecParams = serde_json::from_value(request.params.clone())?;

        // Use the container ID as the name if we only have the ID
        let container_name = params.container.clone();
        app.exec_into_container(&params.container, &container_name);

        Ok(json!({ "container": params.container }))
    }

    fn handle_docker_run(&self, request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        let params: DockerRunParams = serde_json::from_value(request.params.clone())?;

        // If custom options are provided, use run_image_with_options
        if params.name.is_some()
            || !params.ports.is_empty()
            || !params.volumes.is_empty()
            || !params.env.is_empty()
        {
            let mut options = crate::docker::DockerRunOptions::new();
            options.name = params.name.clone();
            options.port_mappings = params.ports.clone();
            options.volume_mounts = params.volumes.clone();
            options.env_vars = params.env.clone();
            if let Some(ref shell) = params.shell {
                options.shell = shell.clone();
            }

            app.run_image_with_options(&params.image, &params.image, &options);
        } else {
            // Simple run without options
            app.run_image_interactive(&params.image, &params.image);
        }

        Ok(json!({ "image": params.image }))
    }

    fn handle_docker_quick_connect(
        &self,
        request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let params: DockerQuickConnectParams = serde_json::from_value(request.params.clone())?;

        if params.slot < 1 || params.slot > 9 {
            return Err(ApiError::InvalidParams(
                "Slot must be between 1 and 9".into(),
            ));
        }

        let slot_idx = params.slot - 1;
        app.docker_connect_by_index(slot_idx);

        Ok(json!({ "slot": params.slot }))
    }

    fn handle_docker_quick_connect_slots(
        &self,
        _request: &ApiRequest,
        app: &mut App,
    ) -> Result<Value, ApiError> {
        let docker_items = &app.docker_items;

        let mut slots = Vec::with_capacity(9);
        for i in 0..9 {
            let slot_info = if let Some(item) = docker_items.get_quick_connect(i) {
                DockerQuickConnectSlot {
                    slot: i + 1,
                    target: Some(item.name.clone()),
                    target_type: Some(item.item_type.as_str().to_string()),
                }
            } else {
                DockerQuickConnectSlot {
                    slot: i + 1,
                    target: None,
                    target_type: None,
                }
            };
            slots.push(slot_info);
        }

        let result = DockerQuickConnectSlotsResult { slots };
        Ok(serde_json::to_value(result)?)
    }

    fn handle_docker_stats(&self, _request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        if !app.is_docker_session() {
            return Err(ApiError::InvalidParams(
                "Not in a Docker session. Use docker.exec or docker.run first.".into(),
            ));
        }

        app.show_docker_stats();
        Ok(json!({}))
    }

    fn handle_docker_logs(&self, _request: &ApiRequest, app: &mut App) -> Result<Value, ApiError> {
        if !app.is_docker_session() {
            return Err(ApiError::InvalidParams(
                "Not in a Docker session. Use docker.exec or docker.run first.".into(),
            ));
        }

        app.show_docker_logs();
        Ok(json!({}))
    }
}

impl Default for ApiHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let _handler = ApiHandler::new();
        // Handler should be created without panicking
        assert!(true);
    }
}
