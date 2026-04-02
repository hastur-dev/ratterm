//! Integrated debugger using the Debug Adapter Protocol (DAP).
//!
//! Provides breakpoint management, a DAP client for communicating with
//! debug adapters, session management, variable inspection, call stack
//! navigation, a debug console, and launch configuration parsing.

pub mod breakpoints;
pub mod callstack;
pub mod client;
pub mod console;
pub mod launch;
pub mod session;
pub mod variables;

use std::fmt;

// Re-export main types
pub use breakpoints::BreakpointStore;
pub use callstack::StackFrame;
pub use client::DapClient;
pub use console::DebugConsole;
pub use launch::LaunchConfig;
pub use session::DebugSession;
pub use variables::Variable;

/// Current state of the debug session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugState {
    /// No active debug session.
    Idle,
    /// Debug session is running (not paused).
    Running,
    /// Execution is paused at a specific frame.
    Paused {
        /// The stack frame where execution paused.
        frame: StackFrame,
    },
    /// Debug session has stopped (terminated).
    Stopped,
}

impl Default for DebugState {
    fn default() -> Self {
        Self::Idle
    }
}

impl fmt::Display for DebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused { frame } => {
                write!(
                    f,
                    "Paused at {}:{}",
                    frame.source_path_display(),
                    frame.line
                )
            }
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

/// Events sent from the DAP client back to the application.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// Debug session initialized successfully.
    Initialized,
    /// Execution stopped (breakpoint, step, etc.).
    Stopped {
        /// Reason for stopping.
        reason: StopReason,
        /// Thread that stopped.
        thread_id: i64,
    },
    /// Execution continued.
    Continued {
        /// Thread that continued.
        thread_id: i64,
    },
    /// Debug session terminated.
    Terminated,
    /// Output from the debuggee or adapter.
    Output {
        /// Output category.
        category: String,
        /// Output text.
        output: String,
    },
    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
    /// Variables response for a given frame.
    Variables {
        /// Frame ID these variables belong to.
        frame_id: i64,
        /// The variables.
        vars: Vec<Variable>,
    },
    /// Stack trace response.
    StackTrace {
        /// The stack frames.
        frames: Vec<StackFrame>,
    },
    /// Expression evaluation result.
    EvalResult {
        /// The expression that was evaluated.
        expression: String,
        /// The result value.
        result: String,
    },
}

/// Reason the debuggee stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit a breakpoint.
    Breakpoint,
    /// Completed a step operation.
    Step,
    /// Paused by user request.
    Pause,
    /// An exception was thrown.
    Exception,
    /// Entry point reached.
    Entry,
    /// Other reason.
    Other(String),
}

impl StopReason {
    /// Parses a DAP stop reason string.
    #[must_use]
    pub fn from_dap(reason: &str) -> Self {
        match reason {
            "breakpoint" => Self::Breakpoint,
            "step" => Self::Step,
            "pause" => Self::Pause,
            "exception" => Self::Exception,
            "entry" => Self::Entry,
            other => Self::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_state_default_is_idle() {
        let state = DebugState::default();
        assert_eq!(state, DebugState::Idle);
    }

    #[test]
    fn test_debug_state_display() {
        assert_eq!(DebugState::Idle.to_string(), "Idle");
        assert_eq!(DebugState::Running.to_string(), "Running");
        assert_eq!(DebugState::Stopped.to_string(), "Stopped");

        let frame = StackFrame {
            id: 0,
            name: "main".to_string(),
            source_path: Some("src/main.rs".to_string()),
            line: 42,
            column: 0,
        };
        let paused = DebugState::Paused { frame };
        assert_eq!(paused.to_string(), "Paused at src/main.rs:42");
    }

    #[test]
    fn test_stop_reason_from_dap() {
        assert_eq!(StopReason::from_dap("breakpoint"), StopReason::Breakpoint);
        assert_eq!(StopReason::from_dap("step"), StopReason::Step);
        assert_eq!(StopReason::from_dap("pause"), StopReason::Pause);
        assert_eq!(StopReason::from_dap("exception"), StopReason::Exception);
        assert_eq!(StopReason::from_dap("entry"), StopReason::Entry);
        assert_eq!(
            StopReason::from_dap("unknown"),
            StopReason::Other("unknown".to_string())
        );
    }
}
