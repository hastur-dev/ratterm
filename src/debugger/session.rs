//! Debug session management.
//!
//! Tracks the state of an active debug session including the DAP client
//! connection, current stack frames, variables, and debug console.

use std::path::PathBuf;

use super::DebugState;
use super::callstack::StackFrame;
use super::console::DebugConsole;
use super::launch::LaunchConfig;
use super::variables::Variable;

/// An active debug session.
#[derive(Debug)]
pub struct DebugSession {
    /// Current debug state.
    state: DebugState,
    /// Launch configuration used to start this session.
    config: LaunchConfig,
    /// Current call stack frames.
    stack_frames: Vec<StackFrame>,
    /// Selected stack frame index.
    selected_frame: usize,
    /// Variables for the currently selected frame.
    variables: Vec<Variable>,
    /// Selected variable index.
    selected_variable: usize,
    /// Debug console.
    console: DebugConsole,
    /// Working directory for the debug session.
    cwd: PathBuf,
    /// Whether the debug panel is visible.
    panel_visible: bool,
    /// Active panel tab.
    active_tab: DebugPanelTab,
    /// Thread ID of the stopped thread.
    thread_id: Option<i64>,
    /// Next DAP request sequence number.
    next_seq: i64,
}

/// Which tab is active in the debug panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugPanelTab {
    /// Call stack view.
    #[default]
    CallStack,
    /// Variables view.
    Variables,
    /// Debug console.
    Console,
}

impl DebugPanelTab {
    /// Returns the next tab in order.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::CallStack => Self::Variables,
            Self::Variables => Self::Console,
            Self::Console => Self::CallStack,
        }
    }

    /// Returns the previous tab in order.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::CallStack => Self::Console,
            Self::Variables => Self::CallStack,
            Self::Console => Self::Variables,
        }
    }

    /// Returns the display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CallStack => "Call Stack",
            Self::Variables => "Variables",
            Self::Console => "Console",
        }
    }
}

impl DebugSession {
    /// Creates a new debug session with the given configuration.
    #[must_use]
    pub fn new(config: LaunchConfig, cwd: PathBuf) -> Self {
        Self {
            state: DebugState::Idle,
            config,
            stack_frames: Vec::new(),
            selected_frame: 0,
            variables: Vec::new(),
            selected_variable: 0,
            console: DebugConsole::new(),
            cwd,
            panel_visible: true,
            active_tab: DebugPanelTab::default(),
            thread_id: None,
            next_seq: 1,
        }
    }

    /// Returns the current debug state.
    #[must_use]
    pub fn state(&self) -> &DebugState {
        &self.state
    }

    /// Sets the debug state.
    pub fn set_state(&mut self, state: DebugState) {
        self.state = state;
    }

    /// Returns the launch config.
    #[must_use]
    pub fn config(&self) -> &LaunchConfig {
        &self.config
    }

    /// Returns the working directory.
    #[must_use]
    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Returns the current stack frames.
    #[must_use]
    pub fn stack_frames(&self) -> &[StackFrame] {
        &self.stack_frames
    }

    /// Sets the stack frames.
    pub fn set_stack_frames(&mut self, frames: Vec<StackFrame>) {
        self.stack_frames = frames;
        if self.selected_frame >= self.stack_frames.len() {
            self.selected_frame = 0;
        }
    }

    /// Returns the selected frame index.
    #[must_use]
    pub fn selected_frame(&self) -> usize {
        self.selected_frame
    }

    /// Returns the currently selected stack frame (if any).
    #[must_use]
    pub fn current_frame(&self) -> Option<&StackFrame> {
        self.stack_frames.get(self.selected_frame)
    }

    /// Selects the next stack frame.
    pub fn select_next_frame(&mut self) {
        if !self.stack_frames.is_empty() {
            self.selected_frame = (self.selected_frame + 1).min(self.stack_frames.len() - 1);
        }
    }

    /// Selects the previous stack frame.
    pub fn select_prev_frame(&mut self) {
        self.selected_frame = self.selected_frame.saturating_sub(1);
    }

    /// Returns the variables for the current frame.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// Sets the variables.
    pub fn set_variables(&mut self, vars: Vec<Variable>) {
        self.variables = vars;
        if self.selected_variable >= self.variables.len() {
            self.selected_variable = 0;
        }
    }

    /// Returns the selected variable index.
    #[must_use]
    pub fn selected_variable(&self) -> usize {
        self.selected_variable
    }

    /// Selects the next variable.
    pub fn select_next_variable(&mut self) {
        if !self.variables.is_empty() {
            self.selected_variable = (self.selected_variable + 1).min(self.variables.len() - 1);
        }
    }

    /// Selects the previous variable.
    pub fn select_prev_variable(&mut self) {
        self.selected_variable = self.selected_variable.saturating_sub(1);
    }

    /// Toggles expansion of the currently selected variable.
    pub fn toggle_variable_expand(&mut self) {
        if let Some(var) = self.variables.get_mut(self.selected_variable)
            && var.is_expandable()
        {
            var.expanded = !var.expanded;
        }
    }

    /// Returns a reference to the debug console.
    #[must_use]
    pub fn console(&self) -> &DebugConsole {
        &self.console
    }

    /// Returns a mutable reference to the debug console.
    pub fn console_mut(&mut self) -> &mut DebugConsole {
        &mut self.console
    }

    /// Returns whether the debug panel is visible.
    #[must_use]
    pub fn panel_visible(&self) -> bool {
        self.panel_visible
    }

    /// Toggles the debug panel visibility.
    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    /// Returns the active tab.
    #[must_use]
    pub fn active_tab(&self) -> DebugPanelTab {
        self.active_tab
    }

    /// Switches to the next tab.
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
    }

    /// Switches to the previous tab.
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
    }

    /// Sets the thread ID.
    pub fn set_thread_id(&mut self, id: i64) {
        self.thread_id = Some(id);
    }

    /// Returns the thread ID.
    #[must_use]
    pub fn thread_id(&self) -> Option<i64> {
        self.thread_id
    }

    /// Gets and increments the next sequence number.
    pub fn next_seq(&mut self) -> i64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    /// Returns true if the session is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        matches!(self.state, DebugState::Paused { .. })
    }

    /// Returns true if the session is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == DebugState::Running
    }

    /// Returns true if the session is active (running or paused).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self.state, DebugState::Running | DebugState::Paused { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::launch::LaunchConfig;

    fn test_session() -> DebugSession {
        DebugSession::new(LaunchConfig::default(), PathBuf::from("/test"))
    }

    #[test]
    fn test_new_session_is_idle() {
        let session = test_session();
        assert_eq!(*session.state(), DebugState::Idle);
        assert!(!session.is_active());
        assert!(!session.is_paused());
        assert!(!session.is_running());
    }

    #[test]
    fn test_set_state() {
        let mut session = test_session();
        session.set_state(DebugState::Running);
        assert!(session.is_running());
        assert!(session.is_active());
    }

    #[test]
    fn test_stack_frame_navigation() {
        let mut session = test_session();
        session.set_stack_frames(vec![
            StackFrame {
                id: 0,
                name: "main".into(),
                source_path: None,
                line: 1,
                column: 0,
            },
            StackFrame {
                id: 1,
                name: "foo".into(),
                source_path: None,
                line: 10,
                column: 0,
            },
            StackFrame {
                id: 2,
                name: "bar".into(),
                source_path: None,
                line: 20,
                column: 0,
            },
        ]);

        assert_eq!(session.selected_frame(), 0);
        session.select_next_frame();
        assert_eq!(session.selected_frame(), 1);
        session.select_next_frame();
        assert_eq!(session.selected_frame(), 2);
        session.select_next_frame(); // Should clamp
        assert_eq!(session.selected_frame(), 2);

        session.select_prev_frame();
        assert_eq!(session.selected_frame(), 1);
        session.select_prev_frame();
        assert_eq!(session.selected_frame(), 0);
        session.select_prev_frame(); // Should clamp at 0
        assert_eq!(session.selected_frame(), 0);
    }

    #[test]
    fn test_variable_navigation() {
        let mut session = test_session();
        session.set_variables(vec![Variable::new("a", "1"), Variable::new("b", "2")]);

        assert_eq!(session.selected_variable(), 0);
        session.select_next_variable();
        assert_eq!(session.selected_variable(), 1);
        session.select_next_variable(); // Clamp
        assert_eq!(session.selected_variable(), 1);

        session.select_prev_variable();
        assert_eq!(session.selected_variable(), 0);
    }

    #[test]
    fn test_tab_cycling() {
        let mut session = test_session();
        assert_eq!(session.active_tab(), DebugPanelTab::CallStack);
        session.next_tab();
        assert_eq!(session.active_tab(), DebugPanelTab::Variables);
        session.next_tab();
        assert_eq!(session.active_tab(), DebugPanelTab::Console);
        session.next_tab();
        assert_eq!(session.active_tab(), DebugPanelTab::CallStack);
    }

    #[test]
    fn test_tab_prev() {
        let mut session = test_session();
        session.prev_tab();
        assert_eq!(session.active_tab(), DebugPanelTab::Console);
    }

    #[test]
    fn test_panel_toggle() {
        let mut session = test_session();
        assert!(session.panel_visible());
        session.toggle_panel();
        assert!(!session.panel_visible());
        session.toggle_panel();
        assert!(session.panel_visible());
    }

    #[test]
    fn test_next_seq() {
        let mut session = test_session();
        assert_eq!(session.next_seq(), 1);
        assert_eq!(session.next_seq(), 2);
        assert_eq!(session.next_seq(), 3);
    }

    #[test]
    fn test_debug_panel_tab_labels() {
        assert_eq!(DebugPanelTab::CallStack.label(), "Call Stack");
        assert_eq!(DebugPanelTab::Variables.label(), "Variables");
        assert_eq!(DebugPanelTab::Console.label(), "Console");
    }

    #[test]
    fn test_set_stack_frames_resets_selection() {
        let mut session = test_session();
        session.set_stack_frames(vec![
            StackFrame {
                id: 0,
                name: "a".into(),
                source_path: None,
                line: 1,
                column: 0,
            },
            StackFrame {
                id: 1,
                name: "b".into(),
                source_path: None,
                line: 2,
                column: 0,
            },
        ]);
        session.select_next_frame();
        assert_eq!(session.selected_frame(), 1);

        // Setting fewer frames should reset selection
        session.set_stack_frames(vec![StackFrame {
            id: 0,
            name: "c".into(),
            source_path: None,
            line: 1,
            column: 0,
        }]);
        assert_eq!(session.selected_frame(), 0);
    }
}
