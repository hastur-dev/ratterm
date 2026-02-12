//! E2E tests: Command palette (Ctrl+Shift+P / F1).

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+Shift+P opens command palette.
#[test]
#[ignore = "Requires PTY"]
fn test_command_palette_opens() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Open command palette
    h.send_text(keys::CTRL_SHIFT_P).expect("open palette");
    h.wait_ms(500);

    // Close with ESC
    h.send_text(keys::ESC).expect("close palette");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: F1 also opens command palette.
#[test]
#[ignore = "Requires PTY"]
fn test_command_palette_f1() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // F1 opens command palette
    h.send_text(keys::F1).expect("F1");
    h.wait_ms(500);

    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Command palette accepts typed input.
#[test]
#[ignore = "Requires PTY"]
fn test_command_palette_search() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_SHIFT_P).expect("open palette");
    h.wait_ms(500);

    // Type a command search
    h.send_text("mode").expect("type search");
    h.wait_ms(300);

    // Navigate results
    h.send_text(keys::DOWN).expect("down");
    h.wait_ms(100);
    h.send_text(keys::UP).expect("up");
    h.wait_ms(100);

    // Cancel
    h.send_text(keys::ESC).expect("cancel");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Open and close palette multiple times doesn't crash.
#[test]
#[ignore = "Requires PTY"]
fn test_command_palette_open_close_cycle() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    for _ in 0..5 {
        h.send_text(keys::CTRL_SHIFT_P).expect("open");
        h.wait_ms(300);
        h.send_text(keys::ESC).expect("close");
        h.wait_ms(200);
    }

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
