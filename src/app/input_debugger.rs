//! Debugger input handling for the App.
//!
//! Handles F-key shortcuts for debug operations and debug panel navigation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::PlatformKeys;

use super::App;

impl App {
    /// Handles debugger-related global key events. Returns true if handled.
    pub(super) fn handle_debugger_key(&mut self, key: KeyEvent) -> bool {
        let platform = PlatformKeys::detect();
        if platform
            .debug_step_into_binding()
            .matches(key.modifiers, key.code)
        {
            self.debug_step_in();
            return true;
        }
        if platform
            .debug_step_out_binding()
            .matches(key.modifiers, key.code)
        {
            self.debug_step_out();
            return true;
        }

        match (key.modifiers, key.code) {
            // F9: Toggle breakpoint on current editor line
            (KeyModifiers::NONE, KeyCode::F(9)) => {
                self.toggle_breakpoint_at_cursor();
                true
            }

            // F5: Continue / start debugging
            (KeyModifiers::NONE, KeyCode::F(5)) => {
                self.debug_continue_or_start();
                true
            }

            // Shift+F5: Stop debugging
            (KeyModifiers::SHIFT, KeyCode::F(5)) => {
                self.debug_stop();
                true
            }

            // Ctrl+Shift+F5: Restart debugging
            (m, KeyCode::F(5)) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.debug_restart();
                true
            }

            // F10: Step over
            (KeyModifiers::NONE, KeyCode::F(10)) => {
                self.debug_step_over();
                true
            }

            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_event_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn create_test_app() -> Option<App> {
        App::new(80, 24).ok()
    }

    #[test]
    fn test_f9_toggles_breakpoint() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let handled = app.handle_debugger_key(key_event(KeyCode::F(9)));
        assert!(handled, "F9 should be handled");
    }

    #[test]
    fn test_f5_starts_or_continues() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let handled = app.handle_debugger_key(key_event(KeyCode::F(5)));
        assert!(handled, "F5 should be handled");
    }

    #[test]
    fn test_shift_f5_stops() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let handled = app.handle_debugger_key(key_event_mod(KeyCode::F(5), KeyModifiers::SHIFT));
        assert!(handled, "Shift+F5 should be handled");
    }

    #[test]
    fn test_f10_steps_over() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let handled = app.handle_debugger_key(key_event(KeyCode::F(10)));
        assert!(handled, "F10 should be handled");
    }

    #[test]
    fn test_platform_step_in_binding() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let binding = PlatformKeys::detect().debug_step_into_binding();
        let handled = app.handle_debugger_key(key_event_mod(binding.code, binding.modifiers));
        assert!(handled, "the platform step-in binding should be handled");
    }

    #[test]
    fn test_platform_step_out_binding() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let binding = PlatformKeys::detect().debug_step_out_binding();
        let handled = app.handle_debugger_key(key_event_mod(binding.code, binding.modifiers));
        assert!(handled, "the platform step-out binding should be handled");
    }

    #[test]
    fn test_unrecognized_key_not_handled() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        let handled = app.handle_debugger_key(key_event(KeyCode::Char('x')));
        assert!(!handled, "Unrecognized key should not be handled");
    }
}
