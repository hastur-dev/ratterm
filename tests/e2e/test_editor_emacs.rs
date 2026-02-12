//! E2E tests: Editor in Emacs input mode.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Helper: spawn ratterm with Emacs keybindings and focus the editor.
fn spawn_with_emacs_editor() -> RattermHarness {
    let h = RattermHarness::spawn().expect("spawn");
    // Create a .ratrc with mode=emacs
    let ratrc = h.create_ratrc("mode = emacs\n");
    // Respawn with the config
    drop(h);
    let mut h =
        RattermHarness::spawn_with_args(&["--config", ratrc.to_str().expect("path")])
            .expect("spawn with emacs config");
    h.wait_ms(2000);
    // Focus editor pane
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);
    h
}

/// Helper: simpler spawn — just spawn with default config, switch mode via
/// Ctrl+Shift+Tab mode switcher.
fn spawn_and_switch_to_emacs() -> RattermHarness {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open mode switcher (Ctrl+Shift+Tab)
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);

    // Navigate to Emacs (it's second option: Vim, Emacs, Default)
    h.send_text(keys::DOWN).expect("navigate to emacs");
    h.wait_ms(100);
    h.send_text(keys::ENTER).expect("select emacs");
    h.wait_ms(500);

    h
}

/// Test: Ctrl+F moves right, Ctrl+B moves left in Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_cursor_movement() {
    let mut h = spawn_and_switch_to_emacs();

    // Type some text (Emacs is always in insert mode)
    h.send_text("hello emacs").expect("type");
    h.wait_ms(300);

    // Ctrl+B = move left
    h.send_text(keys::CTRL_B).expect("move left");
    h.wait_ms(100);

    // Ctrl+F = move right
    h.send_text(keys::CTRL_F).expect("move right");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+P moves up, Ctrl+N moves down in Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_vertical_movement() {
    let mut h = spawn_and_switch_to_emacs();

    h.send_text("line one").expect("type");
    h.send_text(keys::ENTER).expect("newline");
    h.send_text("line two").expect("type");
    h.wait_ms(300);

    // Ctrl+P = up
    h.send_text(keys::CTRL_P).expect("up");
    h.wait_ms(100);

    // Ctrl+N = down
    h.send_text(keys::CTRL_N).expect("down");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+A moves to start, Ctrl+E moves to end in Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_line_start_end() {
    let mut h = spawn_and_switch_to_emacs();

    h.send_text("hello emacs world").expect("type");
    h.wait_ms(300);

    // Ctrl+A = start of line
    h.send_text(keys::CTRL_A).expect("line start");
    h.wait_ms(100);

    // Ctrl+E = end of line
    h.send_text(keys::CTRL_E).expect("line end");
    h.wait_ms(100);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+D deletes character at cursor in Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_delete_char() {
    let mut h = spawn_and_switch_to_emacs();

    h.send_text("hello").expect("type");
    h.wait_ms(200);

    // Move to start, delete forward
    h.send_text(keys::CTRL_A).expect("start");
    h.wait_ms(100);
    h.send_text(keys::CTRL_D).expect("delete");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+K deletes from cursor to end of line (kill line).
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_kill_line() {
    let mut h = spawn_and_switch_to_emacs();

    h.send_text("hello world to kill").expect("type");
    h.wait_ms(200);

    // Move to middle-ish
    h.send_text(keys::CTRL_A).expect("start");
    h.wait_ms(100);
    // Move right a few chars
    for _ in 0..5 {
        h.send_text(keys::CTRL_F).expect("right");
    }
    h.wait_ms(100);

    // Kill line from cursor to end
    h.send_text(keys::CTRL_K).expect("kill line");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+X saves file in Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_emacs_save() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let file = temp_dir.path().join("emacs_test.txt");
    std::fs::write(&file, "original").expect("write");

    let file_str = file.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&[file_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Switch to emacs mode
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);
    h.send_text(keys::DOWN).expect("emacs");
    h.send_text(keys::ENTER).expect("select");
    h.wait_ms(500);

    // Modify
    h.send_text("emacs edit ").expect("type");
    h.wait_ms(200);

    // Save with Ctrl+X (Emacs save)
    h.send_text(keys::CTRL_X).expect("save");
    h.wait_ms(500);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(500);

    let content = std::fs::read_to_string(&file).expect("read saved");
    assert!(
        content.contains("emacs edit"),
        "File should contain 'emacs edit', got: {}",
        content
    );
}
