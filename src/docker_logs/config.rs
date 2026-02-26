//! Configuration for Docker log streaming.

/// Configuration for log streaming behavior.
#[derive(Debug, Clone)]
pub struct LogStreamConfig {
    /// Maximum number of log entries to keep in memory.
    pub buffer_size: usize,
    /// Number of lines to fetch from history when opening logs.
    pub tail_lines: u64,
    /// Automatically scroll to new entries.
    pub auto_scroll: bool,
    /// Show timestamps in log display.
    pub show_timestamps: bool,
    /// Enable persistent log storage.
    pub storage_enabled: bool,
    /// Hours to retain stored logs before cleanup.
    pub storage_retention_hours: u64,
    /// Enable color-coded log levels.
    pub color_coding: bool,
}

impl Default for LogStreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10_000,
            tail_lines: 500,
            auto_scroll: true,
            show_timestamps: true,
            storage_enabled: true,
            storage_retention_hours: 168, // 7 days
            color_coding: true,
        }
    }
}

impl LogStreamConfig {
    /// Creates a config from application config settings.
    ///
    /// Reads `docker_log_*` keys from the config if present,
    /// falling back to defaults for any unset values.
    #[must_use]
    pub fn from_settings(settings: &[(String, String)]) -> Self {
        assert!(
            settings.len() < 1_000_000,
            "settings list unreasonably large"
        );

        let mut config = Self::default();
        for (key, value) in settings {
            config.apply_setting(key, value);
        }
        config
    }

    /// Applies a single setting key-value pair.
    pub fn apply_setting(&mut self, key: &str, value: &str) {
        match key {
            "docker_log_buffer_size" => {
                if let Ok(n) = value.parse::<usize>() {
                    self.buffer_size = n.clamp(100, 1_000_000);
                }
            }
            "docker_log_tail_lines" => {
                if let Ok(n) = value.parse::<u64>() {
                    self.tail_lines = n.clamp(10, 100_000);
                }
            }
            "docker_log_auto_scroll" => {
                self.auto_scroll = parse_bool(value);
            }
            "docker_log_timestamps" => {
                self.show_timestamps = parse_bool(value);
            }
            "docker_log_storage" => {
                self.storage_enabled = parse_bool(value);
            }
            "docker_log_retention" => {
                if let Ok(n) = value.parse::<u64>() {
                    self.storage_retention_hours = n.clamp(1, 8760); // 1h to 1 year
                }
            }
            "docker_log_color" => {
                self.color_coding = parse_bool(value);
            }
            _ => {}
        }
    }
}

/// Parses a boolean value from a config string.
fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "true" | "yes" | "1" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = LogStreamConfig::default();
        assert_eq!(config.buffer_size, 10_000);
        assert_eq!(config.tail_lines, 500);
        assert!(config.auto_scroll);
        assert!(config.show_timestamps);
        assert!(config.storage_enabled);
        assert_eq!(config.storage_retention_hours, 168);
        assert!(config.color_coding);
    }

    #[test]
    fn test_apply_buffer_size() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_buffer_size", "5000");
        assert_eq!(config.buffer_size, 5000);
    }

    #[test]
    fn test_apply_buffer_size_clamped() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_buffer_size", "50");
        assert_eq!(config.buffer_size, 100); // clamped to min
        config.apply_setting("docker_log_buffer_size", "99999999");
        assert_eq!(config.buffer_size, 1_000_000); // clamped to max
    }

    #[test]
    fn test_apply_tail_lines() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_tail_lines", "1000");
        assert_eq!(config.tail_lines, 1000);
    }

    #[test]
    fn test_apply_auto_scroll() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_auto_scroll", "false");
        assert!(!config.auto_scroll);
        config.apply_setting("docker_log_auto_scroll", "true");
        assert!(config.auto_scroll);
    }

    #[test]
    fn test_apply_timestamps() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_timestamps", "no");
        assert!(!config.show_timestamps);
    }

    #[test]
    fn test_apply_storage() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_storage", "off");
        assert!(!config.storage_enabled);
    }

    #[test]
    fn test_apply_retention() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_retention", "48");
        assert_eq!(config.storage_retention_hours, 48);
    }

    #[test]
    fn test_apply_color() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("docker_log_color", "0");
        assert!(!config.color_coding);
    }

    #[test]
    fn test_from_settings() {
        let settings = vec![
            ("docker_log_buffer_size".to_string(), "2000".to_string()),
            ("docker_log_tail_lines".to_string(), "100".to_string()),
            ("docker_log_auto_scroll".to_string(), "false".to_string()),
        ];
        let config = LogStreamConfig::from_settings(&settings);
        assert_eq!(config.buffer_size, 2000);
        assert_eq!(config.tail_lines, 100);
        assert!(!config.auto_scroll);
    }

    #[test]
    fn test_invalid_number_ignored() {
        let mut config = LogStreamConfig::default();
        let original_size = config.buffer_size;
        config.apply_setting("docker_log_buffer_size", "not_a_number");
        assert_eq!(config.buffer_size, original_size);
    }

    #[test]
    fn test_unknown_key_ignored() {
        let mut config = LogStreamConfig::default();
        config.apply_setting("unknown_key", "value");
        // Should not panic, config unchanged
        assert_eq!(config.buffer_size, 10_000);
    }

    #[test]
    fn test_parse_bool_variants() {
        assert!(parse_bool("true"));
        assert!(parse_bool("yes"));
        assert!(parse_bool("1"));
        assert!(parse_bool("on"));
        assert!(parse_bool("TRUE"));
        assert!(parse_bool("Yes"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool("0"));
        assert!(!parse_bool("off"));
    }
}
