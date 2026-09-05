//! Turning one pod log line into the shared [`LogEntry`] type.
//!
//! Everything here is pure, so the conversion the log viewer depends on is
//! tested without a cluster.

use std::collections::HashSet;

use chrono::{DateTime, Utc};

use crate::docker_logs::types::{LogEntry, LogSource};

/// Largest line kept in full. Longer lines are truncated with a marker so one
/// pathological line cannot fill the ring buffer.
pub const MAX_LOG_LINE_BYTES: usize = 64 * 1024;

/// Appended to a line that was cut at [`MAX_LOG_LINE_BYTES`].
pub const TRUNCATION_MARKER: &str = "... (truncated)";

/// Tracks which lines a follower has already delivered.
///
/// Timestamps are not unique — a burst writes several lines into the same
/// instant on some runtimes — so the cursor keeps the exact lines seen at the
/// newest timestamp as well as the timestamp itself.
#[derive(Debug, Default)]
pub struct FollowCursor {
    newest: Option<DateTime<Utc>>,
    seen_at_newest: HashSet<String>,
}

impl FollowCursor {
    /// Creates a cursor that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the newest timestamp delivered so far.
    #[must_use]
    pub const fn newest(&self) -> Option<DateTime<Utc>> {
        self.newest
    }

    /// Decides whether a line is new, and records it if it is.
    ///
    /// A line with no timestamp is always accepted: without one there is no
    /// way to tell it from a repeat, and dropping real output is worse than
    /// showing a duplicate.
    pub fn accept(&mut self, timestamp: Option<DateTime<Utc>>, line: &str) -> bool {
        let Some(timestamp) = timestamp else {
            return true;
        };

        match self.newest {
            Some(newest) if timestamp < newest => false,
            Some(newest) if timestamp == newest => self.seen_at_newest.insert(line.to_string()),
            _ => {
                self.newest = Some(timestamp);
                self.seen_at_newest.clear();
                self.seen_at_newest.insert(line.to_string());
                true
            }
        }
    }
}

/// Splits a server-timestamped log line into its timestamp and its message.
///
/// The API server writes `<RFC3339> <message>` when timestamps are requested.
/// A line that does not start with a parseable timestamp is returned whole,
/// because a container can also write something that merely looks like one.
#[must_use]
pub fn split_timestamp(line: &str) -> (Option<DateTime<Utc>>, &str) {
    let Some((prefix, rest)) = line.split_once(' ') else {
        return (None, line);
    };
    match DateTime::parse_from_rfc3339(prefix) {
        Ok(parsed) => (Some(parsed.with_timezone(&Utc)), rest),
        Err(_) => (None, line),
    }
}

/// Shortens a line that is longer than [`MAX_LOG_LINE_BYTES`].
///
/// The cut is made on a character boundary so the result is still valid UTF-8.
#[must_use]
pub fn truncate_line(line: &str) -> String {
    if line.len() <= MAX_LOG_LINE_BYTES {
        return line.to_string();
    }
    let cut = line
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= MAX_LOG_LINE_BYTES)
        .last()
        .unwrap_or(0);
    format!("{}{TRUNCATION_MARKER}", &line[..cut])
}

/// Returns the identifier pod logs are stored under.
///
/// The log storage sanitises this into a directory name, keeping letters,
/// digits, `-` and `_`, so the separator has to be one of those for the
/// namespace, pod and container to stay distinguishable on disk.
#[must_use]
pub fn log_source_id(namespace: &str, pod: &str, container: Option<&str>) -> String {
    let base = format!("k8s_{namespace}_{pod}");
    match container {
        Some(name) if !name.is_empty() => format!("{base}_{name}"),
        _ => base,
    }
}

/// Returns the name shown next to each line in the log viewer.
#[must_use]
pub fn log_display_name(namespace: &str, pod: &str, container: Option<&str>) -> String {
    match container {
        Some(name) if !name.is_empty() => format!("{namespace}/{pod} [{name}]"),
        _ => format!("{namespace}/{pod}"),
    }
}

/// Converts one log line into the shared [`LogEntry`] type.
///
/// The message keeps whatever the container wrote, including ANSI escapes: the
/// log viewer renders them, and stripping them here would lose colour that the
/// Docker path keeps. `received_at` is used as the timestamp only when the
/// line carries none.
#[must_use]
pub fn log_entry_from_line(
    line: &str,
    namespace: &str,
    pod: &str,
    container: Option<&str>,
    received_at: DateTime<Utc>,
) -> LogEntry {
    let (timestamp, message) = split_timestamp(line);
    let message = truncate_line(message.trim_end_matches(['\r', '\n']));
    let stamp = timestamp
        .unwrap_or(received_at)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    LogEntry::new(
        stamp,
        LogSource::Stdout,
        message,
        log_source_id(namespace, pod, container),
        log_display_name(namespace, pod, container),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::docker_logs::types::LogLevel;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).expect("timestamp")
    }

    #[test]
    fn a_plain_line_becomes_a_log_entry() {
        let entry = log_entry_from_line(
            "2026-09-05T12:00:00.500Z [INFO] listening on :8080",
            "web",
            "api-0",
            Some("api"),
            at(0),
        );

        assert_eq!(entry.message, "[INFO] listening on :8080");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.source, LogSource::Stdout);
        assert_eq!(entry.container_id, "k8s_web_api-0_api");
        assert_eq!(entry.container_name, "web/api-0 [api]");
        assert_eq!(entry.timestamp, "2026-09-05T12:00:00.500Z");
    }

    #[test]
    fn a_line_without_a_server_timestamp_uses_the_receive_time() {
        let entry = log_entry_from_line("plain output", "web", "api-0", None, at(1_000));
        assert_eq!(entry.message, "plain output");
        assert_eq!(entry.timestamp, "1970-01-01T00:16:40.000Z");
        assert_eq!(entry.container_id, "k8s_web_api-0");
        assert_eq!(entry.container_name, "web/api-0");
    }

    #[test]
    fn a_line_with_ansi_escapes_keeps_them_and_still_parses_a_level() {
        let raw = "2026-09-05T12:00:00Z \u{1b}[31m[ERROR]\u{1b}[0m disk full";
        let entry = log_entry_from_line(raw, "web", "api-0", None, at(0));

        assert!(entry.message.contains('\u{1b}'), "escapes were stripped");
        assert!(entry.message.ends_with("disk full"));
        assert_eq!(entry.level, LogLevel::Error);
    }

    #[test]
    fn an_empty_line_becomes_an_empty_message_rather_than_panicking() {
        let entry = log_entry_from_line("", "web", "api-0", None, at(5));
        assert_eq!(entry.message, "");
        assert_eq!(entry.level, LogLevel::Unknown);
        assert!(!entry.container_id.is_empty());
    }

    #[test]
    fn a_timestamped_but_otherwise_empty_line_keeps_its_timestamp() {
        let entry = log_entry_from_line("2026-09-05T12:00:00Z ", "web", "api-0", None, at(5));
        assert_eq!(entry.message, "");
        assert_eq!(entry.timestamp, "2026-09-05T12:00:00.000Z");
    }

    #[test]
    fn a_very_long_line_is_truncated_with_a_marker() {
        let long = "x".repeat(MAX_LOG_LINE_BYTES * 2);
        let entry = log_entry_from_line(&long, "web", "api-0", None, at(0));

        assert!(entry.message.ends_with(TRUNCATION_MARKER), "not truncated");
        assert!(
            entry.message.len() <= MAX_LOG_LINE_BYTES + TRUNCATION_MARKER.len() + 4,
            "truncated to {} bytes",
            entry.message.len()
        );
    }

    #[test]
    fn truncation_cuts_on_a_character_boundary() {
        // Three-byte characters so the cut cannot land on a boundary by luck.
        let long = "\u{4e2d}".repeat(MAX_LOG_LINE_BYTES);
        let truncated = truncate_line(&long);
        assert!(truncated.ends_with(TRUNCATION_MARKER));
        // Reaching this line at all proves the slice was valid UTF-8.
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn a_line_at_the_limit_is_left_alone() {
        let exact = "y".repeat(MAX_LOG_LINE_BYTES);
        assert_eq!(truncate_line(&exact), exact);
        assert_eq!(truncate_line(""), "");
    }

    #[test]
    fn trailing_carriage_returns_are_removed() {
        let entry = log_entry_from_line("2026-09-05T12:00:00Z hello\r", "web", "p", None, at(0));
        assert_eq!(entry.message, "hello");
    }

    #[test]
    fn a_server_timestamp_is_split_from_the_message() {
        let (timestamp, message) = split_timestamp("2026-09-05T12:00:00Z hello world");
        assert_eq!(message, "hello world");
        assert_eq!(timestamp, Some(at(1_788_609_600)));
    }

    #[test]
    fn a_line_that_only_looks_timestamped_is_left_whole() {
        let (timestamp, message) = split_timestamp("2026-09-05 something happened");
        assert!(timestamp.is_none());
        assert_eq!(message, "2026-09-05 something happened");

        let (timestamp, message) = split_timestamp("no-spaces-at-all");
        assert!(timestamp.is_none());
        assert_eq!(message, "no-spaces-at-all");
    }

    #[test]
    fn a_source_id_survives_the_log_storage_sanitiser() {
        let id = log_source_id("web", "api-0", Some("sidecar"));
        assert_eq!(id, "k8s_web_api-0_sidecar");
        assert!(
            id.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            "{id} would be mangled on disk"
        );
        assert_eq!(log_source_id("web", "api-0", None), "k8s_web_api-0");
        assert_eq!(log_source_id("web", "api-0", Some("")), "k8s_web_api-0");
    }

    #[test]
    fn different_namespaces_get_different_source_ids() {
        assert_ne!(
            log_source_id("web", "api", None),
            log_source_id("data", "api", None)
        );
    }

    #[test]
    fn a_display_name_reads_as_a_path() {
        assert_eq!(
            log_display_name("web", "api-0", Some("api")),
            "web/api-0 [api]"
        );
        assert_eq!(log_display_name("web", "api-0", None), "web/api-0");
        assert_eq!(log_display_name("web", "api-0", Some("")), "web/api-0");
    }

    #[test]
    fn the_cursor_drops_lines_it_has_already_seen() {
        let mut cursor = FollowCursor::new();
        assert!(cursor.accept(Some(at(100)), "first"));
        assert!(!cursor.accept(Some(at(100)), "first"), "repeat accepted");
        assert!(cursor.accept(Some(at(100)), "second at same instant"));
        assert!(cursor.accept(Some(at(101)), "later"));
        assert!(!cursor.accept(Some(at(100)), "older"), "older accepted");
        assert_eq!(cursor.newest(), Some(at(101)));
    }

    #[test]
    fn the_cursor_forgets_earlier_instants_when_it_advances() {
        let mut cursor = FollowCursor::new();
        cursor.accept(Some(at(100)), "a");
        cursor.accept(Some(at(101)), "b");
        // "a" is now older than the cursor, so it is refused on its timestamp
        // rather than on the remembered set.
        assert!(!cursor.accept(Some(at(100)), "a"));
    }

    #[test]
    fn the_cursor_always_accepts_untimestamped_lines() {
        let mut cursor = FollowCursor::new();
        assert!(cursor.accept(None, "no timestamp"));
        assert!(cursor.accept(None, "no timestamp"));
        assert!(cursor.newest().is_none());
    }
}
