//! Expectrl-based TUI test harness for end-to-end testing.
//!
//! Provides a `TuiTestSession` that spawns the compiled ratterm binary
//! and offers helper methods for sending keys and verifying screen output.

#![allow(dead_code, clippy::expect_used)]

use std::time::Duration;

use expectrl::Session;

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

/// Wraps an expectrl Session with TUI-specific helpers.
pub struct TuiTestSession {
    session: Session,
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
    /// # Errors
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn_with_args(args: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        let path = binary_path();
        let cmd = if args.is_empty() {
            path.clone()
        } else {
            format!("{} {}", path, args.join(" "))
        };

        let session = expectrl::spawn(cmd)?;
        Ok(Self { session })
    }

    /// Waits up to `timeout` for the given string to appear in output.
    /// Strips ANSI escape codes before matching.
    pub fn expect_text(&mut self, text: &str, timeout: Duration) -> Result<(), String> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            let screen = self.read_screen();
            if screen.contains(text) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let final_screen = self.read_screen();
        Err(format!(
            "Timed out waiting for '{}' after {:?}. Screen content:\n{}",
            text, timeout, final_screen
        ))
    }

    /// Reads all currently available output, strips ANSI codes, returns as String.
    pub fn read_screen(&mut self) -> String {
        // Try to read available bytes without blocking
        let mut all_bytes = Vec::new();
        let mut buf = [0u8; 4096];

        // Use a short non-blocking read approach
        loop {
            match self.session.try_read(&mut buf) {
                Ok(0) => break,
                Ok(n) => all_bytes.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        let stripped = strip_ansi_escapes::strip(&all_bytes);
        String::from_utf8_lossy(&stripped).to_string()
    }

    /// Sends a raw control code (e.g., Ctrl+Q = 0x11).
    pub fn send_control(&mut self, byte: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.session.send([byte])?;
        Ok(())
    }

    /// Sends an escape sequence (e.g., "\x1b[A" for arrow up).
    pub fn send_escape_seq(&mut self, seq: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.session.send(seq)?;
        Ok(())
    }

    /// Sends a single character key press.
    pub fn send_key(&mut self, c: char) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.session.send(s)?;
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

    /// Sends Ctrl+Q to quit the application, waits for exit.
    pub fn quit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control(CTRL_Q)?;
        std::thread::sleep(Duration::from_millis(500));
        Ok(())
    }

    /// Convenience: short sleep to allow TUI to re-render after input.
    pub fn wait_render(&self) {
        std::thread::sleep(Duration::from_millis(300));
    }

    /// Longer wait for initial startup rendering.
    pub fn wait_startup(&self) {
        std::thread::sleep(Duration::from_millis(800));
    }
}
