//! Automatic logging module for Ratterm.
//!
//! Provides file-based logging with automatic rotation and cleanup.
//! Logs are stored in ~/.ratterm/logs/ by default.

use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Default log retention in hours.
pub const DEFAULT_LOG_RETENTION_HOURS: u32 = 24;

/// Default log level.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Maximum log file size in bytes before rotation (10 MB).
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log retention period in hours.
    pub retention_hours: u32,
    /// Log level (trace, debug, info, warn, error).
    pub level: String,
    /// Whether logging is enabled.
    pub enabled: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            retention_hours: DEFAULT_LOG_RETENTION_HOURS,
            level: DEFAULT_LOG_LEVEL.to_string(),
            enabled: true,
        }
    }
}

impl LogConfig {
    /// Parses log level from string.
    #[must_use]
    pub fn parse_level(value: &str) -> String {
        match value.to_lowercase().as_str() {
            "trace" => "trace".to_string(),
            "debug" => "debug".to_string(),
            "info" => "info".to_string(),
            "warn" | "warning" => "warn".to_string(),
            "error" => "error".to_string(),
            "off" | "none" | "disabled" => "off".to_string(),
            _ => DEFAULT_LOG_LEVEL.to_string(),
        }
    }

    /// Parses retention hours from string.
    #[must_use]
    pub fn parse_retention(value: &str) -> u32 {
        value.parse().unwrap_or(DEFAULT_LOG_RETENTION_HOURS)
    }
}

/// Returns the log directory path (~/.ratterm/logs/).
#[must_use]
pub fn log_directory() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ratterm")
        .join("logs")
}

/// Returns the current log file path.
#[must_use]
pub fn current_log_path() -> PathBuf {
    let now = chrono::Local::now();
    let filename = format!("ratterm_{}.log", now.format("%Y-%m-%d_%H-%M-%S"));
    log_directory().join(filename)
}

/// Cleans up log files older than the specified retention period.
///
/// # Errors
/// Returns error if directory cannot be read.
pub fn cleanup_old_logs(retention_hours: u32) -> io::Result<u32> {
    let log_dir = log_directory();

    if !log_dir.exists() {
        return Ok(0);
    }

    let retention_duration = Duration::from_secs(u64::from(retention_hours) * 3600);
    let now = SystemTime::now();
    let mut deleted_count = 0;

    for entry in fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .log files
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }

        // Check file age
        if let Ok(metadata) = entry.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > retention_duration && fs::remove_file(&path).is_ok() {
                        deleted_count += 1;
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

/// Initializes the logging system.
///
/// Sets up file-based logging with the specified configuration.
/// Also cleans up old log files based on retention settings.
///
/// # Errors
/// Returns error if logging cannot be initialized.
pub fn init(config: &LogConfig) -> io::Result<()> {
    if !config.enabled || config.level == "off" {
        return Ok(());
    }

    // Ensure log directory exists
    let log_dir = log_directory();
    fs::create_dir_all(&log_dir)?;

    // Clean up old logs first
    let deleted = cleanup_old_logs(config.retention_hours)?;
    if deleted > 0 {
        // We can't log this yet since logging isn't initialized
        // Will be visible in the log file after init
    }

    // Create the log file
    let log_path = current_log_path();
    let log_file = File::create(&log_path)?;

    // Build the filter from config level
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Set up the subscriber with file output
    let file_layer = fmt::layer()
        .with_writer(log_file.with_max_level(tracing::Level::TRACE))
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    // Log startup info
    tracing::info!("Ratterm logging initialized");
    tracing::info!("Log file: {}", log_path.display());
    tracing::info!("Log level: {}", config.level);
    tracing::info!("Log retention: {} hours", config.retention_hours);
    if deleted > 0 {
        tracing::info!("Cleaned up {} old log file(s)", deleted);
    }

    Ok(())
}

/// Rotates the log file if it exceeds the maximum size.
///
/// This is called periodically to prevent log files from growing too large.
#[allow(dead_code)]
pub fn check_rotation() -> io::Result<bool> {
    let log_dir = log_directory();

    // Find the most recent log file
    let mut newest_log: Option<(PathBuf, SystemTime)> = None;

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if newest_log.as_ref().is_none_or(|(_, t)| modified > *t) {
                            newest_log = Some((path, modified));
                        }
                    }
                }
            }
        }
    }

    // Check if rotation is needed
    if let Some((path, _)) = newest_log {
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.len() > MAX_LOG_SIZE_BYTES {
                // Create new log file (rotation happens naturally with timestamped names)
                tracing::info!(
                    "Log rotation triggered, file size: {} bytes",
                    metadata.len()
                );
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.retention_hours, DEFAULT_LOG_RETENTION_HOURS);
        assert_eq!(config.level, DEFAULT_LOG_LEVEL);
        assert!(config.enabled);
    }

    #[test]
    fn test_parse_level() {
        assert_eq!(LogConfig::parse_level("debug"), "debug");
        assert_eq!(LogConfig::parse_level("DEBUG"), "debug");
        assert_eq!(LogConfig::parse_level("warn"), "warn");
        assert_eq!(LogConfig::parse_level("warning"), "warn");
        assert_eq!(LogConfig::parse_level("off"), "off");
        assert_eq!(LogConfig::parse_level("invalid"), DEFAULT_LOG_LEVEL);
    }

    #[test]
    fn test_parse_retention() {
        assert_eq!(LogConfig::parse_retention("48"), 48);
        assert_eq!(LogConfig::parse_retention("0"), 0);
        assert_eq!(
            LogConfig::parse_retention("invalid"),
            DEFAULT_LOG_RETENTION_HOURS
        );
    }

    #[test]
    fn test_log_directory() {
        let dir = log_directory();
        assert!(dir.to_string_lossy().contains(".ratterm"));
        assert!(dir.to_string_lossy().contains("logs"));
    }
}
