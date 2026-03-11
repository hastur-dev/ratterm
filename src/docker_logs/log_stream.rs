//! Live log streaming from Docker containers via bollard.
//!
//! Spawns a tokio task that reads from `docker logs --follow` and sends
//! parsed `LogEntry` values through an `mpsc` channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;

use super::types::{DockerLogsError, LogEntry, LogSource};

/// Manages a live log stream from a Docker container.
#[derive(Debug)]
pub struct LogStream {
    /// Flag to signal the streaming task to stop.
    stop_flag: Arc<AtomicBool>,
    /// Container ID being streamed.
    container_id: String,
    /// Container name for display.
    container_name: String,
}

impl LogStream {
    /// Starts streaming logs from a container.
    ///
    /// Returns the `LogStream` handle and a receiver for log entries.
    ///
    /// # Errors
    /// Returns error if the stream cannot be started.
    pub fn start(
        docker: bollard::Docker,
        container_id: String,
        container_name: String,
        tail_lines: u64,
    ) -> Result<(Self, mpsc::Receiver<LogEntry>), DockerLogsError> {
        assert!(
            !container_id.is_empty(),
            "container_id must not be empty"
        );

        let stop_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel(1000);

        let stream = Self {
            stop_flag: Arc::clone(&stop_flag),
            container_id: container_id.clone(),
            container_name: container_name.clone(),
        };

        let flag = Arc::clone(&stop_flag);

        tokio::spawn(async move {
            Self::stream_task(docker, container_id, container_name, tail_lines, tx, flag)
                .await;
        });

        Ok((stream, rx))
    }

    /// The actual streaming task that runs in a tokio spawn.
    async fn stream_task(
        docker: bollard::Docker,
        container_id: String,
        container_name: String,
        tail_lines: u64,
        tx: mpsc::Sender<LogEntry>,
        stop_flag: Arc<AtomicBool>,
    ) {
        use bollard::container::LogsOptions;
        use tokio_stream::StreamExt as _;

        let options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            timestamps: true,
            tail: tail_lines.to_string(),
            ..Default::default()
        };

        let mut stream = docker.logs(&container_id, Some(options));

        while let Some(result) = stream.next().await {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            match result {
                Ok(output) => {
                    let entry =
                        parse_log_output(&output, &container_id, &container_name);
                    if tx.send(entry).await.is_err() {
                        // Receiver dropped
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Log stream error for {}: {}", container_id, e);
                    break;
                }
            }
        }
    }

    /// Signals the streaming task to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Returns true if the stream is still active (not stopped).
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed)
    }

    /// Returns the container ID being streamed.
    #[must_use]
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Returns the container name.
    #[must_use]
    pub fn container_name(&self) -> &str {
        &self.container_name
    }
}

impl Drop for LogStream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Parses a bollard `LogOutput` into a `LogEntry`.
///
/// This is a pure function that can be tested without Docker.
#[must_use]
pub fn parse_log_output(
    output: &bollard::container::LogOutput,
    container_id: &str,
    container_name: &str,
) -> LogEntry {
    let (source, raw) = match output {
        bollard::container::LogOutput::StdOut { message } => {
            (LogSource::Stdout, String::from_utf8_lossy(message).to_string())
        }
        bollard::container::LogOutput::StdErr { message } => {
            (LogSource::Stderr, String::from_utf8_lossy(message).to_string())
        }
        bollard::container::LogOutput::StdIn { message } => {
            (LogSource::Stdout, String::from_utf8_lossy(message).to_string())
        }
        bollard::container::LogOutput::Console { message } => {
            (LogSource::Stdout, String::from_utf8_lossy(message).to_string())
        }
    };

    // Try to extract timestamp from the beginning of the line
    // Docker timestamps look like: 2026-01-01T00:00:00.000000000Z
    let (timestamp, message) = extract_timestamp(&raw);

    LogEntry::new(
        timestamp,
        source,
        message,
        container_id.to_string(),
        container_name.to_string(),
    )
}

/// Extracts a Docker timestamp from the beginning of a log line.
///
/// Returns (timestamp, rest_of_line). If no timestamp found, returns
/// the current time and the full line.
fn extract_timestamp(line: &str) -> (String, String) {
    // Docker timestamp format: 2026-01-01T00:00:00.000000000Z
    // Minimum length: 20 chars (YYYY-MM-DDTHH:MM:SSZ)
    if line.len() >= 20 {
        let potential_ts = &line[..30.min(line.len())];
        // Check if it starts with a date pattern
        if potential_ts.len() >= 4
            && potential_ts.as_bytes()[4] == b'-'
            && potential_ts.contains('T')
        {
            // Find where timestamp ends (space after Z or after fractional seconds)
            if let Some(space_pos) = line.find(' ') {
                let ts = line[..space_pos].to_string();
                let msg = line[space_pos + 1..].to_string();
                return (ts, msg);
            }
        }
    }

    (chrono::Utc::now().to_rfc3339(), line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_stdout(msg: &str) -> bollard::container::LogOutput {
        bollard::container::LogOutput::StdOut {
            message: msg.as_bytes().to_vec().into(),
        }
    }

    fn make_stderr(msg: &str) -> bollard::container::LogOutput {
        bollard::container::LogOutput::StdErr {
            message: msg.as_bytes().to_vec().into(),
        }
    }

    #[test]
    fn test_parse_log_output_stdout() {
        let output = make_stdout("[INFO] Server started");
        let entry = parse_log_output(&output, "abc123", "my-app");
        assert_eq!(entry.source, LogSource::Stdout);
        assert!(entry.message.contains("Server started"));
        assert_eq!(entry.container_id, "abc123");
        assert_eq!(entry.container_name, "my-app");
    }

    #[test]
    fn test_parse_log_output_stderr() {
        let output = make_stderr("[ERROR] Connection failed");
        let entry = parse_log_output(&output, "abc123", "my-app");
        assert_eq!(entry.source, LogSource::Stderr);
        assert_eq!(entry.level, super::super::types::LogLevel::Error);
    }

    #[test]
    fn test_parse_log_output_with_timestamp() {
        let output = make_stdout("2026-01-15T10:30:00.123456789Z [INFO] ready");
        let entry = parse_log_output(&output, "abc", "test");
        assert!(entry.timestamp.contains("2026-01-15"));
        assert!(entry.message.contains("ready"));
    }

    #[test]
    fn test_extract_timestamp_with_docker_format() {
        let (ts, msg) = extract_timestamp("2026-01-15T10:30:00.123456789Z [INFO] hello");
        assert!(ts.contains("2026-01-15"));
        assert!(msg.contains("[INFO] hello"));
    }

    #[test]
    fn test_extract_timestamp_no_timestamp() {
        let (ts, msg) = extract_timestamp("just a plain line");
        assert!(!ts.is_empty()); // Should get current time
        assert_eq!(msg, "just a plain line");
    }

    #[test]
    fn test_stop_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));

        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_channel_send_receive() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let (tx, mut rx) = mpsc::channel(10);

            let entry = LogEntry::new(
                "ts".to_string(),
                LogSource::Stdout,
                "hello".to_string(),
                "cid".to_string(),
                "cname".to_string(),
            );

            tx.send(entry).await.expect("send");
            let received = rx.recv().await.expect("recv");
            assert_eq!(received.message, "hello");
        });
    }

    #[test]
    #[ignore]
    fn test_start_stream_requires_docker() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let docker = bollard::Docker::connect_with_local_defaults().expect("docker");
            let result = LogStream::start(
                docker,
                "nonexistent".to_string(),
                "test".to_string(),
                10,
            );
            assert!(result.is_ok());
        });
    }
}
