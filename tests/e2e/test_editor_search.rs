//! E2E tests: Editor search/find functionality.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+F opens search in file.
#[test]
#[ignore = "Requires PTY"]
fn test_search_in_file_opens() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("search_test.txt");
    std::fs::write(
        &file,
        "line one\nline two\nline three\nfind me here\nline five\n",
    )
    .expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open search
    h.send_text(keys::CTRL_F).expect("Ctrl+F search");
    h.wait_ms(500);

    // Type search query
    h.send_text("find me").expect("type query");
    h.wait_ms(300);

    // Close search with ESC
    h.send_text(keys::ESC).expect("close search");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+F opens search across files.
#[test]
#[ignore = "Requires PTY"]
fn test_search_across_files() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open search across files
    h.send_text(keys::CTRL_SHIFT_F).expect("Ctrl+Shift+F");
    h.wait_ms(500);

    // Type query
    h.send_text("test query").expect("type");
    h.wait_ms(300);

    // Close with ESC
    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+D opens directory search.
#[test]
#[ignore = "Requires PTY"]
fn test_search_directories() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_SHIFT_D).expect("dir search");
    h.wait_ms(500);

    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+E opens file search.
#[test]
#[ignore = "Requires PTY"]
fn test_search_files() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_SHIFT_E).expect("file search");
    h.wait_ms(500);

    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Search and close cycle doesn't crash.
#[test]
#[ignore = "Requires PTY"]
fn test_search_open_close_cycle() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open and close search multiple times
    for _ in 0..3 {
        h.send_text(keys::CTRL_F).expect("open search");
        h.wait_ms(300);
        h.send_text(keys::ESC).expect("close search");
        h.wait_ms(300);
    }

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
