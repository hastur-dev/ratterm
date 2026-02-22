//! ConPTY-based TUI test harness for end-to-end testing on Windows.
//!
//! Provides a `TuiTestSession` that spawns the compiled ratterm binary
//! via ConPTY with a background reader thread that continuously drains
//! output — required for ConPTY to function correctly on Windows.
//!
//! ## Why ConPTY instead of expectrl?
//!
//! ConPTY (Windows Pseudo Console) requires its output pipe to be
//! continuously drained on a background thread. Without this, the
//! internal buffer fills up and the child process blocks on writes,
//! causing deadlocks. expectrl's `try_read()` and `expect()` methods
//! do single-shot reads on the main thread, which fails on Windows
//! ConPTY for TUI applications that produce continuous output.

#![allow(dead_code, clippy::expect_used)]

use std::io::{Read, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use conpty::Process;
use conpty::io::PipeWriter;

// Control code constants for key input.
pub const CTRL_Q: u8 = 0x11;
pub const CTRL_T: u8 = 0x14;
pub const CTRL_S: u8 = 0x13;
pub const CTRL_O: u8 = 0x0F;
pub const CTRL_W: u8 = 0x17;
pub const CTRL_F: u8 = 0x06;
pub const ESC: u8 = 0x1B;
pub const ENTER: u8 = 0x0D;
pub const TAB: u8 = 0x09;

/// Returns the path to the release binary.
fn binary_path() -> String {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if cfg!(windows) {
        path.push("target/release/rat.exe");
    } else {
        path.push("target/release/rat");
    }
    path.to_string_lossy().to_string()
}

/// Wraps a ConPTY process with TUI-specific helpers.
///
/// Uses a background reader thread to continuously drain the ConPTY
/// output pipe, accumulating all output in a shared buffer. This is
/// critical for ConPTY on Windows — without continuous draining, the
/// pipe buffer fills up and the child process deadlocks.
pub struct TuiTestSession {
    /// ConPTY process handle (kept alive for is_alive/wait/resize).
    process: Process,
    /// Writer pipe to send input to the child process.
    writer: PipeWriter,
    /// Accumulated output from the background reader thread.
    output: Arc<Mutex<Vec<u8>>>,
    /// Background reader thread handle.
    _reader_thread: Option<JoinHandle<()>>,
}

impl TuiTestSession {
    /// Spawns the ratterm binary with `--no-update` to skip update checks.
    ///
    /// # Errors
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn() -> Result<Self, Box<dyn std::error::Error>> {
        Self::spawn_with_args(&["--no-update"])
    }

    /// Spawns the ratterm binary with the given arguments.
    ///
    /// Creates a ConPTY pseudo-console with an explicit 120x40 size,
    /// then starts a background thread to continuously drain output.
    ///
    /// # Errors
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn_with_args(args: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        let binary = binary_path();
        let mut cmd = Command::new(&binary);
        for arg in args {
            cmd.arg(arg);
        }

        let mut proc = Process::spawn(cmd)?;
        // Set explicit console size — ConPTY may inherit a tiny or
        // zero size from the parent when running under `cargo test`.
        proc.resize(120, 40)?;

        let reader = proc.output()?;
        let writer = proc.input()?;

        // Shared buffer for accumulating output.
        let output = Arc::new(Mutex::new(Vec::with_capacity(64 * 1024)));
        let output_clone = Arc::clone(&output);

        // Background reader thread — CRITICAL for ConPTY.
        // Without continuous draining, the pipe buffer fills and
        // the child process deadlocks on its next write.
        let reader_thread = thread::Builder::new()
            .name("conpty-reader".into())
            .spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut guard) = output_clone.lock() {
                                guard.extend_from_slice(&buf[..n]);
                            }
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self {
            process: proc,
            writer,
            output,
            _reader_thread: Some(reader_thread),
        })
    }

    /// Waits up to `timeout` for the given string to appear in output.
    ///
    /// Polls the accumulated output buffer every 50ms, stripping ANSI
    /// escape codes before searching. Returns Ok if found, or an error
    /// with diagnostic output if the timeout expires.
    pub fn expect_text(&self, text: &str, timeout: Duration) -> Result<(), String> {
        assert!(!text.is_empty(), "search text must not be empty");

        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                let guard = self.output.lock().map_err(|e| e.to_string())?;
                let stripped = strip_ansi_escapes::strip(&*guard);
                let screen = String::from_utf8_lossy(&stripped);
                return Err(format!(
                    "Timed out waiting for '{}' after {:?}. Buffer: {} bytes, \
                     stripped: {} chars. Last 500 chars:\n{}",
                    text,
                    timeout,
                    guard.len(),
                    screen.len(),
                    &screen[screen.len().saturating_sub(500)..],
                ));
            }

            if let Ok(guard) = self.output.lock() {
                let stripped = strip_ansi_escapes::strip(&*guard);
                let screen = String::from_utf8_lossy(&stripped);
                if screen.contains(text) {
                    return Ok(());
                }
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Reads all currently accumulated output, strips ANSI codes.
    pub fn read_screen(&self) -> String {
        if let Ok(guard) = self.output.lock() {
            let stripped = strip_ansi_escapes::strip(&*guard);
            String::from_utf8_lossy(&stripped).to_string()
        } else {
            String::new()
        }
    }

    /// Returns the raw byte count accumulated from the ConPTY output.
    pub fn output_byte_count(&self) -> usize {
        self.output.lock().map_or(0, |g| g.len())
    }

    /// Sends a raw control code (e.g., Ctrl+Q = 0x11).
    pub fn send_control(&mut self, byte: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.writer.write_all(&[byte])?;
        Ok(())
    }

    /// Sends an escape sequence (e.g., "\x1b[A" for arrow up).
    pub fn send_escape_seq(&mut self, seq: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.writer.write_all(seq.as_bytes())?;
        Ok(())
    }

    /// Sends a single character key press.
    pub fn send_key(&mut self, c: char) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.writer.write_all(s.as_bytes())?;
        Ok(())
    }

    /// Sends Enter key.
    pub fn send_enter(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control(ENTER)
    }

    /// Sends Escape key.
    pub fn send_escape(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control(ESC)
    }

    /// Sends F1 key.
    pub fn send_f1(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1bOP")
    }

    /// Sends F2 key.
    pub fn send_f2(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1bOQ")
    }

    /// Sends F3 key.
    pub fn send_f3(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1bOR")
    }

    /// Sends F4 key.
    pub fn send_f4(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1bOS")
    }

    /// Sends Arrow Down key.
    pub fn send_arrow_down(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1b[B")
    }

    /// Sends Arrow Up key.
    pub fn send_arrow_up(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_escape_seq("\x1b[A")
    }

    /// Returns whether the child process is still running.
    pub fn is_alive(&self) -> bool {
        self.process.is_alive()
    }

    /// Sends Ctrl+Q to quit the application, waits for exit.
    pub fn quit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control(CTRL_Q)?;
        // Wait for the process to exit (up to 3 seconds).
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.process.is_alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// Convenience: short sleep to allow TUI to re-render after input.
    pub fn wait_render(&self) {
        thread::sleep(Duration::from_millis(300));
    }

    /// Longer wait for initial startup rendering.
    pub fn wait_startup(&self) {
        thread::sleep(Duration::from_millis(800));
    }
}
