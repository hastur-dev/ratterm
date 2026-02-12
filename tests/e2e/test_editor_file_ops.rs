//! E2E tests: Editor file operations (open, save, save-as, new file).

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Open a file via CLI argument and verify it loads without crashing.
#[test]
#[ignore = "Requires PTY"]
fn test_open_file_via_arg() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("hello.rs");
    std::fs::write(&file, "fn main() {\n    println!(\"hello\");\n}\n").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn with file");
    h.wait_ms(2000);

    // Should have opened without crashing
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
}

/// Test: Create a new file with Ctrl+N.
#[test]
#[ignore = "Requires PTY"]
fn test_create_new_file() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Ctrl+N creates a new file (may open a dialog)
    h.send_text(keys::CTRL_N).expect("new file");
    h.wait_ms(1000);

    // If a popup appears, type a filename and confirm
    h.send_text("newfile.txt").expect("type filename");
    h.send_text(keys::ENTER).expect("confirm");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Save a modified file with Ctrl+S.
#[test]
#[ignore = "Requires PTY"]
fn test_save_file() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("save_test.txt");
    std::fs::write(&file, "original content\n").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Enter insert mode (Vim is default), add text
    h.send_text("i").expect("insert mode");
    h.send_text("SAVED_TEXT ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Save
    h.send_text(keys::CTRL_S).expect("save");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    // Verify file was modified
    let content = std::fs::read_to_string(&file).expect("read");
    assert!(
        content.contains("SAVED_TEXT"),
        "File should contain 'SAVED_TEXT', got: {}",
        content
    );
}

/// Test: Open a file via file browser (Ctrl+O).
#[test]
#[ignore = "Requires PTY"]
fn test_open_file_via_browser() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open file browser
    h.send_text(keys::CTRL_O).expect("file browser");
    h.wait_ms(1000);

    // Close file browser without selecting (ESC)
    h.send_text(keys::ESC).expect("close browser");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Open a large file doesn't crash.
#[test]
#[ignore = "Requires PTY"]
fn test_open_large_file() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("large.txt");

    // Generate a file with 10,000 lines
    let content: String = (0..10_000)
        .map(|i| format!("Line {i}: This is a test line with some content.\n"))
        .collect();
    std::fs::write(&file, &content).expect("write large file");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(3000); // Give extra time for large file

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);
}
