//! In-memory ring buffer for log entries.
//!
//! Stores a bounded number of log entries with support for filtering,
//! scrolling, and pausing.

use std::collections::VecDeque;

use super::types::LogEntry;

/// Ring buffer for storing log entries in memory.
#[derive(Debug, Clone)]
pub struct LogBuffer {
    /// The entries in the buffer.
    entries: VecDeque<LogEntry>,
    /// Maximum capacity.
    capacity: usize,
    /// Current filter string (empty = no filter).
    filter: String,
    /// Scroll offset from the bottom (0 = at bottom/latest).
    scroll_offset: usize,
    /// Whether the stream is paused.
    paused: bool,
    /// Total entries received (including evicted).
    total_received: u64,
}

impl LogBuffer {
    /// Creates a new log buffer with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be positive");
        Self {
            entries: VecDeque::with_capacity(capacity.min(10_000)),
            capacity,
            filter: String::new(),
            scroll_offset: 0,
            paused: false,
            total_received: 0,
        }
    }

    /// Pushes a new entry into the buffer, evicting the oldest if full.
    pub fn push(&mut self, entry: LogEntry) {
        self.total_received += 1;

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);

        // Auto-scroll: keep at bottom when not paused and not scrolled up
        if !self.paused && self.scroll_offset == 0 {
            // Already at bottom, no adjustment needed
        }
    }

    /// Clears all entries from the buffer.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0;
    }

    /// Returns the number of entries currently in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the total number of entries received since creation.
    #[must_use]
    pub fn total_received(&self) -> u64 {
        self.total_received
    }

    /// Sets the filter string. Empty string means no filter.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        // Reset scroll when filter changes
        self.scroll_offset = 0;
    }

    /// Returns the current filter string.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Returns visible entries (filtered and within scroll view).
    ///
    /// Returns entries from bottom (newest) up, limited by `visible_rows`.
    #[must_use]
    pub fn visible_entries(&self, visible_rows: usize) -> Vec<&LogEntry> {
        assert!(visible_rows < 100_000, "visible_rows unreasonably large");

        let filtered: Vec<&LogEntry> = if self.filter.is_empty() {
            self.entries.iter().collect()
        } else {
            let lower_filter = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.message.to_lowercase().contains(&lower_filter))
                .collect()
        };

        let total = filtered.len();
        if total == 0 {
            return Vec::new();
        }

        // Calculate the window to show
        let end = total.saturating_sub(self.scroll_offset);
        let start = end.saturating_sub(visible_rows);

        filtered[start..end].to_vec()
    }

    /// Returns all entries matching the current filter.
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        if self.filter.is_empty() {
            self.entries.len()
        } else {
            let lower_filter = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.message.to_lowercase().contains(&lower_filter))
                .count()
        }
    }

    /// Scrolls up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.filtered_count().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    /// Scrolls down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scrolls to the top (oldest entries).
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.filtered_count().saturating_sub(1);
    }

    /// Scrolls to the bottom (newest entries).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Returns the current scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns true if scrolled to the bottom (most recent).
    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }

    /// Pauses the stream (stops auto-scroll).
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes the stream (re-enables auto-scroll and snaps to bottom).
    pub fn resume(&mut self) {
        self.paused = false;
        self.scroll_offset = 0;
    }

    /// Returns true if the stream is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Returns a slice of all entries (unfiltered).
    #[must_use]
    pub fn all_entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker_logs::types::LogSource;

    fn make_entry(msg: &str) -> LogEntry {
        LogEntry::new(
            "2026-01-01T00:00:00Z".to_string(),
            LogSource::Stdout,
            msg.to_string(),
            "test-container-id".to_string(),
            "test-container".to_string(),
        )
    }

    #[test]
    fn test_push_and_len() {
        let mut buf = LogBuffer::new(100);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);

        buf.push(make_entry("line 1"));
        assert_eq!(buf.len(), 1);
        assert!(!buf.is_empty());

        buf.push(make_entry("line 2"));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_eviction_at_capacity() {
        let mut buf = LogBuffer::new(3);
        buf.push(make_entry("line 1"));
        buf.push(make_entry("line 2"));
        buf.push(make_entry("line 3"));
        assert_eq!(buf.len(), 3);

        buf.push(make_entry("line 4"));
        assert_eq!(buf.len(), 3);
        // Oldest entry should be evicted
        assert_eq!(buf.all_entries()[0].message, "line 2");
        assert_eq!(buf.total_received(), 4);
    }

    #[test]
    fn test_clear() {
        let mut buf = LogBuffer::new(100);
        buf.push(make_entry("line 1"));
        buf.push(make_entry("line 2"));
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_filter() {
        let mut buf = LogBuffer::new(100);
        buf.push(make_entry("[ERROR] failed"));
        buf.push(make_entry("[INFO] success"));
        buf.push(make_entry("[ERROR] another failure"));

        buf.set_filter("ERROR".to_string());
        assert_eq!(buf.filtered_count(), 2);

        let visible = buf.visible_entries(10);
        assert_eq!(visible.len(), 2);
        assert!(visible[0].message.contains("ERROR"));
        assert!(visible[1].message.contains("ERROR"));
    }

    #[test]
    fn test_filter_case_insensitive() {
        let mut buf = LogBuffer::new(100);
        buf.push(make_entry("[Error] mixed case"));
        buf.push(make_entry("[info] other"));

        buf.set_filter("error".to_string());
        assert_eq!(buf.filtered_count(), 1);
    }

    #[test]
    fn test_filter_empty_shows_all() {
        let mut buf = LogBuffer::new(100);
        buf.push(make_entry("line 1"));
        buf.push(make_entry("line 2"));

        buf.set_filter(String::new());
        assert_eq!(buf.filtered_count(), 2);
    }

    #[test]
    fn test_scroll_up_down() {
        let mut buf = LogBuffer::new(100);
        for i in 0..20 {
            buf.push(make_entry(&format!("line {}", i)));
        }

        assert!(buf.is_at_bottom());
        assert_eq!(buf.scroll_offset(), 0);

        buf.scroll_up(5);
        assert_eq!(buf.scroll_offset(), 5);
        assert!(!buf.is_at_bottom());

        buf.scroll_down(3);
        assert_eq!(buf.scroll_offset(), 2);

        buf.scroll_down(10);
        assert_eq!(buf.scroll_offset(), 0);
        assert!(buf.is_at_bottom());
    }

    #[test]
    fn test_scroll_to_top_bottom() {
        let mut buf = LogBuffer::new(100);
        for i in 0..20 {
            buf.push(make_entry(&format!("line {}", i)));
        }

        buf.scroll_to_top();
        assert_eq!(buf.scroll_offset(), 19);

        buf.scroll_to_bottom();
        assert_eq!(buf.scroll_offset(), 0);
        assert!(buf.is_at_bottom());
    }

    #[test]
    fn test_scroll_clamped() {
        let mut buf = LogBuffer::new(100);
        buf.push(make_entry("only one"));

        buf.scroll_up(100);
        // Should clamp to max (0 since only 1 entry)
        assert_eq!(buf.scroll_offset(), 0);
    }

    #[test]
    fn test_pause_resume() {
        let mut buf = LogBuffer::new(100);
        assert!(!buf.is_paused());

        buf.pause();
        assert!(buf.is_paused());

        buf.resume();
        assert!(!buf.is_paused());
        assert!(buf.is_at_bottom());
    }

    #[test]
    fn test_visible_entries_windowed() {
        let mut buf = LogBuffer::new(100);
        for i in 0..10 {
            buf.push(make_entry(&format!("line {}", i)));
        }

        // Show last 3 entries
        let visible = buf.visible_entries(3);
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].message, "line 7");
        assert_eq!(visible[1].message, "line 8");
        assert_eq!(visible[2].message, "line 9");
    }

    #[test]
    fn test_visible_entries_with_scroll() {
        let mut buf = LogBuffer::new(100);
        for i in 0..10 {
            buf.push(make_entry(&format!("line {}", i)));
        }

        buf.scroll_up(3);
        let visible = buf.visible_entries(3);
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].message, "line 4");
        assert_eq!(visible[1].message, "line 5");
        assert_eq!(visible[2].message, "line 6");
    }

    #[test]
    fn test_visible_entries_empty_buffer() {
        let buf = LogBuffer::new(100);
        let visible = buf.visible_entries(10);
        assert!(visible.is_empty());
    }

    #[test]
    fn test_total_received_tracks_evictions() {
        let mut buf = LogBuffer::new(2);
        buf.push(make_entry("a"));
        buf.push(make_entry("b"));
        buf.push(make_entry("c"));
        assert_eq!(buf.total_received(), 3);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_set_filter_resets_scroll() {
        let mut buf = LogBuffer::new(100);
        for i in 0..10 {
            buf.push(make_entry(&format!("line {}", i)));
        }
        buf.scroll_up(5);
        assert_eq!(buf.scroll_offset(), 5);

        buf.set_filter("line".to_string());
        assert_eq!(buf.scroll_offset(), 0);
    }
}
