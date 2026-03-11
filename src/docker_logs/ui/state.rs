//! Docker log viewer state machine.
//!
//! Manages the view mode, container list, log buffer, and search state
//! for the Docker log streaming sub-view.

use crate::app::input_traits::ListSelectable;
use crate::docker_logs::config::LogStreamConfig;
use crate::docker_logs::log_buffer::LogBuffer;
use crate::docker_logs::search::SearchManager;
use crate::docker_logs::types::ContainerLogInfo;

/// View mode for the Docker log viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogViewMode {
    /// Selecting a container from the list.
    #[default]
    ContainerList,
    /// Actively streaming logs.
    Streaming,
    /// Stream is paused (scrolling through history).
    Paused,
    /// Searching/filtering log entries.
    Searching,
    /// Viewing saved searches.
    SavedSearches,
}

/// State for the Docker log viewer.
#[derive(Debug, Clone)]
pub struct DockerLogsState {
    /// Current view mode.
    mode: LogViewMode,
    /// Available containers.
    containers: Vec<ContainerLogInfo>,
    /// Selected container index (in ContainerList mode).
    selected_idx: usize,
    /// Log buffer for the active stream.
    log_buffer: LogBuffer,
    /// Search/filter input text.
    search_input: String,
    /// Search cursor position.
    search_cursor: usize,
    /// Saved search manager.
    search_manager: SearchManager,
    /// Selected index in saved searches list.
    saved_search_idx: usize,
    /// Configuration.
    config: LogStreamConfig,
    /// Active container ID (when streaming).
    active_container_id: Option<String>,
    /// Active container name (when streaming).
    active_container_name: Option<String>,
    /// Status message.
    status: Option<String>,
    /// Error message.
    error: Option<String>,
}

impl DockerLogsState {
    /// Creates a new Docker logs state with default config.
    #[must_use]
    pub fn new(config: LogStreamConfig) -> Self {
        let buffer_size = config.buffer_size;
        Self {
            mode: LogViewMode::ContainerList,
            containers: Vec::new(),
            selected_idx: 0,
            log_buffer: LogBuffer::new(buffer_size),
            search_input: String::new(),
            search_cursor: 0,
            search_manager: SearchManager::new(),
            saved_search_idx: 0,
            config,
            active_container_id: None,
            active_container_name: None,
            status: None,
            error: None,
        }
    }

    // ========================================================================
    // Mode accessors
    // ========================================================================

    /// Returns the current view mode.
    #[must_use]
    pub fn mode(&self) -> LogViewMode {
        self.mode
    }

    /// Sets the view mode.
    pub fn set_mode(&mut self, mode: LogViewMode) {
        self.mode = mode;
    }

    // ========================================================================
    // Container list
    // ========================================================================

    /// Sets the container list.
    pub fn set_containers(&mut self, containers: Vec<ContainerLogInfo>) {
        self.containers = containers;
        if self.selected_idx >= self.containers.len() {
            self.selected_idx = self.containers.len().saturating_sub(1);
        }
    }

    /// Returns the container list.
    #[must_use]
    pub fn containers(&self) -> &[ContainerLogInfo] {
        &self.containers
    }

    /// Returns the selected container index.
    #[must_use]
    pub fn selected_idx(&self) -> usize {
        self.selected_idx
    }

    /// Returns the currently selected container.
    #[must_use]
    pub fn selected_container(&self) -> Option<&ContainerLogInfo> {
        self.containers.get(self.selected_idx)
    }

    // ========================================================================
    // State transitions
    // ========================================================================

    /// Transitions to streaming mode for the given container.
    pub fn enter_streaming(&mut self, container_id: String, container_name: String) {
        self.active_container_id = Some(container_id);
        self.active_container_name = Some(container_name);
        self.log_buffer.clear();
        self.log_buffer.resume();
        self.mode = LogViewMode::Streaming;
        self.error = None;
    }

    /// Pauses the stream.
    pub fn pause(&mut self) {
        self.log_buffer.pause();
        self.mode = LogViewMode::Paused;
    }

    /// Resumes the stream.
    pub fn resume(&mut self) {
        self.log_buffer.resume();
        self.mode = LogViewMode::Streaming;
    }

    /// Toggles between paused and streaming.
    pub fn toggle_pause(&mut self) {
        if self.mode == LogViewMode::Paused {
            self.resume();
        } else if self.mode == LogViewMode::Streaming {
            self.pause();
        }
    }

    /// Enters search mode.
    pub fn enter_search(&mut self) {
        self.search_input.clear();
        self.search_cursor = 0;
        self.mode = LogViewMode::Searching;
    }

    /// Exits search mode, applying the filter.
    pub fn exit_search(&mut self) {
        self.log_buffer.set_filter(self.search_input.clone());
        self.mode = if self.log_buffer.is_paused() {
            LogViewMode::Paused
        } else {
            LogViewMode::Streaming
        };
    }

    /// Cancels search mode, clearing the filter.
    pub fn cancel_search(&mut self) {
        self.search_input.clear();
        self.search_cursor = 0;
        self.log_buffer.set_filter(String::new());
        self.mode = if self.log_buffer.is_paused() {
            LogViewMode::Paused
        } else {
            LogViewMode::Streaming
        };
    }

    /// Goes back to the container list.
    pub fn back_to_list(&mut self) {
        self.active_container_id = None;
        self.active_container_name = None;
        self.log_buffer.clear();
        self.search_input.clear();
        self.mode = LogViewMode::ContainerList;
    }

    /// Enters saved searches view.
    pub fn enter_saved_searches(&mut self) {
        self.saved_search_idx = 0;
        self.mode = LogViewMode::SavedSearches;
    }

    /// Exits saved searches view.
    pub fn exit_saved_searches(&mut self) {
        self.mode = if self.active_container_id.is_some() {
            if self.log_buffer.is_paused() {
                LogViewMode::Paused
            } else {
                LogViewMode::Streaming
            }
        } else {
            LogViewMode::ContainerList
        };
    }

    // ========================================================================
    // Log buffer access
    // ========================================================================

    /// Returns a reference to the log buffer.
    #[must_use]
    pub fn log_buffer(&self) -> &LogBuffer {
        &self.log_buffer
    }

    /// Returns a mutable reference to the log buffer.
    pub fn log_buffer_mut(&mut self) -> &mut LogBuffer {
        &mut self.log_buffer
    }

    // ========================================================================
    // Search input
    // ========================================================================

    /// Returns the current search input.
    #[must_use]
    pub fn search_input(&self) -> &str {
        &self.search_input
    }

    /// Inserts a character into the search input at cursor position.
    pub fn search_insert_char(&mut self, c: char) {
        self.search_input.insert(self.search_cursor, c);
        self.search_cursor += c.len_utf8();
        // Live filter update
        self.log_buffer.set_filter(self.search_input.clone());
    }

    /// Deletes the character before the cursor in search input.
    pub fn search_backspace(&mut self) {
        if self.search_cursor > 0 {
            let prev = self.search_input[..self.search_cursor]
                .chars()
                .last()
                .map(char::len_utf8)
                .unwrap_or(1);
            self.search_cursor -= prev;
            self.search_input.remove(self.search_cursor);
            self.log_buffer.set_filter(self.search_input.clone());
        }
    }

    /// Returns the search cursor position.
    #[must_use]
    pub fn search_cursor(&self) -> usize {
        self.search_cursor
    }

    // ========================================================================
    // Saved searches
    // ========================================================================

    /// Returns a reference to the search manager.
    #[must_use]
    pub fn search_manager(&self) -> &SearchManager {
        &self.search_manager
    }

    /// Returns a mutable reference to the search manager.
    pub fn search_manager_mut(&mut self) -> &mut SearchManager {
        &mut self.search_manager
    }

    /// Returns the selected saved search index.
    #[must_use]
    pub fn saved_search_idx(&self) -> usize {
        self.saved_search_idx
    }

    /// Saves the current search input as a saved search.
    pub fn save_current_search(&mut self, name: String) {
        if !self.search_input.is_empty() {
            self.search_manager
                .add(name, self.search_input.clone());
        }
    }

    // ========================================================================
    // Config access
    // ========================================================================

    /// Returns a reference to the config.
    #[must_use]
    pub fn config(&self) -> &LogStreamConfig {
        &self.config
    }

    // ========================================================================
    // Active container
    // ========================================================================

    /// Returns the active container ID.
    #[must_use]
    pub fn active_container_id(&self) -> Option<&str> {
        self.active_container_id.as_deref()
    }

    /// Returns the active container name.
    #[must_use]
    pub fn active_container_name(&self) -> Option<&str> {
        self.active_container_name.as_deref()
    }

    // ========================================================================
    // Status/error
    // ========================================================================

    /// Sets a status message.
    pub fn set_status(&mut self, msg: String) {
        self.status = Some(msg);
    }

    /// Returns the status message.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Sets an error message.
    pub fn set_error(&mut self, msg: String) {
        self.error = Some(msg);
    }

    /// Returns the error message.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Clears the error message.
    pub fn clear_error(&mut self) {
        self.error = None;
    }
}

/// `ListSelectable` implementation for container list navigation.
impl ListSelectable for DockerLogsState {
    fn select_prev(&mut self) {
        match self.mode {
            LogViewMode::ContainerList => {
                self.selected_idx = self.selected_idx.saturating_sub(1);
            }
            LogViewMode::Streaming | LogViewMode::Paused => {
                self.log_buffer.scroll_up(1);
                if self.mode == LogViewMode::Streaming {
                    self.pause();
                }
            }
            LogViewMode::SavedSearches => {
                self.saved_search_idx = self.saved_search_idx.saturating_sub(1);
            }
            LogViewMode::Searching => {}
        }
    }

    fn select_next(&mut self) {
        match self.mode {
            LogViewMode::ContainerList => {
                if !self.containers.is_empty() {
                    self.selected_idx =
                        (self.selected_idx + 1).min(self.containers.len() - 1);
                }
            }
            LogViewMode::Streaming | LogViewMode::Paused => {
                self.log_buffer.scroll_down(1);
                // If scrolled back to bottom and was paused by scrolling, resume
                if self.log_buffer.is_at_bottom() && self.mode == LogViewMode::Paused {
                    self.resume();
                }
            }
            LogViewMode::SavedSearches => {
                let max = self.search_manager.len().saturating_sub(1);
                self.saved_search_idx = (self.saved_search_idx + 1).min(max);
            }
            LogViewMode::Searching => {}
        }
    }

    fn select_first(&mut self) {
        match self.mode {
            LogViewMode::ContainerList => {
                self.selected_idx = 0;
            }
            LogViewMode::Streaming | LogViewMode::Paused => {
                self.log_buffer.scroll_to_top();
                if self.mode == LogViewMode::Streaming {
                    self.pause();
                }
            }
            LogViewMode::SavedSearches => {
                self.saved_search_idx = 0;
            }
            LogViewMode::Searching => {}
        }
    }

    fn select_last(&mut self) {
        match self.mode {
            LogViewMode::ContainerList => {
                self.selected_idx = self.containers.len().saturating_sub(1);
            }
            LogViewMode::Streaming | LogViewMode::Paused => {
                self.log_buffer.scroll_to_bottom();
                if self.mode == LogViewMode::Paused && self.log_buffer.is_at_bottom() {
                    self.resume();
                }
            }
            LogViewMode::SavedSearches => {
                self.saved_search_idx =
                    self.search_manager.len().saturating_sub(1);
            }
            LogViewMode::Searching => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker_logs::types::AccessStatus;

    fn default_state() -> DockerLogsState {
        DockerLogsState::new(LogStreamConfig::default())
    }

    fn state_with_containers() -> DockerLogsState {
        let mut state = default_state();
        state.set_containers(vec![
            ContainerLogInfo {
                id: "c1".to_string(),
                name: "container-1".to_string(),
                image: "nginx".to_string(),
                status: "running".to_string(),
                access: AccessStatus::Accessible,
            },
            ContainerLogInfo {
                id: "c2".to_string(),
                name: "container-2".to_string(),
                image: "redis".to_string(),
                status: "running".to_string(),
                access: AccessStatus::Accessible,
            },
            ContainerLogInfo {
                id: "c3".to_string(),
                name: "container-3".to_string(),
                image: "postgres".to_string(),
                status: "exited".to_string(),
                access: AccessStatus::Unknown,
            },
        ]);
        state
    }

    #[test]
    fn test_initial_mode() {
        let state = default_state();
        assert_eq!(state.mode(), LogViewMode::ContainerList);
    }

    #[test]
    fn test_container_list_navigation() {
        let mut state = state_with_containers();
        assert_eq!(state.selected_idx(), 0);

        state.select_next();
        assert_eq!(state.selected_idx(), 1);

        state.select_next();
        assert_eq!(state.selected_idx(), 2);

        // Should clamp at end
        state.select_next();
        assert_eq!(state.selected_idx(), 2);

        state.select_prev();
        assert_eq!(state.selected_idx(), 1);

        state.select_first();
        assert_eq!(state.selected_idx(), 0);

        state.select_last();
        assert_eq!(state.selected_idx(), 2);
    }

    #[test]
    fn test_enter_streaming() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "container-1".to_string());

        assert_eq!(state.mode(), LogViewMode::Streaming);
        assert_eq!(state.active_container_id(), Some("c1"));
        assert_eq!(state.active_container_name(), Some("container-1"));
    }

    #[test]
    fn test_pause_resume() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.pause();
        assert_eq!(state.mode(), LogViewMode::Paused);

        state.resume();
        assert_eq!(state.mode(), LogViewMode::Streaming);
    }

    #[test]
    fn test_toggle_pause() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.toggle_pause();
        assert_eq!(state.mode(), LogViewMode::Paused);

        state.toggle_pause();
        assert_eq!(state.mode(), LogViewMode::Streaming);
    }

    #[test]
    fn test_enter_exit_search() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.enter_search();
        assert_eq!(state.mode(), LogViewMode::Searching);
        assert!(state.search_input().is_empty());

        state.search_insert_char('e');
        state.search_insert_char('r');
        assert_eq!(state.search_input(), "er");

        state.exit_search();
        assert_eq!(state.mode(), LogViewMode::Streaming);
        assert_eq!(state.log_buffer().filter(), "er");
    }

    #[test]
    fn test_cancel_search() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.enter_search();
        state.search_insert_char('x');

        state.cancel_search();
        assert_eq!(state.mode(), LogViewMode::Streaming);
        assert!(state.search_input().is_empty());
        assert!(state.log_buffer().filter().is_empty());
    }

    #[test]
    fn test_search_backspace() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());
        state.enter_search();

        state.search_insert_char('a');
        state.search_insert_char('b');
        state.search_insert_char('c');
        assert_eq!(state.search_input(), "abc");

        state.search_backspace();
        assert_eq!(state.search_input(), "ab");

        state.search_backspace();
        state.search_backspace();
        assert_eq!(state.search_input(), "");

        // Extra backspace on empty should be safe
        state.search_backspace();
        assert_eq!(state.search_input(), "");
    }

    #[test]
    fn test_back_to_list() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.back_to_list();
        assert_eq!(state.mode(), LogViewMode::ContainerList);
        assert!(state.active_container_id().is_none());
    }

    #[test]
    fn test_streaming_scroll_pauses() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        // Add entries to scroll
        use crate::docker_logs::types::{LogEntry, LogSource};
        for i in 0..20 {
            state.log_buffer_mut().push(LogEntry::new(
                "ts".to_string(),
                LogSource::Stdout,
                format!("line {}", i),
                "c1".to_string(),
                "test".to_string(),
            ));
        }

        assert_eq!(state.mode(), LogViewMode::Streaming);

        // Scrolling up in streaming mode should pause
        state.select_prev();
        assert_eq!(state.mode(), LogViewMode::Paused);
    }

    #[test]
    fn test_saved_searches() {
        let mut state = default_state();
        state.enter_streaming("c1".to_string(), "test".to_string());

        state.enter_search();
        state.search_insert_char('E');
        state.search_insert_char('R');
        state.search_insert_char('R');

        state.save_current_search("errors".to_string());
        assert_eq!(state.search_manager().len(), 1);

        state.enter_saved_searches();
        assert_eq!(state.mode(), LogViewMode::SavedSearches);
        assert_eq!(state.saved_search_idx(), 0);

        state.exit_saved_searches();
        assert_ne!(state.mode(), LogViewMode::SavedSearches);
    }

    #[test]
    fn test_status_error() {
        let mut state = default_state();
        assert!(state.status().is_none());
        assert!(state.error().is_none());

        state.set_status("connected".to_string());
        assert_eq!(state.status(), Some("connected"));

        state.set_error("timeout".to_string());
        assert_eq!(state.error(), Some("timeout"));

        state.clear_error();
        assert!(state.error().is_none());
    }

    #[test]
    fn test_selected_container() {
        let mut state = state_with_containers();
        let container = state.selected_container();
        assert!(container.is_some());
        assert_eq!(container.expect("exists").name, "container-1");

        state.select_next();
        assert_eq!(
            state.selected_container().expect("exists").name,
            "container-2"
        );
    }

    #[test]
    fn test_empty_container_list_navigation() {
        let mut state = default_state();
        // Should not panic on empty list
        state.select_next();
        state.select_prev();
        state.select_first();
        state.select_last();
        assert_eq!(state.selected_idx(), 0);
    }
}
