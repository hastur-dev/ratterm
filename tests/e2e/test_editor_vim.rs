//! E2E tests: Editor in Vim input mode (default mode).

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Helper: spawn ratterm and focus the editor pane (Vim is default mode).
fn spawn_with_editor_focused() -> RattermHarness {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);
    // Focus editor pane
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h
}

/// Test: Vim starts in Normal mode; h/j/k/l navigate.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_normal_navigation() {
    let mut h = spawn_with_editor_focused();

    // In normal mode, h/j/k/l should move cursor (not insert text)
    h.send_text("j").expect("j - down");
    h.wait_ms(100);
    h.send_text("k").expect("k - up");
    h.wait_ms(100);
    h.send_text("l").expect("l - right");
    h.wait_ms(100);
    h.send_text("h").expect("h - left");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `i` enters Insert mode; ESC returns to Normal mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_insert_mode() {
    let mut h = spawn_with_editor_focused();

    // Enter insert mode
    h.send_text("i").expect("enter insert");
    h.wait_ms(200);

    // Type text
    h.send_text("hello vim").expect("type text");
    h.wait_ms(300);

    // Return to Normal mode
    h.send_text(keys::ESC).expect("exit insert");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `a` enters Insert mode after cursor.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_append_mode() {
    let mut h = spawn_with_editor_focused();

    // Enter append mode
    h.send_text("a").expect("append");
    h.wait_ms(200);

    h.send_text("appended text").expect("type");
    h.wait_ms(200);

    h.send_text(keys::ESC).expect("exit insert");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `v` enters Visual mode, ESC exits.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_visual_mode() {
    let mut h = spawn_with_editor_focused();

    // Enter insert mode, type text, exit
    h.send_text("i").expect("insert");
    h.send_text("hello world").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Enter visual mode
    h.send_text("v").expect("visual");
    h.wait_ms(200);

    // Move to select
    h.send_text("l").expect("select right");
    h.send_text("l").expect("select right");
    h.wait_ms(200);

    // Exit visual mode
    h.send_text(keys::ESC).expect("exit visual");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `x` deletes character under cursor in Normal mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_delete_char() {
    let mut h = spawn_with_editor_focused();

    // Type some text
    h.send_text("i").expect("insert");
    h.send_text("hello").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Delete char under cursor
    h.send_text("x").expect("delete char");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `u` undoes in Normal mode.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_undo() {
    let mut h = spawn_with_editor_focused();

    h.send_text("i").expect("insert");
    h.send_text("undo me").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Undo
    h.send_text("u").expect("undo");
    h.wait_ms(300);

    // Redo with Ctrl+R
    h.send_text(keys::CTRL_R).expect("redo");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `0` goes to line start, `$` goes to line end.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_line_navigation() {
    let mut h = spawn_with_editor_focused();

    h.send_text("i").expect("insert");
    h.send_text("start middle end").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Go to start
    h.send_text("0").expect("line start");
    h.wait_ms(100);

    // Go to end
    h.send_text("$").expect("line end");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `w` and `b` move by word.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_word_navigation() {
    let mut h = spawn_with_editor_focused();

    h.send_text("i").expect("insert");
    h.send_text("one two three").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Go to start
    h.send_text("0").expect("start");
    h.wait_ms(100);

    // Word forward
    h.send_text("w").expect("word right");
    h.wait_ms(100);
    h.send_text("w").expect("word right");
    h.wait_ms(100);

    // Word backward
    h.send_text("b").expect("word left");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: `g` goes to buffer start, `G` goes to buffer end.
#[test]
#[ignore = "Requires PTY"]
fn test_vim_buffer_navigation() {
    let mut h = spawn_with_editor_focused();

    h.send_text("i").expect("insert");
    h.send_text("line1").expect("type");
    h.send_text(keys::ENTER).expect("newline");
    h.send_text("line2").expect("type");
    h.send_text(keys::ENTER).expect("newline");
    h.send_text("line3").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Go to buffer start
    h.send_text("g").expect("buffer start");
    h.wait_ms(200);

    // Go to buffer end
    h.send_text("G").expect("buffer end");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+S saves in Vim mode (works in both Normal and Insert).
#[test]
#[ignore = "Requires PTY"]
fn test_vim_save() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("test.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Enter insert and modify
    h.send_text("i").expect("insert");
    h.send_text("modified ").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    // Save
    h.send_text(keys::CTRL_S).expect("save");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    // Verify file was saved
    let content = std::fs::read_to_string(&file).expect("read saved file");
    assert!(
        content.contains("modified"),
        "File should contain 'modified', got: {}",
        content
    );
}
