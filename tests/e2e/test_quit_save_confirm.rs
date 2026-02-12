//! E2E tests: Quit/save confirmation dialog.
//!
//! When the editor has unsaved changes, Ctrl+Q should show a confirmation dialog:
//! - Y: Save and quit
//! - N: Quit without saving
//! - C / ESC: Cancel quit

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+Q with no changes quits immediately.
#[test]
#[ignore = "Requires PTY"]
fn test_quit_no_changes() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
    // Should have quit without dialog
}

/// Test: Ctrl+Q with unsaved changes shows dialog; ESC cancels.
#[test]
#[ignore = "Requires PTY"]
fn test_quit_cancel_with_esc() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("unsaved.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor and modify the file
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h.send_text("i").expect("insert");
    h.send_text("modified ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Try to quit — should show confirmation
    h.send_text(keys::CTRL_Q).expect("quit attempt");
    h.wait_ms(500);

    // Cancel with ESC
    h.send_text(keys::ESC).expect("cancel quit");
    h.wait_ms(500);

    // App should still be running — quit for real
    h.send_text(keys::CTRL_Q).expect("quit again");
    h.wait_ms(300);
    // Dismiss dialog with N (don't save)
    h.send_text("n").expect("don't save");
    h.wait_ms(500);

    // File should NOT be modified
    let content = std::fs::read_to_string(&file).expect("read");
    assert_eq!(content, "original", "File should not have been saved");
}

/// Test: Ctrl+Q with unsaved changes, press C to cancel.
#[test]
#[ignore = "Requires PTY"]
fn test_quit_cancel_with_c() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("cancel_c.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Modify
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h.send_text("i").expect("insert");
    h.send_text("change ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Quit attempt
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    // Cancel with C
    h.send_text("c").expect("cancel");
    h.wait_ms(500);

    // Force quit without save
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
    h.send_text("n").expect("don't save");
    h.wait_ms(500);
}

/// Test: Ctrl+Q with changes, Y saves and quits.
#[test]
#[ignore = "Requires PTY"]
fn test_quit_save_with_y() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("save_quit.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Modify
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h.send_text("i").expect("insert");
    h.send_text("SAVED_ON_QUIT ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Quit and save
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
    h.send_text("y").expect("save and quit");
    h.wait_ms(1000);

    // File should contain the modification
    let content = std::fs::read_to_string(&file).expect("read");
    assert!(
        content.contains("SAVED_ON_QUIT"),
        "File should contain 'SAVED_ON_QUIT', got: {}",
        content
    );
}

/// Test: Ctrl+Q with changes, N quits without saving.
#[test]
#[ignore = "Requires PTY"]
fn test_quit_no_save_with_n() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("no_save.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Modify
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h.send_text("i").expect("insert");
    h.send_text("SHOULD_NOT_PERSIST ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Quit without saving
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
    h.send_text("n").expect("quit no save");
    h.wait_ms(1000);

    // File should NOT contain the modification
    let content = std::fs::read_to_string(&file).expect("read");
    assert!(
        !content.contains("SHOULD_NOT_PERSIST"),
        "File should NOT contain modification, got: {}",
        content
    );
}
