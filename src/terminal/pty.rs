//! PTY (pseudo-terminal) management using portable-pty.
//!
//! Provides cross-platform PTY spawning and I/O.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use thiserror::Error;

/// Maximum read buffer size.
const READ_BUFFER_SIZE: usize = 4096;

/// Maximum iterations for polling loops.
const MAX_POLL_ITERATIONS: usize = 1000;

/// PTY error type.
#[derive(Debug, Error)]
pub enum PtyError {
    /// Failed to create PTY.
    #[error("Failed to create PTY: {0}")]
    Creation(String),

    /// Failed to spawn process.
    #[error("Failed to spawn process: {0}")]
    Spawn(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// PTY is closed.
    #[error("PTY is closed")]
    Closed,

    /// Maximum number of tabs reached.
    #[error("Maximum number of terminal tabs reached")]
    MaxTabsReached,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

/// PTY event.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// Output data from the PTY.
    Output(Vec<u8>),
    /// PTY process exited.
    Exit(i32),
}

/// PTY configuration.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Shell to spawn (None = system default).
    pub shell: Option<String>,
    /// Arguments to the shell.
    pub args: Vec<String>,
    /// Environment variables to set.
    pub env: Vec<(String, String)>,
    /// Working directory.
    pub working_dir: Option<PathBuf>,
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyConfig {
    /// Creates a new configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shell: None,
            args: Vec::new(),
            env: Vec::new(),
            working_dir: None,
            cols: 80,
            rows: 24,
        }
    }

    /// Sets the shell.
    #[must_use]
    pub fn shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Sets the arguments.
    #[must_use]
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Sets the dimensions.
    #[must_use]
    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// PTY instance.
pub struct Pty {
    /// Master PTY handle.
    master: Box<dyn MasterPty + Send>,
    /// Writer to the PTY.
    writer: Box<dyn Write + Send>,
    /// Receiver for PTY events.
    event_rx: Receiver<PtyEvent>,
    /// Current columns.
    cols: u16,
    /// Current rows.
    rows: u16,
    /// Reader thread handle.
    reader_thread: Option<JoinHandle<()>>,
    /// Process ID.
    pid: Option<u32>,
    /// Running flag.
    running: bool,
}

impl Pty {
    /// Creates a new PTY with the given configuration.
    ///
    /// # Errors
    /// Returns error if PTY creation or process spawning fails.
    pub fn new(config: PtyConfig) -> Result<Self, PtyError> {
        assert!(config.cols > 0, "Columns must be positive");
        assert!(config.rows > 0, "Rows must be positive");

        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Creation(e.to_string()))?;

        // Build the command
        let mut cmd = if let Some(shell) = &config.shell {
            CommandBuilder::new(shell)
        } else {
            CommandBuilder::new_default_prog()
        };

        for arg in &config.args {
            cmd.arg(arg);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        if let Some(dir) = &config.working_dir {
            cmd.cwd(dir);
        }

        // Spawn the child process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;

        let pid = child.process_id();

        // Get the writer
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        // Create channel for events
        let (event_tx, event_rx) = mpsc::channel();

        // Spawn reader thread
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let reader_thread = thread::spawn(move || {
            let mut buffer = vec![0u8; READ_BUFFER_SIZE];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        // EOF
                        let _ = event_tx.send(PtyEvent::Exit(0));
                        break;
                    }
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        if event_tx.send(PtyEvent::Output(data)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        let _ = event_tx.send(PtyEvent::Exit(-1));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            event_rx,
            cols: config.cols,
            rows: config.rows,
            reader_thread: Some(reader_thread),
            pid,
            running: true,
        })
    }

    /// Returns the number of columns.
    #[must_use]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    /// Returns the number of rows.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    /// Returns the process ID.
    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Returns true if the PTY is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Resizes the PTY.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        self.cols = cols;
        self.rows = rows;

        Ok(())
    }

    /// Writes data to the PTY.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn write(&mut self, data: &[u8]) -> Result<(), PtyError> {
        if !self.running {
            return Err(PtyError::Closed);
        }

        self.writer.write_all(data)?;
        self.writer.flush()?;

        Ok(())
    }

    /// Reads available data from the PTY.
    ///
    /// # Errors
    /// Returns error if read fails.
    pub fn read(&mut self) -> Result<Vec<u8>, PtyError> {
        if !self.running {
            return Err(PtyError::Closed);
        }

        let mut output = Vec::new();
        let mut iterations = 0;

        while iterations < MAX_POLL_ITERATIONS {
            match self.event_rx.try_recv() {
                Ok(PtyEvent::Output(data)) => {
                    output.extend(data);
                }
                Ok(PtyEvent::Exit(_code)) => {
                    self.running = false;
                    break;
                }
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.running = false;
                    break;
                }
            }
            iterations += 1;
        }

        Ok(output)
    }

    /// Tries to read an event without blocking.
    ///
    /// # Errors
    /// Returns error if the PTY is closed.
    pub fn try_read_event(&mut self) -> Result<Option<PtyEvent>, PtyError> {
        match self.event_rx.try_recv() {
            Ok(event) => {
                if matches!(event, PtyEvent::Exit(_)) {
                    self.running = false;
                }
                Ok(Some(event))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.running = false;
                Err(PtyError::Closed)
            }
        }
    }

    /// Shuts down the PTY gracefully.
    /// This is a quick shutdown that doesn't wait for threads.
    ///
    /// # Errors
    /// Returns error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<(), PtyError> {
        self.running = false;
        // Don't wait for reader thread - it will terminate when process exits
        // This allows for fast application shutdown
        let _ = self.reader_thread.take();
        Ok(())
    }

    /// Kills the PTY process.
    /// This is a quick kill that doesn't wait for threads.
    ///
    /// # Errors
    /// Returns error if kill fails.
    pub fn kill(&mut self) -> Result<(), PtyError> {
        self.running = false;
        let _ = self.reader_thread.take();
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.running = false;
        // Don't block on join during drop - let thread die with process
        let _ = self.reader_thread.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_config_defaults() {
        let config = PtyConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
        assert!(config.shell.is_none());
    }

    #[test]
    fn test_pty_config_builder() {
        let config = PtyConfig::new().size(120, 40);
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
    }
}
