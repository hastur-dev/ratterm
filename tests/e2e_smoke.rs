//! E2E Smoke Test: Quick validation that the most critical paths work.
//!
//! This test file is intended as a fast sanity check. It exercises the key
//! features in a single test run without deep verification, to confirm
//! nothing is fundamentally broken.

#[path = "e2e/common/mod.rs"]
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

/// Smoke test: --version works (non-interactive).
#[test]
fn smoke_version() {
    let output = std::process::Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ratterm"), "Version output: {}", stdout);
    assert!(output.status.success());
}

/// Smoke test: --verify works (non-interactive).
#[test]
fn smoke_verify() {
    let output = std::process::Command::new(binary_path())
        .arg("--verify")
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("verify-ok"), "Verify output: {}", stdout);
    assert!(output.status.success());
}

/// Smoke test: App launches and quits cleanly.
#[test]
#[ignore = "Requires PTY"]
fn smoke_launch_and_quit() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
}

/// Smoke test: Open a file and quit.
#[test]
#[ignore = "Requires PTY"]
fn smoke_open_file() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("smoke.rs");
    std::fs::write(&file, "fn main() {}\n").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
}

/// Smoke test: Focus switch between panes.
#[test]
#[ignore = "Requires PTY"]
fn smoke_focus_switching() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Switch to editor
    h.send_text(keys::ALT_RIGHT).expect("editor");
    h.wait_ms(300);

    // Switch back to terminal
    h.send_text(keys::ALT_LEFT).expect("terminal");
    h.wait_ms(300);

    // Toggle with Alt+Tab
    h.send_text(keys::ALT_TAB).expect("toggle");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Smoke test: Toggle IDE pane visibility.
#[test]
#[ignore = "Requires PTY"]
fn smoke_toggle_ide() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Hide IDE
    h.send_text(keys::CTRL_I).expect("hide");
    h.wait_ms(300);

    // Show IDE
    h.send_text(keys::CTRL_I).expect("show");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Smoke test: Editor typing works (Vim insert mode).
#[test]
#[ignore = "Requires PTY"]
fn smoke_editor_typing() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("editor");
    h.wait_ms(500);

    // Enter insert, type, exit
    h.send_text("i").expect("insert");
    h.send_text("smoke test typing").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Smoke test: File browser opens and closes.
#[test]
#[ignore = "Requires PTY"]
fn smoke_file_browser() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_O).expect("open browser");
    h.wait_ms(500);

    h.send_text(keys::ESC).expect("close browser");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Smoke test: Resize pane divider.
#[test]
#[ignore = "Requires PTY"]
fn smoke_resize_panes() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::ALT_CLOSE_BRACKET).expect("expand");
    h.wait_ms(200);
    h.send_text(keys::ALT_OPEN_BRACKET).expect("shrink");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Smoke test: Save file roundtrip.
#[test]
#[ignore = "Requires PTY"]
fn smoke_save_file() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("smoke_save.txt");
    std::fs::write(&file, "").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor, type, save
    h.send_text(keys::ALT_RIGHT).expect("editor");
    h.wait_ms(500);
    h.send_text("i").expect("insert");
    h.send_text("smoke saved").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);
    h.send_text(keys::CTRL_S).expect("save");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    let content = std::fs::read_to_string(&file).expect("read");
    assert!(
        content.contains("smoke saved"),
        "Saved file should contain 'smoke saved', got: {}",
        content
    );
}
