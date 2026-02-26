//! Pure input handler for Docker log viewer.
//!
//! Converts key events into `LogAction` values based on the current
//! `LogViewMode`. All logic is pure — no side effects.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::LogViewMode;

/// Actions that can result from key input in the log viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogAction {
    /// No action (key not recognized).
    None,
    /// Navigate up in list or scroll up in log.
    NavigateUp,
    /// Navigate down in list or scroll down in log.
    NavigateDown,
    /// Jump to first item / scroll to top.
    NavigateFirst,
    /// Jump to last item / scroll to bottom.
    NavigateLast,
    /// Page up in log view.
    PageUp,
    /// Page down in log view.
    PageDown,
    /// Activate the selected item (Enter).
    Activate,
    /// Close/go back (Esc).
    Close,
    /// Toggle pause/resume.
    TogglePause,
    /// Enter search/filter mode (Ctrl+F or /).
    StartSearch,
    /// Apply search and exit search mode.
    ApplySearch,
    /// Cancel search mode.
    CancelSearch,
    /// Insert a character into search input.
    InsertChar(char),
    /// Backspace in search input.
    SearchBackspace,
    /// Clear all log entries.
    ClearLogs,
    /// Toggle timestamp display.
    ToggleTimestamps,
    /// Show saved searches.
    ShowSavedSearches,
    /// Save current search.
    SaveSearch,
    /// Delete selected saved search.
    DeleteSavedSearch,
    /// Apply selected saved search.
    ApplySavedSearch,
    /// Show help overlay.
    ShowHelp,
}

/// Dispatches a key event to a `LogAction` based on the current mode.
///
/// This is a pure function with no side effects.
#[must_use]
pub fn handle_log_input(mode: LogViewMode, key: &KeyEvent) -> LogAction {
    match mode {
        LogViewMode::ContainerList => handle_container_list_key(key),
        LogViewMode::Streaming => handle_streaming_key(key),
        LogViewMode::Paused => handle_paused_key(key),
        LogViewMode::Searching => handle_searching_key(key),
        LogViewMode::SavedSearches => handle_saved_searches_key(key),
    }
}

/// Handles keys in container list mode.
fn handle_container_list_key(key: &KeyEvent) -> LogAction {
    match (key.modifiers, key.code) {
        // Navigation
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            LogAction::NavigateUp
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            LogAction::NavigateDown
        }
        (KeyModifiers::NONE, KeyCode::Home) => LogAction::NavigateFirst,
        (KeyModifiers::NONE, KeyCode::End) => LogAction::NavigateLast,
        // Activate
        (KeyModifiers::NONE, KeyCode::Enter) => LogAction::Activate,
        // Close
        (KeyModifiers::NONE, KeyCode::Esc) => LogAction::Close,
        // Help
        (KeyModifiers::NONE, KeyCode::Char('?')) => LogAction::ShowHelp,
        _ => LogAction::None,
    }
}

/// Handles keys in streaming mode.
fn handle_streaming_key(key: &KeyEvent) -> LogAction {
    match (key.modifiers, key.code) {
        // Scroll
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            LogAction::NavigateUp
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            LogAction::NavigateDown
        }
        (KeyModifiers::NONE, KeyCode::Home) | (KeyModifiers::NONE, KeyCode::Char('g')) => {
            LogAction::NavigateFirst
        }
        (KeyModifiers::NONE, KeyCode::End)
        | (KeyModifiers::SHIFT, KeyCode::Char('G')) => LogAction::NavigateLast,
        (KeyModifiers::NONE, KeyCode::PageUp) => LogAction::PageUp,
        (KeyModifiers::NONE, KeyCode::PageDown) => LogAction::PageDown,
        // Pause
        (KeyModifiers::NONE, KeyCode::Char(' ')) => LogAction::TogglePause,
        // Search
        (KeyModifiers::CONTROL, KeyCode::Char('f'))
        | (KeyModifiers::NONE, KeyCode::Char('/')) => LogAction::StartSearch,
        // Clear
        (KeyModifiers::NONE, KeyCode::Char('c')) => LogAction::ClearLogs,
        // Timestamps
        (KeyModifiers::NONE, KeyCode::Char('t')) => LogAction::ToggleTimestamps,
        // Saved searches
        (KeyModifiers::SHIFT, KeyCode::Char('S')) => LogAction::ShowSavedSearches,
        // Close (go back to container list)
        (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Char('q')) => {
            LogAction::Close
        }
        // Help
        (KeyModifiers::NONE, KeyCode::Char('?')) => LogAction::ShowHelp,
        _ => LogAction::None,
    }
}

/// Handles keys in paused mode (same as streaming but with resume).
fn handle_paused_key(key: &KeyEvent) -> LogAction {
    // Paused mode has the same keys as streaming
    handle_streaming_key(key)
}

/// Handles keys in search/filter mode.
fn handle_searching_key(key: &KeyEvent) -> LogAction {
    match (key.modifiers, key.code) {
        // Apply search
        (KeyModifiers::NONE, KeyCode::Enter) => LogAction::ApplySearch,
        // Cancel
        (KeyModifiers::NONE, KeyCode::Esc) => LogAction::CancelSearch,
        // Backspace
        (KeyModifiers::NONE, KeyCode::Backspace) => LogAction::SearchBackspace,
        // Save current search
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => LogAction::SaveSearch,
        // Character input
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            LogAction::InsertChar(c)
        }
        _ => LogAction::None,
    }
}

/// Handles keys in saved searches mode.
fn handle_saved_searches_key(key: &KeyEvent) -> LogAction {
    match (key.modifiers, key.code) {
        // Navigation
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            LogAction::NavigateUp
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            LogAction::NavigateDown
        }
        (KeyModifiers::NONE, KeyCode::Home) => LogAction::NavigateFirst,
        (KeyModifiers::NONE, KeyCode::End) => LogAction::NavigateLast,
        // Apply saved search
        (KeyModifiers::NONE, KeyCode::Enter) => LogAction::ApplySavedSearch,
        // Delete saved search
        (KeyModifiers::NONE, KeyCode::Char('d') | KeyCode::Delete) => {
            LogAction::DeleteSavedSearch
        }
        // Close
        (KeyModifiers::NONE, KeyCode::Esc) => LogAction::Close,
        // Help
        (KeyModifiers::NONE, KeyCode::Char('?')) => LogAction::ShowHelp,
        _ => LogAction::None,
    }
}

/// Returns true if the key is the search trigger (Ctrl+F).
#[must_use]
pub fn is_search_key(key: &KeyEvent) -> bool {
    matches!(
        (key.modifiers, key.code),
        (KeyModifiers::CONTROL, KeyCode::Char('f'))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // ========================================================================
    // Container list mode tests
    // ========================================================================

    #[test]
    fn test_container_list_navigation() {
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Up)),
            LogAction::NavigateUp
        );
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Down)),
            LogAction::NavigateDown
        );
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Char('j'))),
            LogAction::NavigateDown
        );
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Char('k'))),
            LogAction::NavigateUp
        );
    }

    #[test]
    fn test_container_list_activate_close() {
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Enter)),
            LogAction::Activate
        );
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Esc)),
            LogAction::Close
        );
    }

    #[test]
    fn test_container_list_home_end() {
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Home)),
            LogAction::NavigateFirst
        );
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::End)),
            LogAction::NavigateLast
        );
    }

    #[test]
    fn test_container_list_help() {
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Char('?'))),
            LogAction::ShowHelp
        );
    }

    // ========================================================================
    // Streaming mode tests
    // ========================================================================

    #[test]
    fn test_streaming_pause() {
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Char(' '))),
            LogAction::TogglePause
        );
    }

    #[test]
    fn test_streaming_search() {
        assert_eq!(
            handle_log_input(
                LogViewMode::Streaming,
                &key_mod(KeyCode::Char('f'), KeyModifiers::CONTROL)
            ),
            LogAction::StartSearch
        );
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Char('/'))),
            LogAction::StartSearch
        );
    }

    #[test]
    fn test_streaming_clear() {
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Char('c'))),
            LogAction::ClearLogs
        );
    }

    #[test]
    fn test_streaming_close() {
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Esc)),
            LogAction::Close
        );
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Char('q'))),
            LogAction::Close
        );
    }

    #[test]
    fn test_streaming_scroll() {
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Up)),
            LogAction::NavigateUp
        );
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::PageUp)),
            LogAction::PageUp
        );
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::PageDown)),
            LogAction::PageDown
        );
    }

    #[test]
    fn test_streaming_timestamps() {
        assert_eq!(
            handle_log_input(LogViewMode::Streaming, &key(KeyCode::Char('t'))),
            LogAction::ToggleTimestamps
        );
    }

    #[test]
    fn test_streaming_saved_searches() {
        assert_eq!(
            handle_log_input(
                LogViewMode::Streaming,
                &key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)
            ),
            LogAction::ShowSavedSearches
        );
    }

    // ========================================================================
    // Paused mode tests (should behave like streaming)
    // ========================================================================

    #[test]
    fn test_paused_same_as_streaming() {
        assert_eq!(
            handle_log_input(LogViewMode::Paused, &key(KeyCode::Char(' '))),
            LogAction::TogglePause
        );
        assert_eq!(
            handle_log_input(LogViewMode::Paused, &key(KeyCode::Char('/'))),
            LogAction::StartSearch
        );
    }

    // ========================================================================
    // Searching mode tests
    // ========================================================================

    #[test]
    fn test_searching_char_input() {
        assert_eq!(
            handle_log_input(LogViewMode::Searching, &key(KeyCode::Char('a'))),
            LogAction::InsertChar('a')
        );
    }

    #[test]
    fn test_searching_shift_char() {
        assert_eq!(
            handle_log_input(
                LogViewMode::Searching,
                &key_mod(KeyCode::Char('A'), KeyModifiers::SHIFT)
            ),
            LogAction::InsertChar('A')
        );
    }

    #[test]
    fn test_searching_backspace() {
        assert_eq!(
            handle_log_input(LogViewMode::Searching, &key(KeyCode::Backspace)),
            LogAction::SearchBackspace
        );
    }

    #[test]
    fn test_searching_apply_cancel() {
        assert_eq!(
            handle_log_input(LogViewMode::Searching, &key(KeyCode::Enter)),
            LogAction::ApplySearch
        );
        assert_eq!(
            handle_log_input(LogViewMode::Searching, &key(KeyCode::Esc)),
            LogAction::CancelSearch
        );
    }

    #[test]
    fn test_searching_save() {
        assert_eq!(
            handle_log_input(
                LogViewMode::Searching,
                &key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL)
            ),
            LogAction::SaveSearch
        );
    }

    // ========================================================================
    // Saved searches mode tests
    // ========================================================================

    #[test]
    fn test_saved_searches_navigation() {
        assert_eq!(
            handle_log_input(LogViewMode::SavedSearches, &key(KeyCode::Up)),
            LogAction::NavigateUp
        );
        assert_eq!(
            handle_log_input(LogViewMode::SavedSearches, &key(KeyCode::Down)),
            LogAction::NavigateDown
        );
    }

    #[test]
    fn test_saved_searches_apply() {
        assert_eq!(
            handle_log_input(LogViewMode::SavedSearches, &key(KeyCode::Enter)),
            LogAction::ApplySavedSearch
        );
    }

    #[test]
    fn test_saved_searches_delete() {
        assert_eq!(
            handle_log_input(LogViewMode::SavedSearches, &key(KeyCode::Char('d'))),
            LogAction::DeleteSavedSearch
        );
    }

    // ========================================================================
    // is_search_key tests
    // ========================================================================

    #[test]
    fn test_is_search_key() {
        assert!(is_search_key(&key_mod(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_search_key(&key(KeyCode::Char('f'))));
        assert!(!is_search_key(&key(KeyCode::Char('/'))));
    }

    // ========================================================================
    // Unhandled key tests
    // ========================================================================

    #[test]
    fn test_unhandled_keys_return_none() {
        assert_eq!(
            handle_log_input(LogViewMode::ContainerList, &key(KeyCode::Char('z'))),
            LogAction::None
        );
        assert_eq!(
            handle_log_input(
                LogViewMode::Streaming,
                &key_mod(KeyCode::Char('x'), KeyModifiers::ALT)
            ),
            LogAction::None
        );
    }
}
