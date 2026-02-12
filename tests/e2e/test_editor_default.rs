//! E2E tests: Editor in Default (arrow-key) input mode.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Helper: spawn ratterm and focus the editor pane.
fn spawn_with_editor_focused() -> RattermHarness {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);
    // Focus editor pane
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h
}

/// Test: Arrow keys move cursor in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_arrow_navigation() {
    let mut h = spawn_with_editor_focused();

    // Type some text first
    h.send_text("hello world").expect("type text");
    h.wait_ms(300);

    // Move left with arrow keys
    h.send_text(keys::LEFT).expect("left");
    h.send_text(keys::LEFT).expect("left");
    h.wait_ms(200);

    // Move right
    h.send_text(keys::RIGHT).expect("right");
    h.wait_ms(200);

    // Move up/down (no-op on single line, but shouldn't crash)
    h.send_text(keys::UP).expect("up");
    h.send_text(keys::DOWN).expect("down");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Home/End keys move to line start/end in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_home_end() {
    let mut h = spawn_with_editor_focused();

    h.send_text("hello world").expect("type text");
    h.wait_ms(300);

    // Home goes to start of line
    h.send_text(keys::HOME).expect("home");
    h.wait_ms(200);

    // End goes to end of line
    h.send_text(keys::END).expect("end");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Left/Right move by word in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_word_navigation() {
    let mut h = spawn_with_editor_focused();

    h.send_text("hello beautiful world").expect("type text");
    h.wait_ms(300);

    // Word left
    h.send_text(keys::CTRL_LEFT).expect("word left");
    h.wait_ms(200);
    h.send_text(keys::CTRL_LEFT).expect("word left");
    h.wait_ms(200);

    // Word right
    h.send_text(keys::CTRL_RIGHT).expect("word right");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Z undoes, Ctrl+Y redoes in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_undo_redo() {
    let mut h = spawn_with_editor_focused();

    h.send_text("hello").expect("type");
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

/// Test: Delete key removes character at cursor in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_delete_key() {
    let mut h = spawn_with_editor_focused();

    h.send_text("hello").expect("type");
    h.wait_ms(200);

    // Move left then delete forward
    h.send_text(keys::HOME).expect("home");
    h.wait_ms(100);
    h.send_text(keys::DELETE).expect("delete");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Backspace removes character behind cursor in Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_default_backspace() {
    let mut h = spawn_with_editor_focused();

    h.send_text("hello").expect("type");
    h.wait_ms(200);

    // Backspace at end
    h.send_text(keys::BACKSPACE).expect("backspace");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
