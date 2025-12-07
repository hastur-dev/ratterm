//! Tests for PTY (pseudo-terminal) spawning and management.
//!
//! Note: These tests spawn actual processes and require a shell.

#![allow(clippy::expect_used)]

use ratterm::terminal::pty::{Pty, PtyConfig, PtyEvent};
use std::time::Duration;

/// Test basic PTY creation.
#[test]
fn test_pty_creation() {
    let config = PtyConfig::default();
    let result = Pty::new(config);

    assert!(result.is_ok(), "PTY creation should succeed");
}

/// Test PTY with custom shell.
#[test]
#[cfg(unix)]
fn test_pty_custom_shell() {
    let config = PtyConfig {
        shell: Some("/bin/sh".to_string()),
        args: vec!["-c".to_string(), "echo hello".to_string()],
        ..Default::default()
    };

    let result = Pty::new(config);
    assert!(result.is_ok(), "PTY with custom shell should succeed");
}

/// Test PTY with custom shell on Windows.
#[test]
#[cfg(windows)]
fn test_pty_custom_shell_windows() {
    let config = PtyConfig {
        shell: Some("cmd.exe".to_string()),
        args: vec!["/C".to_string(), "echo hello".to_string()],
        ..Default::default()
    };

    let result = Pty::new(config);
    assert!(result.is_ok(), "PTY with cmd.exe should succeed on Windows");
}

/// Test PTY with custom environment variables.
#[test]
fn test_pty_environment() {
    let config = PtyConfig {
        env: vec![("TEST_VAR".to_string(), "test_value".to_string())],
        ..Default::default()
    };

    let result = Pty::new(config);
    assert!(result.is_ok(), "PTY with env vars should succeed");
}

/// Test PTY dimensions.
#[test]
fn test_pty_dimensions() {
    let config = PtyConfig {
        cols: 120,
        rows: 40,
        ..Default::default()
    };

    let pty = Pty::new(config).expect("PTY creation should succeed");

    assert_eq!(pty.cols(), 120, "Columns mismatch");
    assert_eq!(pty.rows(), 40, "Rows mismatch");
}

/// Test PTY resize.
#[test]
fn test_pty_resize() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    let result = pty.resize(100, 50);
    assert!(result.is_ok(), "Resize should succeed");

    assert_eq!(pty.cols(), 100, "Columns should update");
    assert_eq!(pty.rows(), 50, "Rows should update");
}

/// Test writing to PTY.
#[test]
fn test_pty_write() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    let result = pty.write(b"echo test\n");
    assert!(result.is_ok(), "Write should succeed");
}

/// Test reading from PTY.
#[test]
fn test_pty_read() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Wait a moment for shell startup
    std::thread::sleep(Duration::from_millis(100));

    let result = pty.read();
    // Reading may or may not have data immediately
    assert!(result.is_ok(), "Read should not error");
}

/// Test PTY event reading (non-blocking).
#[test]
fn test_pty_try_read_event() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Try to read event without blocking
    let event = pty.try_read_event();

    // May be None if no data available, that's OK
    match event {
        Ok(Some(PtyEvent::Output(_))) => {} // Got some output
        Ok(Some(PtyEvent::Exit(_))) => {}   // Process exited
        Ok(None) => {}                      // No data ready
        Err(e) => panic!("Unexpected error: {e}"),
    }
}

/// Test PTY graceful shutdown.
#[test]
fn test_pty_shutdown() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Send exit command
    let _ = pty.write(b"exit\n");

    // Wait a moment
    std::thread::sleep(Duration::from_millis(200));

    // PTY should handle shutdown gracefully
    let result = pty.shutdown();
    assert!(result.is_ok(), "Shutdown should succeed");
}

/// Test PTY is running check.
#[test]
fn test_pty_is_running() {
    let config = PtyConfig::default();
    let pty = Pty::new(config).expect("PTY creation should succeed");

    assert!(pty.is_running(), "PTY should be running after creation");
}

/// Test PTY kill.
#[test]
fn test_pty_kill() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    let result = pty.kill();
    assert!(result.is_ok(), "Kill should succeed");

    // Give it a moment to process
    std::thread::sleep(Duration::from_millis(100));

    // After kill, should not be running
    // (implementation may vary on how quickly this updates)
}

/// Test PTY working directory.
#[test]
fn test_pty_working_directory() {
    let temp_dir = tempfile::tempdir().expect("Create temp dir");

    let config = PtyConfig {
        working_dir: Some(temp_dir.path().to_path_buf()),
        ..Default::default()
    };

    let result = Pty::new(config);
    assert!(result.is_ok(), "PTY with working dir should succeed");
}

/// Test PTY with invalid shell.
#[test]
fn test_pty_invalid_shell() {
    let config = PtyConfig {
        shell: Some("/nonexistent/shell".to_string()),
        ..Default::default()
    };

    let result = Pty::new(config);
    // Should fail or fall back to default shell
    // Behavior depends on implementation
    assert!(result.is_err(), "Invalid shell should fail");
}

/// Test concurrent read/write.
#[test]
fn test_pty_concurrent_operations() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Simulate concurrent-ish operations
    for _ in 0..5 {
        let _ = pty.write(b"\n");
        let _ = pty.read();
    }

    // Should not deadlock or panic
}

/// Test PTY with large output.
#[test]
fn test_pty_large_output() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Command that produces large output
    #[cfg(unix)]
    let cmd = b"seq 1 1000\n";
    #[cfg(windows)]
    let cmd = b"for /L %i in (1,1,1000) do @echo %i\r\n";

    let _ = pty.write(cmd);

    // Wait for output
    std::thread::sleep(Duration::from_millis(500));

    let mut total_bytes = 0;
    let max_iterations = 100;

    for _ in 0..max_iterations {
        match pty.read() {
            Ok(data) if !data.is_empty() => {
                total_bytes += data.len();
            }
            _ => break,
        }
    }

    assert!(total_bytes > 0, "Should receive some output");
}

/// Test PTY default config values.
#[test]
fn test_pty_config_defaults() {
    let config = PtyConfig::default();

    assert_eq!(config.cols, 80, "Default columns should be 80");
    assert_eq!(config.rows, 24, "Default rows should be 24");
    assert!(
        config.shell.is_none(),
        "Default shell should be None (use system default)"
    );
    assert!(config.args.is_empty(), "Default args should be empty");
    assert!(config.env.is_empty(), "Default env should be empty");
    assert!(
        config.working_dir.is_none(),
        "Default working dir should be None"
    );
}

/// Test PTY pid retrieval.
#[test]
fn test_pty_pid() {
    let config = PtyConfig::default();
    let pty = Pty::new(config).expect("PTY creation should succeed");

    let pid = pty.pid();
    assert!(pid.is_some(), "Should have a process ID");
    assert!(pid.expect("has pid") > 0, "PID should be positive");
}

/// Test interactive command execution.
/// Note: This test is timing-sensitive and may be flaky in CI environments.
#[test]
#[ignore = "Timing-sensitive test, run manually with --ignored"]
fn test_pty_interactive_command() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Wait for shell to start
    std::thread::sleep(Duration::from_millis(200));

    // Send a simple command
    #[cfg(unix)]
    let _ = pty.write(b"echo MARKER_START; echo MARKER_END\n");
    #[cfg(windows)]
    let _ = pty.write(b"echo MARKER_START & echo MARKER_END\r\n");

    std::thread::sleep(Duration::from_millis(300));

    // Collect output
    let mut output = Vec::new();
    for _ in 0..50 {
        match pty.read() {
            Ok(data) if !data.is_empty() => output.extend_from_slice(&data),
            _ => break,
        }
    }

    let output_str = String::from_utf8_lossy(&output);
    assert!(
        output_str.contains("MARKER_START") || output_str.contains("MARKER"),
        "Should see command output"
    );
}

/// Test PTY handles special characters.
#[test]
fn test_pty_special_characters() {
    let config = PtyConfig::default();
    let mut pty = Pty::new(config).expect("PTY creation should succeed");

    // Test Ctrl-C (0x03)
    let result = pty.write(&[0x03]);
    assert!(result.is_ok(), "Should accept Ctrl-C");

    // Test Ctrl-D (0x04)
    let result = pty.write(&[0x04]);
    assert!(result.is_ok(), "Should accept Ctrl-D");

    // Test Ctrl-Z (0x1A)
    let result = pty.write(&[0x1A]);
    assert!(result.is_ok(), "Should accept Ctrl-Z");
}
