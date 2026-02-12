//! Core test harness for spawning and interacting with the ratterm binary.
//!
//! Uses `expectrl` to drive the TUI application through a pseudo-terminal,
//! sending keystrokes and asserting on screen output.

use expectrl::session::OsSession;
use expectrl::{Error as ExpectError, Expect, Regex};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

/// Default timeout for expect operations (milliseconds).
const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Binary name (platform-dependent).
#[cfg(windows)]
const BINARY_NAME: &str = "rat.exe";
#[cfg(not(windows))]
const BINARY_NAME: &str = "rat";

/// Ensure the release binary is built. Returns the path to it.
///
/// # Panics
///
/// Panics if `cargo build --release` fails.
fn ensure_binary_built() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binary = manifest_dir
        .join("target")
        .join("release")
        .join(BINARY_NAME);

    if !binary.exists() {
        let status = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&manifest_dir)
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "cargo build --release failed");
    }

    binary
}

/// The primary test harness: wraps an expectrl session talking to a `rat` process.
pub struct RattermHarness {
    pub session: OsSession,
    pub temp_dir: TempDir,
    pub binary_path: PathBuf,
    timeout: Duration,
}

impl RattermHarness {
    /// Spawn ratterm with no arguments (fresh terminal).
    ///
    /// # Errors
    ///
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn() -> Result<Self, Box<dyn std::error::Error>> {
        Self::spawn_with_args(&[])
    }

    /// Spawn ratterm with a file argument.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn_with_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::spawn_with_args(&[file_path])
    }

    /// Spawn ratterm with arbitrary arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary cannot be spawned.
    pub fn spawn_with_args(args: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        let binary_path = ensure_binary_built();
        let temp_dir = TempDir::new()?;

        // Build the command string — expectrl::spawn takes a single string
        let mut cmd = format!("\"{}\"", binary_path.display());
        for arg in args {
            cmd.push(' ');
            // Quote args that contain spaces
            if arg.contains(' ') {
                cmd.push('"');
                cmd.push_str(arg);
                cmd.push('"');
            } else {
                cmd.push_str(arg);
            }
        }

        let mut session = expectrl::spawn(&cmd)?;
        session.set_expect_timeout(Some(Duration::from_millis(DEFAULT_TIMEOUT_MS)));

        Ok(Self {
            session,
            temp_dir,
            binary_path,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        })
    }

    /// Set custom timeout for slow operations.
    pub fn set_timeout(&mut self, ms: u64) {
        self.timeout = Duration::from_millis(ms);
        self.session.set_expect_timeout(Some(self.timeout));
    }

    /// Send a raw string (for typing text or escape sequences).
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_text(&mut self, text: &str) -> Result<(), ExpectError> {
        self.session.send(text)
    }

    /// Send text followed by a newline (Enter).
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_line(&mut self, text: &str) -> Result<(), ExpectError> {
        self.session.send_line(text)
    }

    /// Send a control code.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails.
    pub fn send_control(&mut self, code: expectrl::ControlCode) -> Result<(), ExpectError> {
        self.session.send(code)
    }

    /// Wait for literal text to appear in the output.
    ///
    /// # Errors
    ///
    /// Returns an error if the text is not found within the timeout.
    pub fn expect_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.session.expect(text)?;
        Ok(())
    }

    /// Wait for a regex pattern to appear in the output.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern is not found within the timeout.
    pub fn expect_regex(&mut self, pattern: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.session.expect(Regex(pattern))?;
        Ok(())
    }

    /// Wait for text that may have ConPTY character doubling on Windows.
    ///
    /// On Windows, the TUI re-renders through ConPTY can cause each
    /// character to appear 1-N times. This uses a regex like `H+E+L+L+O+`
    /// to match regardless of doubling.
    ///
    /// # Errors
    ///
    /// Returns an error if the text is not found within the timeout.
    pub fn expect_text_tolerant(
        &mut self,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pattern = super::assertions::doubled_pattern(text);
        self.session.expect(Regex(&pattern))?;
        Ok(())
    }

    /// Wait a fixed duration (for rendering, animations, etc.).
    pub fn wait_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    /// Create a file in the temp directory and return its path.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be written.
    pub fn create_temp_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        std::fs::write(&path, content).expect("Failed to write temp file");
        path
    }

    /// Create a `.ratrc` config file in the temp directory.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be written.
    pub fn create_ratrc(&self, content: &str) -> PathBuf {
        let path = self.temp_dir.path().join(".ratrc");
        std::fs::write(&path, content).expect("Failed to write .ratrc");
        path
    }

    /// Quit ratterm cleanly by sending Ctrl+Q (`\x11`).
    ///
    /// # Errors
    ///
    /// Returns an error if the send fails.
    pub fn quit(&mut self) -> Result<(), ExpectError> {
        self.session.send("\x11")
    }
}

impl Drop for RattermHarness {
    fn drop(&mut self) {
        // Best-effort cleanup: try to quit gracefully
        let _ = self.quit();
        self.wait_ms(200);
    }
}
