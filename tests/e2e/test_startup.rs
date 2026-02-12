//! E2E tests: Application startup, version display, and clean quit.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Binary path helper for non-interactive subprocess tests.
fn binary_path() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    #[cfg(windows)]
    let bin = manifest_dir.join("target").join("release").join("rat.exe");
    #[cfg(not(windows))]
    let bin = manifest_dir.join("target").join("release").join("rat");
    assert!(bin.exists(), "Release binary not found at {:?}", bin);
    bin
}

/// Test: `rat --version` prints version and exits (non-interactive).
#[test]
fn test_version_flag() {
    let output = std::process::Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("Failed to run rat --version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ratterm v"),
        "Expected version string, got: {}",
        stdout
    );
    assert!(output.status.success(), "rat --version should exit 0");
}

/// Test: `rat --verify` prints verification and exits (non-interactive).
#[test]
fn test_verify_flag() {
    let output = std::process::Command::new(binary_path())
        .arg("--verify")
        .output()
        .expect("Failed to run rat --verify");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("verify-ok"),
        "Expected verify-ok string, got: {}",
        stdout
    );
    assert!(output.status.success(), "rat --verify should exit 0");
}

/// Test: App launches and quits cleanly with Ctrl+Q.
#[test]
#[ignore = "Requires PTY environment; run with: cargo test -- --ignored"]
fn test_app_launches_and_quits() {
    let mut h = RattermHarness::spawn().expect("Failed to spawn ratterm");

    // Give the TUI time to render
    h.wait_ms(2000);

    // Quit cleanly
    h.send_text(keys::CTRL_Q).expect("Failed to send Ctrl+Q");
    h.wait_ms(500);
}

/// Test: App opens a file passed as CLI argument.
#[test]
#[ignore = "Requires PTY environment"]
fn test_open_with_file_arg() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("hello.rs");
    std::fs::write(
        &test_file,
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .expect("Failed to write test file");

    let file_str = test_file.to_str().expect("Invalid path");
    let mut h =
        RattermHarness::spawn_with_args(&[file_str]).expect("Failed to spawn ratterm with file");

    h.wait_ms(2000);

    // Quit — the editor should have opened the file without crashing
    h.send_text(keys::CTRL_Q).expect("Failed to quit");
    h.wait_ms(500);
}
