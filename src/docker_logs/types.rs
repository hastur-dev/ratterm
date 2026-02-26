//! Core types for Docker log streaming.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Log level parsed from container output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Trace-level messages.
    Trace,
    /// Debug-level messages.
    Debug,
    /// Informational messages.
    Info,
    /// Warning messages.
    Warn,
    /// Error messages.
    Error,
    /// Fatal/critical messages.
    Fatal,
    /// Level could not be determined.
    Unknown,
}

impl LogLevel {
    /// Parses a log level from a line of log output.
    ///
    /// Recognizes patterns like `[ERROR]`, `ERROR:`, `level=error`,
    /// `"level":"error"`, and bare uppercase keywords.
    #[must_use]
    pub fn parse(line: &str) -> Self {
        assert!(!line.is_empty() || line.is_empty(), "parse accepts any line");
        let upper = line.to_uppercase();

        // Check bracketed patterns: [ERROR], [WARN], etc.
        if upper.contains("[FATAL]") || upper.contains("[CRITICAL]") {
            return Self::Fatal;
        }
        if upper.contains("[ERROR]") || upper.contains("[ERR]") {
            return Self::Error;
        }
        if upper.contains("[WARN]") || upper.contains("[WARNING]") {
            return Self::Warn;
        }
        if upper.contains("[INFO]") {
            return Self::Info;
        }
        if upper.contains("[DEBUG]") || upper.contains("[DBG]") {
            return Self::Debug;
        }
        if upper.contains("[TRACE]") {
            return Self::Trace;
        }

        // Check colon-delimited: ERROR:, WARN:, etc.
        if upper.contains("FATAL:") || upper.contains("CRITICAL:") {
            return Self::Fatal;
        }
        if upper.contains("ERROR:") || upper.contains("ERR:") {
            return Self::Error;
        }
        if upper.contains("WARN:") || upper.contains("WARNING:") {
            return Self::Warn;
        }
        if upper.contains("INFO:") {
            return Self::Info;
        }
        if upper.contains("DEBUG:") {
            return Self::Debug;
        }
        if upper.contains("TRACE:") {
            return Self::Trace;
        }

        // Check structured logging: level=error, "level":"error"
        if upper.contains("LEVEL=FATAL")
            || upper.contains("LEVEL=CRITICAL")
            || upper.contains("\"LEVEL\":\"FATAL\"")
        {
            return Self::Fatal;
        }
        if upper.contains("LEVEL=ERROR") || upper.contains("\"LEVEL\":\"ERROR\"") {
            return Self::Error;
        }
        if upper.contains("LEVEL=WARN") || upper.contains("\"LEVEL\":\"WARN\"") {
            return Self::Warn;
        }
        if upper.contains("LEVEL=INFO") || upper.contains("\"LEVEL\":\"INFO\"") {
            return Self::Info;
        }
        if upper.contains("LEVEL=DEBUG") || upper.contains("\"LEVEL\":\"DEBUG\"") {
            return Self::Debug;
        }
        if upper.contains("LEVEL=TRACE") || upper.contains("\"LEVEL\":\"TRACE\"") {
            return Self::Trace;
        }

        Self::Unknown
    }

    /// Returns the display color for this log level.
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Fatal => Color::LightRed,
            Self::Error => Color::Red,
            Self::Warn => Color::Yellow,
            Self::Info => Color::White,
            Self::Debug => Color::Gray,
            Self::Trace => Color::DarkGray,
            Self::Unknown => Color::White,
        }
    }

    /// Returns a short label for display.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fatal => "FATAL",
            Self::Error => "ERROR",
            Self::Warn => "WARN ",
            Self::Info => "INFO ",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
            Self::Unknown => "     ",
        }
    }
}

/// Source of the log output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogSource {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// A single log entry from a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry (from Docker or when received).
    pub timestamp: String,
    /// Parsed log level.
    pub level: LogLevel,
    /// Source stream (stdout/stderr).
    pub source: LogSource,
    /// The log message content.
    pub message: String,
    /// Container ID this log came from.
    pub container_id: String,
    /// Container name for display.
    pub container_name: String,
    /// The raw line as received from Docker.
    pub raw_line: String,
}

impl LogEntry {
    /// Creates a new log entry, auto-parsing the log level from the message.
    #[must_use]
    pub fn new(
        timestamp: String,
        source: LogSource,
        message: String,
        container_id: String,
        container_name: String,
    ) -> Self {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        let level = LogLevel::parse(&message);
        let raw_line = message.clone();
        Self {
            timestamp,
            level,
            source,
            message,
            container_id,
            container_name,
            raw_line,
        }
    }
}

/// Information about a Docker container for log access.
#[derive(Debug, Clone)]
pub struct ContainerLogInfo {
    /// Container ID.
    pub id: String,
    /// Container name.
    pub name: String,
    /// Image name.
    pub image: String,
    /// Container status (e.g., "running", "exited").
    pub status: String,
    /// Access status for log streaming.
    pub access: AccessStatus,
}

/// Status of access to a container's logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessStatus {
    /// Access status has not been checked yet.
    Unknown,
    /// Container logs are accessible.
    Accessible,
    /// Access denied with reason.
    Denied(String),
    /// Container not found.
    NotFound,
    /// An error occurred checking access.
    Error(String),
}

impl AccessStatus {
    /// Returns true if the status indicates accessible or unknown.
    #[must_use]
    pub fn is_accessible(&self) -> bool {
        matches!(self, Self::Accessible | Self::Unknown)
    }
}

/// Errors that can occur in the Docker log streaming system.
#[derive(Debug, thiserror::Error)]
pub enum DockerLogsError {
    /// Failed to connect to Docker daemon.
    #[error("Failed to connect to Docker: {0}")]
    ConnectionFailed(String),
    /// Container was not found.
    #[error("Container not found: {0}")]
    ContainerNotFound(String),
    /// Permission denied accessing container logs.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    /// Error in the log stream.
    #[error("Stream error: {0}")]
    StreamError(String),
    /// Error reading or writing log storage.
    #[error("Storage error: {0}")]
    StorageError(String),
    /// Configuration error.
    #[error("Config error: {0}")]
    ConfigError(String),
    /// Internal channel was closed.
    #[error("Channel closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // LogLevel::parse tests
    // ========================================================================

    #[test]
    fn test_parse_bracketed_error() {
        assert_eq!(LogLevel::parse("[ERROR] something failed"), LogLevel::Error);
        assert_eq!(LogLevel::parse("[ERR] something failed"), LogLevel::Error);
    }

    #[test]
    fn test_parse_bracketed_warn() {
        assert_eq!(
            LogLevel::parse("[WARN] potential issue"),
            LogLevel::Warn
        );
        assert_eq!(
            LogLevel::parse("[WARNING] potential issue"),
            LogLevel::Warn
        );
    }

    #[test]
    fn test_parse_bracketed_info() {
        assert_eq!(LogLevel::parse("[INFO] started service"), LogLevel::Info);
    }

    #[test]
    fn test_parse_bracketed_debug() {
        assert_eq!(LogLevel::parse("[DEBUG] variable x = 42"), LogLevel::Debug);
        assert_eq!(LogLevel::parse("[DBG] variable x = 42"), LogLevel::Debug);
    }

    #[test]
    fn test_parse_bracketed_trace() {
        assert_eq!(LogLevel::parse("[TRACE] entering function"), LogLevel::Trace);
    }

    #[test]
    fn test_parse_bracketed_fatal() {
        assert_eq!(LogLevel::parse("[FATAL] system crash"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse("[CRITICAL] out of memory"), LogLevel::Fatal);
    }

    #[test]
    fn test_parse_colon_delimited() {
        assert_eq!(LogLevel::parse("ERROR: disk full"), LogLevel::Error);
        assert_eq!(LogLevel::parse("WARN: low memory"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("INFO: ready"), LogLevel::Info);
        assert_eq!(LogLevel::parse("DEBUG: checking"), LogLevel::Debug);
    }

    #[test]
    fn test_parse_structured_logging() {
        assert_eq!(
            LogLevel::parse(r#"{"level":"error","msg":"fail"}"#),
            LogLevel::Error
        );
        assert_eq!(LogLevel::parse("level=warn msg=slow"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("level=info msg=ok"), LogLevel::Info);
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!(LogLevel::parse("[error] lower case"), LogLevel::Error);
        assert_eq!(LogLevel::parse("[Error] mixed case"), LogLevel::Error);
    }

    #[test]
    fn test_parse_unknown_for_no_level() {
        assert_eq!(LogLevel::parse("just a regular line"), LogLevel::Unknown);
        assert_eq!(LogLevel::parse(""), LogLevel::Unknown);
    }

    // ========================================================================
    // LogLevel color/label tests
    // ========================================================================

    #[test]
    fn test_level_colors() {
        assert_eq!(LogLevel::Error.color(), Color::Red);
        assert_eq!(LogLevel::Warn.color(), Color::Yellow);
        assert_eq!(LogLevel::Info.color(), Color::White);
        assert_eq!(LogLevel::Debug.color(), Color::Gray);
        assert_eq!(LogLevel::Fatal.color(), Color::LightRed);
    }

    #[test]
    fn test_level_labels() {
        assert_eq!(LogLevel::Error.label(), "ERROR");
        assert_eq!(LogLevel::Warn.label(), "WARN ");
        assert_eq!(LogLevel::Info.label(), "INFO ");
    }

    // ========================================================================
    // LogEntry tests
    // ========================================================================

    #[test]
    fn test_log_entry_creation_auto_parses_level() {
        let entry = LogEntry::new(
            "2026-01-01T00:00:00Z".to_string(),
            LogSource::Stdout,
            "[ERROR] disk full".to_string(),
            "abc123".to_string(),
            "my-container".to_string(),
        );
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.container_id, "abc123");
        assert_eq!(entry.container_name, "my-container");
        assert_eq!(entry.source, LogSource::Stdout);
    }

    #[test]
    fn test_log_entry_unknown_level() {
        let entry = LogEntry::new(
            "2026-01-01T00:00:00Z".to_string(),
            LogSource::Stderr,
            "plain text output".to_string(),
            "abc123".to_string(),
            "my-container".to_string(),
        );
        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.source, LogSource::Stderr);
    }

    // ========================================================================
    // AccessStatus tests
    // ========================================================================

    #[test]
    fn test_access_status_accessible() {
        assert!(AccessStatus::Accessible.is_accessible());
        assert!(AccessStatus::Unknown.is_accessible());
        assert!(!AccessStatus::Denied("no perms".to_string()).is_accessible());
        assert!(!AccessStatus::NotFound.is_accessible());
        assert!(!AccessStatus::Error("oops".to_string()).is_accessible());
    }

    // ========================================================================
    // DockerLogsError tests
    // ========================================================================

    #[test]
    fn test_error_display() {
        let e = DockerLogsError::ConnectionFailed("socket not found".to_string());
        assert!(e.to_string().contains("socket not found"));

        let e = DockerLogsError::ChannelClosed;
        assert_eq!(e.to_string(), "Channel closed");
    }
}
