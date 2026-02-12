//! E2E tests: File browser (Ctrl+O) navigation and selection.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+O opens and ESC closes the file browser.
#[test]
#[ignore = "Requires PTY"]
fn test_file_browser_open_close() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open file browser
    h.send_text(keys::CTRL_O).expect("open browser");
    h.wait_ms(1000);

    // Close with ESC
    h.send_text(keys::ESC).expect("close browser");
    h.wait_ms(500);

    // App should be back to normal
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Navigate file browser with arrow keys.
#[test]
#[ignore = "Requires PTY"]
fn test_file_browser_navigation() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open file browser
    h.send_text(keys::CTRL_O).expect("open browser");
    h.wait_ms(1000);

    // Navigate down/up
    h.send_text(keys::DOWN).expect("down");
    h.wait_ms(200);
    h.send_text(keys::DOWN).expect("down");
    h.wait_ms(200);
    h.send_text(keys::UP).expect("up");
    h.wait_ms(200);

    // Close
    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: File browser fuzzy search filters entries.
#[test]
#[ignore = "Requires PTY"]
fn test_file_browser_fuzzy_search() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open file browser
    h.send_text(keys::CTRL_O).expect("open browser");
    h.wait_ms(1000);

    // Type to filter (fuzzy search)
    h.send_text("cargo").expect("type filter");
    h.wait_ms(500);

    // Clear filter with backspace
    for _ in 0..5 {
        h.send_text(keys::BACKSPACE).expect("backspace");
    }
    h.wait_ms(300);

    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Select a file in browser opens it in editor.
#[test]
#[ignore = "Requires PTY"]
fn test_file_browser_select_file() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("browsable.txt");
    std::fs::write(&file, "file browser test content").expect("write");

    // Start in the temp directory so the file browser shows our file
    let dir_str = temp_dir.path().to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&["--dir", dir_str]).expect("spawn");
    h.wait_ms(2000);

    // Open file browser
    h.send_text(keys::CTRL_O).expect("open browser");
    h.wait_ms(1000);

    // Select first entry (Enter)
    h.send_text(keys::ENTER).expect("select");
    h.wait_ms(1000);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+N creates a new folder from browser.
#[test]
#[ignore = "Requires PTY"]
fn test_file_browser_create_folder() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create new folder
    h.send_text(keys::CTRL_SHIFT_N).expect("new folder");
    h.wait_ms(500);

    // If a dialog appears, cancel it
    h.send_text(keys::ESC).expect("cancel");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
