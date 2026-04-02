//! Debugger operations for the App.
//!
//! Provides methods for breakpoint management, debug session lifecycle,
//! and stepping through code.

use tracing::info;

use crate::debugger::DebugState;
use crate::debugger::launch::LaunchConfig;
use crate::debugger::session::DebugSession;

use super::App;

impl App {
    // ========================================================================
    // Breakpoint Operations
    // ========================================================================

    /// Toggles a breakpoint on the current editor cursor line.
    pub fn toggle_breakpoint_at_cursor(&mut self) {
        let Some(path) = self.editor.path().cloned() else {
            self.set_status("No file open — cannot set breakpoint");
            return;
        };

        let line = self.editor.cursor_position().line + 1; // 1-based
        let file_str = path.display().to_string();

        let added = self.breakpoint_store.toggle(&file_str, line as u32);
        if added {
            self.set_status(format!("Breakpoint set at {}:{}", file_str, line));
        } else {
            self.set_status(format!("Breakpoint removed at {}:{}", file_str, line));
        }

        info!(
            "Breakpoint toggled: {}:{} (added={})",
            file_str, line, added
        );
    }

    /// Returns breakpoint lines for the currently open file (0-based line indices).
    #[must_use]
    pub fn current_file_breakpoints(&self) -> Vec<usize> {
        let Some(path) = self.editor.path() else {
            return Vec::new();
        };

        let file_str = path.display().to_string();
        self.breakpoint_store
            .get(&file_str)
            .iter()
            .map(|&line| (line as usize).saturating_sub(1))
            .collect()
    }

    // ========================================================================
    // Debug Session Lifecycle
    // ========================================================================

    /// Continues execution or starts a new debug session.
    pub fn debug_continue_or_start(&mut self) {
        if let Some(ref mut session) = self.debug_session {
            if session.is_paused() {
                info!("Continuing debug session");
                session.set_state(DebugState::Running);
                self.set_status("Debug: Continuing...");
                return;
            }
            if session.is_running() {
                self.set_status("Debug: Already running");
                return;
            }
        }

        // Start a new session
        self.debug_start();
    }

    /// Starts a new debug session.
    fn debug_start(&mut self) {
        let cwd = self.file_browser.path().to_path_buf();

        // Try to load launch config, or use defaults
        let configs = crate::debugger::launch::load_launch_configs(&cwd);
        let config = configs.into_iter().next().unwrap_or_else(|| {
            // Build a default config based on the current file
            let mut config = LaunchConfig::default();
            if let Some(path) = self.editor.path() {
                config.program = path.display().to_string();
            }
            config
        });

        info!("Starting debug session: program={}", config.program);

        let session = DebugSession::new(config, cwd);
        self.debug_session = Some(session);
        self.debug_panel_visible = true;
        self.set_status("Debug: Session started (adapter not connected)");
    }

    /// Stops the current debug session.
    pub fn debug_stop(&mut self) {
        if let Some(ref mut session) = self.debug_session {
            info!("Stopping debug session");
            session.set_state(DebugState::Stopped);
        }
        self.debug_session = None;
        self.debug_panel_visible = false;
        self.set_status("Debug: Session stopped");
    }

    /// Restarts the current debug session.
    pub fn debug_restart(&mut self) {
        info!("Restarting debug session");
        let config = self
            .debug_session
            .as_ref()
            .map(|s| s.config().clone());
        let cwd = self
            .debug_session
            .as_ref()
            .map(|s| s.cwd().clone())
            .unwrap_or_else(|| self.file_browser.path().to_path_buf());

        self.debug_stop();

        if let Some(config) = config {
            let session = DebugSession::new(config, cwd);
            self.debug_session = Some(session);
            self.debug_panel_visible = true;
            self.set_status("Debug: Session restarted");
        } else {
            self.debug_start();
        }
    }

    // ========================================================================
    // Stepping Operations
    // ========================================================================

    /// Steps over the current line.
    pub fn debug_step_over(&mut self) {
        if let Some(ref mut session) = self.debug_session {
            if session.is_paused() {
                info!("Debug: Step over");
                session.set_state(DebugState::Running);
                self.set_status("Debug: Stepping over...");
            } else {
                self.set_status("Debug: Not paused — cannot step");
            }
        } else {
            self.set_status("Debug: No active session");
        }
    }

    /// Steps into the current function call.
    pub fn debug_step_in(&mut self) {
        if let Some(ref mut session) = self.debug_session {
            if session.is_paused() {
                info!("Debug: Step in");
                session.set_state(DebugState::Running);
                self.set_status("Debug: Stepping in...");
            } else {
                self.set_status("Debug: Not paused — cannot step");
            }
        } else {
            self.set_status("Debug: No active session");
        }
    }

    /// Steps out of the current function.
    pub fn debug_step_out(&mut self) {
        if let Some(ref mut session) = self.debug_session {
            if session.is_paused() {
                info!("Debug: Step out");
                session.set_state(DebugState::Running);
                self.set_status("Debug: Stepping out...");
            } else {
                self.set_status("Debug: Not paused — cannot step");
            }
        } else {
            self.set_status("Debug: No active session");
        }
    }

    // ========================================================================
    // Query Helpers
    // ========================================================================

    /// Returns whether a debug session is active.
    #[must_use]
    pub fn is_debugging(&self) -> bool {
        self.debug_session
            .as_ref()
            .is_some_and(|s| s.is_active())
    }

    /// Returns the current debug state as a display string (for status bar).
    #[must_use]
    pub fn debug_status_text(&self) -> Option<String> {
        let session = self.debug_session.as_ref()?;
        Some(format!("[DEBUG: {}]", session.state()))
    }

    /// Returns a reference to the debug session.
    #[must_use]
    pub fn debug_session(&self) -> Option<&DebugSession> {
        self.debug_session.as_ref()
    }

    /// Returns a mutable reference to the debug session.
    pub fn debug_session_mut(&mut self) -> Option<&mut DebugSession> {
        self.debug_session.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> Option<App> {
        App::new(80, 24).ok()
    }

    #[test]
    fn test_no_session_initially() {
        let Some(app) = create_test_app() else {
            return;
        };
        assert!(!app.is_debugging());
        assert!(app.debug_status_text().is_none());
        assert!(app.debug_session().is_none());
    }

    #[test]
    fn test_debug_start_creates_session() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        app.debug_continue_or_start();
        assert!(app.debug_session().is_some());
        assert!(app.debug_panel_visible);
    }

    #[test]
    fn test_debug_stop_clears_session() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        app.debug_continue_or_start();
        assert!(app.debug_session().is_some());

        app.debug_stop();
        assert!(app.debug_session().is_none());
        assert!(!app.debug_panel_visible);
    }

    #[test]
    fn test_debug_restart() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        app.debug_continue_or_start();
        app.debug_restart();
        assert!(app.debug_session().is_some());
        assert!(app.debug_panel_visible);
    }

    #[test]
    fn test_toggle_breakpoint_no_file() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        app.toggle_breakpoint_at_cursor();
        assert!(app.status().contains("No file open"));
    }

    #[test]
    fn test_current_file_breakpoints_empty() {
        let Some(app) = create_test_app() else {
            return;
        };
        assert!(app.current_file_breakpoints().is_empty());
    }

    #[test]
    fn test_debug_step_no_session() {
        let Some(mut app) = create_test_app() else {
            return;
        };
        app.debug_step_over();
        assert!(app.status().contains("No active session"));
        app.debug_step_in();
        assert!(app.status().contains("No active session"));
        app.debug_step_out();
        assert!(app.status().contains("No active session"));
    }

    #[test]
    fn test_debug_status_text() {
        let Some(mut app) = create_test_app() else {
            return;
        };

        // No session
        assert!(app.debug_status_text().is_none());

        // Start session (idle by default from new())
        app.debug_continue_or_start();
        let text = app.debug_status_text();
        assert!(text.is_some());
        assert!(text.as_ref().is_some_and(|t| t.contains("DEBUG")));
    }
}
