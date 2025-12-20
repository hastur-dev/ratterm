//! Transport layer for API communication.
//!
//! Provides platform-specific IPC implementations:
//! - Windows: Named Pipes
//! - Unix: Domain Sockets

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
pub struct BufferedConnection<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    open: bool,
}

impl<R: BufRead + Send, W: Write + Send> BufferedConnection<R, W> {
    /// Creates a new buffered connection.
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            open: true,
        }
    }
}

impl<R: BufRead + Send, W: Write + Send> Connection for BufferedConnection<R, W> {
    fn read_message(&mut self) -> Result<Option<String>, ApiError> {
        if !self.open {
            return Ok(None);
        }

        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => {
                self.open = false;
                Ok(None)
            }
            Ok(_) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => {
                self.open = false;
                Err(ApiError::Transport(e))
            }
        }
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
