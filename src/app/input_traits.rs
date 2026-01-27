//! Common input handling traits and helpers for navigation patterns.
//!
//! This module provides reusable traits and helper functions to eliminate
//! duplicate input handling code across SSH Manager, Docker Manager,
//! Health Dashboard, and other list-based selectors.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ============================================================================
// Traits
// ============================================================================

/// Trait for list-based selection navigation.
///
/// Implementors provide methods to move selection up, down, to first, and to last item.
/// This is used by SSH Manager, Docker Manager, and Health Dashboard.
pub trait ListSelectable {
    /// Moves selection to the previous item (up).
    fn select_prev(&mut self);

    /// Moves selection to the next item (down).
    fn select_next(&mut self);

    /// Moves selection to the first item.
    fn select_first(&mut self);

    /// Moves selection to the last item.
    fn select_last(&mut self);
}

/// Trait for text input in form fields.
///
/// Implementors provide methods to insert and delete characters in the
/// currently focused field.
pub trait TextInputField {
    /// Inserts a character into the current field.
    fn field_insert(&mut self, c: char);

    /// Removes the last character from the current field (backspace).
    fn field_backspace(&mut self);
}

/// Trait for navigating between form fields.
///
/// Implementors provide methods to move to the next or previous field
/// in a multi-field form.
pub trait FormNavigable {
    /// Moves to the next field.
    fn next_field(&mut self);

    /// Moves to the previous field.
    fn prev_field(&mut self);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Handles Vim-style navigation keys (j/k) and arrow keys for list selection.
///
/// # Arguments
/// * `selectable` - Anything implementing `ListSelectable`
/// * `key` - The key event to process
///
/// # Returns
/// `true` if the key was handled, `false` otherwise
///
/// # Handled Keys
/// - `j` or `Down` (no modifiers): Moves selection down (`select_next`)
/// - `k` or `Up` (no modifiers): Moves selection up (`select_prev`)
pub fn handle_vim_navigation<T: ListSelectable>(selectable: &mut T, key: &KeyEvent) -> bool {
    if key.modifiers != KeyModifiers::NONE {
        return false;
    }

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            selectable.select_next();
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            selectable.select_prev();
            true
        }
        _ => false,
    }
}

/// Handles Home/End keys for jumping to first/last item in a list.
///
/// # Arguments
/// * `selectable` - Anything implementing `ListSelectable`
/// * `key` - The key event to process
///
/// # Returns
/// `true` if the key was handled, `false` otherwise
///
/// # Handled Keys
/// - `Home` (no modifiers): Moves to first item (`select_first`)
/// - `End` (no modifiers): Moves to last item (`select_last`)
pub fn handle_home_end_selection<T: ListSelectable>(selectable: &mut T, key: &KeyEvent) -> bool {
    if key.modifiers != KeyModifiers::NONE {
        return false;
    }

    match key.code {
        KeyCode::Home => {
            selectable.select_first();
            true
        }
        KeyCode::End => {
            selectable.select_last();
            true
        }
        _ => false,
    }
}

/// Handles all list navigation keys (Vim + Home/End).
///
/// This combines `handle_vim_navigation` and `handle_home_end_selection` for
/// complete list navigation support.
///
/// # Arguments
/// * `selectable` - Anything implementing `ListSelectable`
/// * `key` - The key event to process
///
/// # Returns
/// `true` if the key was handled, `false` otherwise
///
/// # Handled Keys
/// - `j` or `Down`: Moves selection down
/// - `k` or `Up`: Moves selection up
/// - `Home`: Moves to first item
/// - `End`: Moves to last item
pub fn handle_full_list_navigation<T: ListSelectable>(selectable: &mut T, key: &KeyEvent) -> bool {
    handle_vim_navigation(selectable, key) || handle_home_end_selection(selectable, key)
}

/// Handles text input keys (character input and backspace).
///
/// # Arguments
/// * `field` - Anything implementing `TextInputField`
/// * `key` - The key event to process
///
/// # Returns
/// `true` if the key was handled, `false` otherwise
///
/// # Handled Keys
/// - `Char(c)` (no modifiers or Shift): Inserts character
/// - `Backspace` (no modifiers): Removes last character
pub fn handle_text_input<T: TextInputField>(field: &mut T, key: &KeyEvent) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            field.field_backspace();
            true
        }
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            field.field_insert(c);
            true
        }
        _ => false,
    }
}

/// Handles form navigation keys (Tab and Shift+Tab/BackTab).
///
/// # Arguments
/// * `form` - Anything implementing `FormNavigable`
/// * `key` - The key event to process
///
/// # Returns
/// `true` if the key was handled, `false` otherwise
///
/// # Handled Keys
/// - `Tab` (no modifiers): Moves to next field
/// - `Tab` (Shift) or `BackTab`: Moves to previous field
pub fn handle_form_navigation<T: FormNavigable>(form: &mut T, key: &KeyEvent) -> bool {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Tab) => {
            form.next_field();
            true
        }
        (KeyModifiers::SHIFT, KeyCode::Tab)
        | (KeyModifiers::SHIFT, KeyCode::BackTab)
        | (KeyModifiers::NONE, KeyCode::BackTab) => {
            form.prev_field();
            true
        }
        _ => false,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock struct for testing ListSelectable
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

    /// Mock struct for testing TextInputField
    struct MockTextField {
        content: String,
        insert_calls: usize,
        backspace_calls: usize,
    }

    impl MockTextField {
        fn new() -> Self {
            Self {
                content: String::new(),
                insert_calls: 0,
                backspace_calls: 0,
            }
        }
    }

    impl TextInputField for MockTextField {
        fn field_insert(&mut self, c: char) {
            self.content.push(c);
            self.insert_calls += 1;
        }

        fn field_backspace(&mut self) {
            self.content.pop();
            self.backspace_calls += 1;
        }
    }

    /// Mock struct for testing FormNavigable
    struct MockForm {
        field_index: usize,
        next_calls: usize,
        prev_calls: usize,
    }

    impl MockForm {
        fn new() -> Self {
            Self {
                field_index: 0,
                next_calls: 0,
                prev_calls: 0,
            }
        }
    }

    impl FormNavigable for MockForm {
        fn next_field(&mut self) {
            self.field_index = (self.field_index + 1).min(3);
            self.next_calls += 1;
        }

        fn prev_field(&mut self) {
            self.field_index = self.field_index.saturating_sub(1);
            self.prev_calls += 1;
        }
    }

    /// Helper to create a KeyEvent
    fn key_event(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // ========================================================================
    // Vim Navigation Tests
    // ========================================================================

    #[test]
    fn test_vim_navigation_j_moves_next() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Char('j'));

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(handled, "j key should be handled");
        assert_eq!(list.next_calls, 1, "select_next should be called once");
        assert_eq!(list.prev_calls, 0, "select_prev should not be called");
    }

    #[test]
    fn test_vim_navigation_k_moves_prev() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Char('k'));

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(handled, "k key should be handled");
        assert_eq!(list.prev_calls, 1, "select_prev should be called once");
        assert_eq!(list.next_calls, 0, "select_next should not be called");
    }

    #[test]
    fn test_vim_navigation_down_arrow_moves_next() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Down);

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(handled, "Down arrow should be handled");
        assert_eq!(list.next_calls, 1, "select_next should be called once");
    }

    #[test]
    fn test_vim_navigation_up_arrow_moves_prev() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Up);

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(handled, "Up arrow should be handled");
        assert_eq!(list.prev_calls, 1, "select_prev should be called once");
    }

    #[test]
    fn test_vim_navigation_unhandled_key_returns_false() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Char('x'));

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(!handled, "x key should not be handled");
        assert_eq!(list.next_calls, 0);
        assert_eq!(list.prev_calls, 0);
    }

    #[test]
    fn test_vim_navigation_ignores_modifiers() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::CONTROL, KeyCode::Char('j'));

        let handled = handle_vim_navigation(&mut list, &key);

        assert!(!handled, "Ctrl+j should not be handled as vim navigation");
        assert_eq!(list.next_calls, 0);
    }

    // ========================================================================
    // Home/End Selection Tests
    // ========================================================================

    #[test]
    fn test_home_end_selection_home_moves_first() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Home);

        let handled = handle_home_end_selection(&mut list, &key);

        assert!(handled, "Home key should be handled");
        assert_eq!(list.first_calls, 1, "select_first should be called once");
        assert_eq!(list.last_calls, 0, "select_last should not be called");
    }

    #[test]
    fn test_home_end_selection_end_moves_last() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::End);

        let handled = handle_home_end_selection(&mut list, &key);

        assert!(handled, "End key should be handled");
        assert_eq!(list.last_calls, 1, "select_last should be called once");
        assert_eq!(list.first_calls, 0, "select_first should not be called");
    }

    #[test]
    fn test_home_end_ignores_modifiers() {
        let mut list = MockList::new();
        let key = key_event(KeyModifiers::SHIFT, KeyCode::Home);

        let handled = handle_home_end_selection(&mut list, &key);

        assert!(!handled, "Shift+Home should not be handled");
        assert_eq!(list.first_calls, 0);
    }

    // ========================================================================
    // Full List Navigation Tests
    // ========================================================================

    #[test]
    fn test_full_list_navigation_combines_handlers() {
        let mut list = MockList::new();

        // Test vim navigation
        let key_j = key_event(KeyModifiers::NONE, KeyCode::Char('j'));
        assert!(handle_full_list_navigation(&mut list, &key_j));
        assert_eq!(list.next_calls, 1);

        // Test home/end navigation
        let key_home = key_event(KeyModifiers::NONE, KeyCode::Home);
        assert!(handle_full_list_navigation(&mut list, &key_home));
        assert_eq!(list.first_calls, 1);

        // Test unhandled key
        let key_x = key_event(KeyModifiers::NONE, KeyCode::Char('x'));
        assert!(!handle_full_list_navigation(&mut list, &key_x));
    }

    // ========================================================================
    // Text Input Tests
    // ========================================================================

    #[test]
    fn test_text_input_char_inserts() {
        let mut field = MockTextField::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Char('a'));

        let handled = handle_text_input(&mut field, &key);

        assert!(handled, "Char key should be handled");
        assert_eq!(field.content, "a");
        assert_eq!(field.insert_calls, 1);
    }

    #[test]
    fn test_text_input_shift_char_inserts() {
        let mut field = MockTextField::new();
        let key = key_event(KeyModifiers::SHIFT, KeyCode::Char('A'));

        let handled = handle_text_input(&mut field, &key);

        assert!(handled, "Shift+Char should be handled");
        assert_eq!(field.content, "A");
        assert_eq!(field.insert_calls, 1);
    }

    #[test]
    fn test_text_input_backspace_removes() {
        let mut field = MockTextField::new();
        field.content = "abc".to_string();
        let key = key_event(KeyModifiers::NONE, KeyCode::Backspace);

        let handled = handle_text_input(&mut field, &key);

        assert!(handled, "Backspace should be handled");
        assert_eq!(field.content, "ab");
        assert_eq!(field.backspace_calls, 1);
    }

    #[test]
    fn test_text_input_ignores_ctrl_char() {
        let mut field = MockTextField::new();
        let key = key_event(KeyModifiers::CONTROL, KeyCode::Char('c'));

        let handled = handle_text_input(&mut field, &key);

        assert!(!handled, "Ctrl+c should not be handled as text input");
        assert_eq!(field.insert_calls, 0);
    }

    // ========================================================================
    // Form Navigation Tests
    // ========================================================================

    #[test]
    fn test_form_navigation_tab_next() {
        let mut form = MockForm::new();
        let key = key_event(KeyModifiers::NONE, KeyCode::Tab);

        let handled = handle_form_navigation(&mut form, &key);

        assert!(handled, "Tab should be handled");
        assert_eq!(form.next_calls, 1, "next_field should be called once");
        assert_eq!(form.prev_calls, 0, "prev_field should not be called");
    }

    #[test]
    fn test_form_navigation_shift_tab_prev() {
        let mut form = MockForm::new();
        form.field_index = 2;
        let key = key_event(KeyModifiers::SHIFT, KeyCode::Tab);

        let handled = handle_form_navigation(&mut form, &key);

        assert!(handled, "Shift+Tab should be handled");
        assert_eq!(form.prev_calls, 1, "prev_field should be called once");
        assert_eq!(form.next_calls, 0, "next_field should not be called");
    }

    #[test]
    fn test_form_navigation_shift_backtab_prev() {
        let mut form = MockForm::new();
        form.field_index = 2;
        let key = key_event(KeyModifiers::SHIFT, KeyCode::BackTab);

        let handled = handle_form_navigation(&mut form, &key);

        assert!(handled, "Shift+BackTab should be handled");
        assert_eq!(form.prev_calls, 1);
    }

    #[test]
    fn test_form_navigation_backtab_alone_prev() {
        let mut form = MockForm::new();
        form.field_index = 2;
        let key = key_event(KeyModifiers::NONE, KeyCode::BackTab);

        let handled = handle_form_navigation(&mut form, &key);

        assert!(handled, "BackTab alone should be handled");
        assert_eq!(form.prev_calls, 1);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_multiple_sequential_operations() {
        let mut list = MockList::new();

        // Simulate a user navigating through a list
        handle_full_list_navigation(
            &mut list,
            &key_event(KeyModifiers::NONE, KeyCode::Char('j')),
        );
        handle_full_list_navigation(
            &mut list,
            &key_event(KeyModifiers::NONE, KeyCode::Char('j')),
        );
        handle_full_list_navigation(&mut list, &key_event(KeyModifiers::NONE, KeyCode::Char('k')));
        handle_full_list_navigation(&mut list, &key_event(KeyModifiers::NONE, KeyCode::Home));
        handle_full_list_navigation(&mut list, &key_event(KeyModifiers::NONE, KeyCode::End));

        assert_eq!(list.next_calls, 2);
        assert_eq!(list.prev_calls, 1);
        assert_eq!(list.first_calls, 1);
        assert_eq!(list.last_calls, 1);
    }

    #[test]
    fn test_typing_workflow() {
        let mut field = MockTextField::new();

        // Type "hello"
        for c in "hello".chars() {
            handle_text_input(&mut field, &key_event(KeyModifiers::NONE, KeyCode::Char(c)));
        }
        assert_eq!(field.content, "hello");
        assert_eq!(field.insert_calls, 5);

        // Backspace twice
        handle_text_input(&mut field, &key_event(KeyModifiers::NONE, KeyCode::Backspace));
        handle_text_input(&mut field, &key_event(KeyModifiers::NONE, KeyCode::Backspace));
        assert_eq!(field.content, "hel");
        assert_eq!(field.backspace_calls, 2);
    }
}
