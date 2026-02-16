//! Unified dashboard navigation system.
//!
//! Provides a single entry point for dashboard key handling that ensures
//! consistent navigation across all dashboard screens (SSH Manager, Docker
//! Manager, Health Dashboard, and any future dashboards).
//!
//! # Usage
//!
//! Any dashboard input handler calls [`apply_dashboard_navigation()`] as its
//! FIRST action. If it returns [`NavResult::Handled`], the key was consumed
//! by the navigation layer and the handler should return immediately.
//! If it returns [`NavResult::Unhandled`], the handler processes the key
//! with its screen-specific logic.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::input_traits::{ListSelectable, handle_full_list_navigation};

/// Result of the navigation layer processing a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavResult {
    /// Key was handled by the navigation layer (list moved).
    Handled,
    /// Key was not a navigation key. Caller should process it.
    Unhandled,
    /// The `?` help key was pressed. Caller should show the hotkey overlay.
    ShowHelp,
    /// Escape was pressed. Caller should close/go back.
    Close,
    /// Enter was pressed. Caller should activate the selected item.
    Activate,
}

/// Applies the standard dashboard navigation layer.
///
/// Handles:
/// - Arrow Up/Down, j/k: list navigation (via [`ListSelectable`])
/// - Home/End: jump to first/last
/// - `?`: request hotkey overlay
/// - Esc: request close
/// - Enter: request activation
///
/// This is a **pure function** — it does not call any `App` methods.
/// The caller interprets the returned [`NavResult`].
pub fn apply_dashboard_navigation<T: ListSelectable>(
    selectable: &mut T,
    key: &KeyEvent,
) -> NavResult {
    // 1. Handle list navigation (arrows, j/k, Home/End)
    if handle_full_list_navigation(selectable, key) {
        return NavResult::Handled;
    }

    // 2. Handle universal dashboard keys
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('?')) => NavResult::ShowHelp,
        (KeyModifiers::NONE, KeyCode::Esc) => NavResult::Close,
        (KeyModifiers::NONE, KeyCode::Enter) => NavResult::Activate,
        _ => NavResult::Unhandled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock list for testing navigation.
    struct MockList {
        prev_calls: usize,
        next_calls: usize,
        first_calls: usize,
        last_calls: usize,
    }

    impl MockList {
        fn new() -> Self {
            Self {
                prev_calls: 0,
                next_calls: 0,
                first_calls: 0,
                last_calls: 0,
            }
        }
    }

    impl ListSelectable for MockList {
        fn select_prev(&mut self) {
            self.prev_calls += 1;
        }
        fn select_next(&mut self) {
            self.next_calls += 1;
        }
        fn select_first(&mut self) {
            self.first_calls += 1;
        }
        fn select_last(&mut self) {
            self.last_calls += 1;
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_arrow_down_navigates() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Down));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.next_calls, 1);
        assert_eq!(list.prev_calls, 0);
    }

    #[test]
    fn test_arrow_up_navigates() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Up));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.prev_calls, 1);
        assert_eq!(list.next_calls, 0);
    }

    #[test]
    fn test_j_navigates_down() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Char('j')));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.next_calls, 1);
    }

    #[test]
    fn test_k_navigates_up() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Char('k')));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.prev_calls, 1);
    }

    #[test]
    fn test_home_navigates_first() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Home));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.first_calls, 1);
    }

    #[test]
    fn test_end_navigates_last() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::End));
        assert_eq!(result, NavResult::Handled);
        assert_eq!(list.last_calls, 1);
    }

    #[test]
    fn test_question_mark_shows_help() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Char('?')));
        assert_eq!(result, NavResult::ShowHelp);
        // Navigation methods should NOT be called
        assert_eq!(list.next_calls, 0);
        assert_eq!(list.prev_calls, 0);
    }

    #[test]
    fn test_escape_closes() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Esc));
        assert_eq!(result, NavResult::Close);
    }

    #[test]
    fn test_enter_activates() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Enter));
        assert_eq!(result, NavResult::Activate);
    }

    #[test]
    fn test_unrecognized_key_unhandled() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Char('x')));
        assert_eq!(result, NavResult::Unhandled);
        assert_eq!(list.next_calls, 0);
        assert_eq!(list.prev_calls, 0);
    }

    #[test]
    fn test_ctrl_modified_keys_unhandled() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(
            &mut list,
            &key_mod(KeyCode::Char('j'), KeyModifiers::CONTROL),
        );
        assert_eq!(result, NavResult::Unhandled);
        assert_eq!(list.next_calls, 0);
    }

    #[test]
    fn test_navigation_does_not_consume_close() {
        let mut list = MockList::new();
        let result = apply_dashboard_navigation(&mut list, &key(KeyCode::Esc));
        assert_eq!(result, NavResult::Close);
        // Esc should NOT trigger any list navigation
        assert_eq!(list.prev_calls, 0);
        assert_eq!(list.next_calls, 0);
        assert_eq!(list.first_calls, 0);
        assert_eq!(list.last_calls, 0);
    }

    #[test]
    fn test_sequential_navigation() {
        let mut list = MockList::new();

        // j, j, k
        apply_dashboard_navigation(&mut list, &key(KeyCode::Char('j')));
        apply_dashboard_navigation(&mut list, &key(KeyCode::Char('j')));
        apply_dashboard_navigation(&mut list, &key(KeyCode::Char('k')));

        assert_eq!(list.next_calls, 2);
        assert_eq!(list.prev_calls, 1);
    }
}
