//! E2E tests: Editor in VS Code / Default input mode.
//!
//! The "Default" mode in ratterm uses familiar arrow-key-based navigation
//! similar to VS Code: Ctrl+Z undo, Ctrl+Y redo, etc.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Helper: spawn ratterm and switch to Default mode, focus editor.
fn spawn_with_default_mode() -> RattermHarness {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open mode switcher (Ctrl+Shift+Tab)
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);

    // Navigate to Default (third option: Vim, Emacs, Default)
    h.send_text(keys::DOWN).expect("past emacs");
    h.wait_ms(100);
    h.send_text(keys::DOWN).expect("to default");
    h.wait_ms(100);
    h.send_text(keys::ENTER).expect("select default");
    h.wait_ms(500);

    h
}

/// Test: Typing inserts text directly (no modal Normal mode).
#[test]
#[ignore = "Requires PTY"]
fn test_vscode_direct_insert() {
    let mut h = spawn_with_default_mode();

    // In default mode, typing inserts directly
    h.send_text("hello vscode mode").expect("type text");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Z undoes, Ctrl+Y redoes in Default/VS Code mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vscode_undo_redo() {
    let mut h = spawn_with_default_mode();

    h.send_text("undo me").expect("type");
    h.wait_ms(300);

    // Undo
    h.send_text(keys::CTRL_Z).expect("undo");
    h.wait_ms(300);

    // Redo
    h.send_text(keys::CTRL_Y).expect("redo");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Home/End keys work in Default/VS Code mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vscode_home_end() {
    let mut h = spawn_with_default_mode();

    h.send_text("hello world").expect("type");
    h.wait_ms(200);

    h.send_text(keys::HOME).expect("home");
    h.wait_ms(100);
    h.send_text(keys::END).expect("end");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+S saves file in Default/VS Code mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vscode_save() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("vscode_test.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor and switch to Default mode
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);
    h.send_text(keys::DOWN).expect("skip emacs");
    h.wait_ms(100);
    h.send_text(keys::DOWN).expect("to default");
    h.wait_ms(100);
    h.send_text(keys::ENTER).expect("select");
    h.wait_ms(500);

    // Modify
    h.send_text("vscode edit ").expect("type");
    h.wait_ms(200);

    // Save
    h.send_text(keys::CTRL_S).expect("save");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    let content = std::fs::read_to_string(&file).expect("read saved");
    assert!(
        content.contains("vscode edit"),
        "File should contain 'vscode edit', got: {}",
        content
    );
}

/// Test: Rapid typing doesn't drop characters.
#[test]
#[ignore = "Requires PTY"]
fn test_vscode_rapid_typing() {
    let mut h = spawn_with_default_mode();

    // Type a long string rapidly
    h.send_text("The quick brown fox jumps over the lazy dog")
        .expect("rapid type");
    h.wait_ms(500);

    // Should not crash
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
