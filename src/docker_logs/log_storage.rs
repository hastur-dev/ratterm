//! Persistent log storage as gzip-compressed JSONL files.
//!
//! Stores one file per container per date:
//! `~/.ratterm/docker_logs/{container_id}/{date}.jsonl.gz`

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::bufread::MultiGzDecoder;
use flate2::write::GzEncoder;

use super::types::{DockerLogsError, LogEntry};

/// Persistent log storage engine.
#[derive(Debug)]
pub struct LogStorage {
    /// Base directory for log files.
    base_path: PathBuf,
    /// Hours to retain logs before cleanup.
    retention_hours: u64,
    /// Whether storage is enabled.
    enabled: bool,
}

impl LogStorage {
    /// Creates a new log storage engine.
    #[must_use]
    pub fn new(enabled: bool, retention_hours: u64) -> Self {
        let base_path = Self::default_base_path();
        Self {
            base_path,
            retention_hours,
            enabled,
        }
    }

    /// Creates a log storage with a custom base path (for testing).
    #[must_use]
    pub fn with_path(base_path: PathBuf, enabled: bool, retention_hours: u64) -> Self {
        Self {
            base_path,
            retention_hours,
            enabled,
        }
    }

    /// Returns the default base path for log storage.
    fn default_base_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratterm")
            .join("docker_logs")
    }

    /// Returns the storage file path for a container on a given date.
    #[must_use]
    pub fn storage_path_for(&self, container_id: &str, date: &str) -> PathBuf {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        assert!(!date.is_empty(), "date must not be empty");

        // Sanitize container_id to prevent path traversal
        let safe_id: String = container_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();

        self.base_path
            .join(&safe_id)
            .join(format!("{date}.jsonl.gz"))
    }

    /// Returns today's date string (YYYY-MM-DD).
    fn today() -> String {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    }

    /// Appends a log entry to the storage file for today.
    ///
    /// # Errors
    /// Returns error if writing fails.
    pub fn append(
        &self,
        container_id: &str,
        entry: &LogEntry,
    ) -> Result<(), DockerLogsError> {
        if !self.enabled {
            return Ok(());
        }

        let date = Self::today();
        let path = self.storage_path_for(container_id, &date);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| DockerLogsError::StorageError(format!("mkdir: {e}")))?;
        }

        // Serialize the entry
        let json = serde_json::to_string(entry)
            .map_err(|e| DockerLogsError::StorageError(format!("serialize: {e}")))?;

        // Append to gzip file
        // We open in append mode — each append creates a new gzip member
        // This is valid gzip (multiple concatenated members)
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| DockerLogsError::StorageError(format!("open: {e}")))?;

        let mut encoder = GzEncoder::new(file, Compression::fast());
        writeln!(encoder, "{json}")
            .map_err(|e| DockerLogsError::StorageError(format!("write: {e}")))?;
        encoder
            .finish()
            .map_err(|e| DockerLogsError::StorageError(format!("flush: {e}")))?;

        Ok(())
    }

    /// Reads log history for a container, returning up to `max_lines` entries.
    ///
    /// Reads from the most recent files first.
    ///
    /// # Errors
    /// Returns error if reading fails.
    pub fn read_history(
        &self,
        container_id: &str,
        max_lines: usize,
    ) -> Result<Vec<LogEntry>, DockerLogsError> {
        if !self.enabled {
            return Ok(Vec::new());
        }

        let safe_id: String = container_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();

        let container_dir = self.base_path.join(&safe_id);
        if !container_dir.exists() {
            return Ok(Vec::new());
        }

        // List .jsonl.gz files, sorted by name (date) descending
        let mut files: Vec<PathBuf> = fs::read_dir(&container_dir)
            .map_err(|e| DockerLogsError::StorageError(format!("readdir: {e}")))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| {
                p.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "gz")
            })
            .collect();

        files.sort();
        files.reverse(); // Most recent first

        let mut entries = Vec::new();
        for file_path in &files {
            if entries.len() >= max_lines {
                break;
            }

            let file_entries = self.read_gzip_file(file_path)?;
            entries.extend(file_entries);
        }

        // Take only the last max_lines (most recent)
        if entries.len() > max_lines {
            entries = entries.split_off(entries.len() - max_lines);
        }

        Ok(entries)
    }

    /// Reads entries from a single gzip JSONL file.
    ///
    /// Uses `MultiGzDecoder` to handle multiple concatenated gzip members
    /// (one per append call).
    fn read_gzip_file(&self, path: &Path) -> Result<Vec<LogEntry>, DockerLogsError> {
        let file = fs::File::open(path)
            .map_err(|e| DockerLogsError::StorageError(format!("open {}: {e}", path.display())))?;

        let decoder = MultiGzDecoder::new(BufReader::new(file));
        let reader = BufReader::new(decoder);
        let mut entries = Vec::new();

        for line in reader.lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    match serde_json::from_str::<LogEntry>(&line) {
                        Ok(entry) => entries.push(entry),
                        Err(_) => {
                            // Skip corrupt lines
                            continue;
                        }
                    }
                }
                Ok(_) => {} // Skip empty lines
                Err(e) => {
                    tracing::debug!("Read ended for {}: {}", path.display(), e);
                    break;
                }
            }
        }

        Ok(entries)
    }

    /// Cleans up log files older than the retention period.
    ///
    /// # Errors
    /// Returns error if cleanup fails.
    pub fn cleanup(&self) -> Result<usize, DockerLogsError> {
        if !self.enabled || !self.base_path.exists() {
            return Ok(0);
        }

        let cutoff = chrono::Utc::now()
            - chrono::Duration::hours(self.retention_hours as i64);
        let cutoff_date = cutoff.format("%Y-%m-%d").to_string();

        let mut removed = 0;

        let container_dirs = fs::read_dir(&self.base_path)
            .map_err(|e| DockerLogsError::StorageError(format!("readdir: {e}")))?;

        for dir_entry in container_dirs.flatten() {
            let dir_path = dir_entry.path();
            if !dir_path.is_dir() {
                continue;
            }

            let files = match fs::read_dir(&dir_path) {
                Ok(files) => files,
                Err(_) => continue,
            };

            for file_entry in files.flatten() {
                let file_path = file_entry.path();
                let file_name = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                // Extract date from filename (YYYY-MM-DD.jsonl)
                let date_part = file_name.split('.').next().unwrap_or("");

                if date_part < cutoff_date.as_str()
                    && fs::remove_file(&file_path).is_ok()
                {
                    removed += 1;
                }
            }

            // Remove empty container directories
            if fs::read_dir(&dir_path)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
            {
                let _ = fs::remove_dir(&dir_path);
            }
        }

        Ok(removed)
    }

    /// Returns the total size of stored logs in bytes.
    ///
    /// # Errors
    /// Returns IO error if filesystem access fails.
    pub fn total_size(&self) -> io::Result<u64> {
        if !self.base_path.exists() {
            return Ok(0);
        }

        let mut total = 0u64;
        for dir_entry in fs::read_dir(&self.base_path)?.flatten() {
            if dir_entry.path().is_dir() {
                for file_entry in fs::read_dir(dir_entry.path())?.flatten() {
                    total += file_entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }

        Ok(total)
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
            "test123".to_string(),
            "test-container".to_string(),
        )
    }

    fn test_storage(dir: &Path) -> LogStorage {
        LogStorage::with_path(dir.to_path_buf(), true, 168)
    }

    #[test]
    fn test_storage_path_for() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        let path = storage.storage_path_for("abc123", "2026-01-15");
        assert!(path.to_str().expect("path").contains("abc123"));
        assert!(path.to_str().expect("path").contains("2026-01-15.jsonl.gz"));
    }

    #[test]
    fn test_storage_path_sanitizes_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        // Path traversal attempt should be sanitized
        let path = storage.storage_path_for("../../../etc", "2026-01-15");
        let path_str = path.to_str().expect("path");
        assert!(!path_str.contains(".."));
        assert!(path_str.contains("etc"));
    }

    #[test]
    fn test_append_and_read_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        let entry = make_entry("[INFO] test message");
        storage.append("test123", &entry).expect("append");

        let entries = storage.read_history("test123", 100).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "[INFO] test message");
    }

    #[test]
    fn test_append_multiple_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        for i in 0..5 {
            let entry = make_entry(&format!("line {}", i));
            storage.append("test123", &entry).expect("append");
        }

        let entries = storage.read_history("test123", 100).expect("read");
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_read_history_max_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        for i in 0..10 {
            let entry = make_entry(&format!("line {}", i));
            storage.append("test123", &entry).expect("append");
        }

        let entries = storage.read_history("test123", 3).expect("read");
        assert!(entries.len() <= 3);
    }

    #[test]
    fn test_read_nonexistent_container() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        let entries = storage
            .read_history("nonexistent", 100)
            .expect("read empty");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_disabled_storage_skips_operations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LogStorage::with_path(dir.path().to_path_buf(), false, 168);

        let entry = make_entry("test");
        storage.append("test123", &entry).expect("disabled append");

        let entries = storage.read_history("test123", 100).expect("disabled read");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_cleanup_removes_old_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LogStorage::with_path(dir.path().to_path_buf(), true, 24);

        // Create a file with an old date
        let old_path = storage.storage_path_for("test123", "2020-01-01");
        if let Some(parent) = old_path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }

        // Write a small gzip file
        let file = fs::File::create(&old_path).expect("create");
        let mut encoder = GzEncoder::new(file, Compression::fast());
        writeln!(encoder, "old data").expect("write");
        encoder.finish().expect("finish");

        assert!(old_path.exists());

        let removed = storage.cleanup().expect("cleanup");
        assert_eq!(removed, 1);
        assert!(!old_path.exists());
    }

    #[test]
    fn test_cleanup_keeps_recent_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LogStorage::with_path(dir.path().to_path_buf(), true, 168);

        // Write a file for today
        let entry = make_entry("recent");
        storage.append("test123", &entry).expect("append");

        let removed = storage.cleanup().expect("cleanup");
        assert_eq!(removed, 0);

        // File should still exist
        let entries = storage.read_history("test123", 100).expect("read");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_total_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        let size_before = storage.total_size().expect("size");
        assert_eq!(size_before, 0);

        let entry = make_entry("test message for size check");
        storage.append("test123", &entry).expect("append");

        let size_after = storage.total_size().expect("size");
        assert!(size_after > 0);
    }

    #[test]
    fn test_gzip_compression_is_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = test_storage(dir.path());

        let entry = make_entry("[ERROR] compressed test");
        storage.append("test123", &entry).expect("append");

        // Verify the file exists and is gzip-compressed
        let date = LogStorage::today();
        let path = storage.storage_path_for("test123", &date);
        assert!(path.exists());

        // Read back should work (proves gzip is valid)
        let entries = storage.read_history("test123", 100).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "[ERROR] compressed test");
    }

    #[test]
    fn test_cleanup_nonexistent_base_path() {
        let storage = LogStorage::with_path(
            PathBuf::from("/nonexistent/path/for/test"),
            true,
            168,
        );
        let result = storage.cleanup();
        assert!(result.is_ok());
        assert_eq!(result.expect("ok"), 0);
    }
}
