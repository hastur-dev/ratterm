//! Call stack frame types for the debugger.

use std::path::Path;

/// A single frame in the call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    /// Frame ID (from the DAP server).
    pub id: i64,
    /// Function or method name.
    pub name: String,
    /// Source file path (if available).
    pub source_path: Option<String>,
    /// Line number (1-based).
    pub line: u32,
    /// Column number (0-based).
    pub column: u32,
}

impl StackFrame {
    /// Returns the source file name for display (just the filename, not full path).
    #[must_use]
    pub fn source_name(&self) -> &str {
        self.source_path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>")
    }

    /// Returns the full source path for display, or `<unknown>` if not available.
    #[must_use]
    pub fn source_path_display(&self) -> &str {
        self.source_path.as_deref().unwrap_or("<unknown>")
    }

    /// Returns a one-line summary for display in the call stack panel.
    #[must_use]
    pub fn display_line(&self) -> String {
        format!("{} ({}:{})", self.name, self.source_name(), self.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_frame_source_name() {
        let frame = StackFrame {
            id: 1,
            name: "main".to_string(),
            source_path: Some("/home/user/project/src/main.rs".to_string()),
            line: 10,
            column: 0,
        };
        assert_eq!(frame.source_name(), "main.rs");
    }

    #[test]
    fn test_stack_frame_source_name_none() {
        let frame = StackFrame {
            id: 1,
            name: "unknown_fn".to_string(),
            source_path: None,
            line: 0,
            column: 0,
        };
        assert_eq!(frame.source_name(), "<unknown>");
    }

    #[test]
    fn test_stack_frame_display_line() {
        let frame = StackFrame {
            id: 1,
            name: "my_func".to_string(),
            source_path: Some("src/lib.rs".to_string()),
            line: 42,
            column: 5,
        };
        assert_eq!(frame.display_line(), "my_func (lib.rs:42)");
    }

    #[test]
    fn test_stack_frame_source_path_display() {
        let frame = StackFrame {
            id: 1,
            name: "f".to_string(),
            source_path: Some("src/main.rs".to_string()),
            line: 1,
            column: 0,
        };
        assert_eq!(frame.source_path_display(), "src/main.rs");

        let no_source = StackFrame {
            id: 2,
            name: "g".to_string(),
            source_path: None,
            line: 0,
            column: 0,
        };
        assert_eq!(no_source.source_path_display(), "<unknown>");
    }
}
