//! Transport layer for API communication.
//!
//! Provides platform-specific IPC implementations:
//! - Windows: Named Pipes
//! - Unix: Domain Sockets

pub mod tcp;

#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub mod unix;

use crate::api::ApiError;
use std::io::{BufRead, Write};

/// Default pipe name on Windows.
#[cfg(windows)]
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\ratterm-api";

/// Default socket path on Unix.
#[cfg(unix)]
pub fn default_socket_path() -> std::path::PathBuf {
    dirs::runtime_dir()
        .or_else(|| std::env::var_os("TMPDIR").map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ratterm-api.sock")
}

/// Connection trait for reading/writing messages.
pub trait Connection: Send {
    /// Reads a single JSON message (newline-delimited).
    fn read_message(&mut self) -> Result<Option<String>, ApiError>;

    /// Writes a single JSON message (with newline).
    fn write_message(&mut self, msg: &str) -> Result<(), ApiError>;

    /// Checks if connection is still open.
    fn is_open(&self) -> bool;
}

/// Generic buffered connection wrapper.
///
/// Holds a partial line across calls. A transport with a read timeout — which
/// is how the server stays responsive to shutdown — returns from `read_line`
/// mid-message; without somewhere to keep those bytes they would be dropped
/// and every following message would be misframed.
pub struct BufferedConnection<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    open: bool,
    pending: String,
}

impl<R: BufRead + Send, W: Write + Send> BufferedConnection<R, W> {
    /// Creates a new buffered connection.
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            open: true,
            pending: String::new(),
        }
    }

    /// Removes and returns the first complete line held in `pending`.
    fn take_pending_line(&mut self) -> Option<String> {
        let idx = self.pending.find('\n')?;
        let line: String = self.pending.drain(..=idx).collect();
        Some(line.trim_end().to_string())
    }
}

/// Returns true for the error kinds that mean "no data yet", not "broken".
fn is_would_block(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
    )
}

impl<R: BufRead + Send, W: Write + Send> Connection for BufferedConnection<R, W> {
    fn read_message(&mut self) -> Result<Option<String>, ApiError> {
        if !self.open {
            return Ok(None);
        }

        // Bounded so a stream of blank lines cannot spin forever.
        const MAX_BLANK_LINES: usize = 1024;

        for _ in 0..MAX_BLANK_LINES {
            if let Some(line) = self.take_pending_line() {
                if line.is_empty() {
                    continue;
                }
                return Ok(Some(line));
            }

            match self.reader.read_line(&mut self.pending) {
                Ok(0) => {
                    self.open = false;
                    return Ok(None);
                }
                Ok(_) => {}
                Err(e) if is_would_block(e.kind()) => return Ok(None),
                Err(e) => {
                    self.open = false;
                    return Err(ApiError::Transport(e));
                }
            }
        }

        Ok(None)
    }

    fn write_message(&mut self, msg: &str) -> Result<(), ApiError> {
        if !self.open {
            return Err(ApiError::Transport(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Connection closed",
            )));
        }

        writeln!(self.writer, "{}", msg)?;
        self.writer.flush()?;
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn test_buffered_connection_read() {
        let input = b"{\"id\":\"1\"}\n{\"id\":\"2\"}\n";
        let reader = BufReader::new(Cursor::new(input.to_vec()));
        let writer = Vec::new();
        let mut conn = BufferedConnection::new(reader, writer);

        let msg1 = conn.read_message().unwrap();
        assert_eq!(msg1, Some("{\"id\":\"1\"}".to_string()));

        let msg2 = conn.read_message().unwrap();
        assert_eq!(msg2, Some("{\"id\":\"2\"}".to_string()));
    }

    #[test]
    fn test_buffered_connection_write() {
        let reader = BufReader::new(Cursor::new(Vec::new()));
        let writer = Vec::new();
        let mut conn = BufferedConnection::new(reader, writer);

        conn.write_message("{\"id\":\"1\"}").unwrap();
        // Note: Can't easily check output in this test setup
    }
}
