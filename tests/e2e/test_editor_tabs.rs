//! E2E tests: Editor tab management.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Open multiple files creates multiple editor tabs.
#[test]
#[ignore = "Requires PTY"]
fn test_open_multiple_files() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");

    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    std::fs::write(&file1, "content one").expect("write");
    std::fs::write(&file2, "content two").expect("write");

    let f1 = file1.to_str().expect("path");
    let f2 = file2.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[f1, f2]).expect("spawn");
    h.wait_ms(2000);

    // Both files should have opened without crash
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
}

/// Test: Alt+Shift+Right switches to next file tab.
#[test]
#[ignore = "Requires PTY"]
fn test_switch_editor_tabs() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");

    let file1 = temp_dir.path().join("tab1.txt");
    let file2 = temp_dir.path().join("tab2.txt");
    std::fs::write(&file1, "tab one").expect("write");
    std::fs::write(&file2, "tab two").expect("write");

    let f1 = file1.to_str().expect("path");
    let f2 = file2.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[f1, f2]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Switch to next tab
    h.send_text(keys::ALT_SHIFT_RIGHT).expect("next tab");
    h.wait_ms(500);

    // Switch back to previous tab
    h.send_text(keys::ALT_SHIFT_LEFT).expect("prev tab");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+W closes the current editor tab.
#[test]
#[ignore = "Requires PTY"]
fn test_close_editor_tab() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");

    let file1 = temp_dir.path().join("close1.txt");
    let file2 = temp_dir.path().join("close2.txt");
    std::fs::write(&file1, "keep this").expect("write");
    std::fs::write(&file2, "close this").expect("write");

    let f1 = file1.to_str().expect("path");
    let f2 = file2.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[f1, f2]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Switch to second tab
    h.send_text(keys::ALT_SHIFT_RIGHT).expect("next tab");
    h.wait_ms(500);

    // Close it
    h.send_text(keys::CTRL_W).expect("close tab");
    h.wait_ms(500);

    // Should be back on first tab
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: New editor tab via Ctrl+T (when editor is focused).
#[test]
#[ignore = "Requires PTY"]
fn test_new_editor_tab() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Create new tab
    h.send_text(keys::CTRL_T).expect("new tab");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
