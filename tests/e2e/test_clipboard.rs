//! E2E tests: Clipboard operations (copy/paste).

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+Shift+C copies selected text (Vim visual mode).
#[test]
#[ignore = "Requires PTY"]
fn test_copy_selection_vim() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("clipboard.txt");
    std::fs::write(&file, "copy this text\nand this line\n").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Enter visual mode and select some text
    h.send_text("v").expect("visual");
    h.wait_ms(100);
    h.send_text("l").expect("select right");
    h.send_text("l").expect("select right");
    h.send_text("l").expect("select right");
    h.wait_ms(100);

    // Copy
    h.send_text(keys::CTRL_SHIFT_C).expect("copy");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+V pastes clipboard content.
#[test]
#[ignore = "Requires PTY"]
fn test_paste() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Enter insert mode
    h.send_text("i").expect("insert");
    h.wait_ms(200);

    // Paste (whatever is in clipboard)
    h.send_text(keys::CTRL_V).expect("paste");
    h.wait_ms(300);

    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Copy-paste roundtrip in editor.
#[test]
#[ignore = "Requires PTY"]
fn test_copy_paste_roundtrip() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("roundtrip.txt");
    std::fs::write(&file, "abcdef\n").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Select text in visual mode
    h.send_text("v").expect("visual");
    h.send_text("l").expect("right");
    h.send_text("l").expect("right");
    h.wait_ms(100);

    // Copy
    h.send_text(keys::CTRL_SHIFT_C).expect("copy");
    h.wait_ms(300);

    // Move to end of line
    h.send_text("$").expect("end");
    h.wait_ms(100);

    // Append and paste
    h.send_text("a").expect("append");
    h.send_text(keys::CTRL_V).expect("paste");
    h.wait_ms(300);

    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
