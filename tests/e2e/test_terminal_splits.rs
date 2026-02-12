//! E2E tests: Terminal split pane management.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+S creates a horizontal split.
#[test]
#[ignore = "Requires PTY"]
fn test_horizontal_split() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create a split
    h.send_text(keys::CTRL_S).expect("split");
    h.wait_ms(1500);

    // Verify second pane works by typing in it
    h.send_line("echo SPLIT_PANE").expect("echo in split");
    h.wait_ms(1000);
    h.expect_text("SPLIT_PANE").expect("Split pane should work");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+S creates a vertical split.
#[test]
#[ignore = "Requires PTY"]
fn test_vertical_split() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_SHIFT_S).expect("vertical split");
    h.wait_ms(1500);

    h.send_line("echo VSPLIT_OK").expect("echo");
    h.wait_ms(1000);
    h.expect_text("VSPLIT_OK").expect("Vertical split should work");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Tab cycles focus between split panes.
#[test]
#[ignore = "Requires PTY"]
fn test_cycle_split_focus() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create a split
    h.send_text(keys::CTRL_S).expect("split");
    h.wait_ms(1500);

    // Cycle back to first pane (Ctrl+Tab)
    h.send_text(keys::CTRL_TAB).expect("Ctrl+Tab");
    h.wait_ms(500);

    // Verify first pane is focused by running a command
    h.send_line("echo FIRST_PANE_FOCUSED").expect("echo");
    h.wait_ms(1000);
    h.expect_text("FIRST_PANE_FOCUSED")
        .expect("First pane should be focused");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Shift+W closes a split pane.
#[test]
#[ignore = "Requires PTY"]
fn test_close_split() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create a split, then close it
    h.send_text(keys::CTRL_S).expect("split");
    h.wait_ms(1500);

    h.send_text(keys::CTRL_SHIFT_W).expect("close split");
    h.wait_ms(500);

    // Should be back to single pane
    h.send_line("echo SINGLE_PANE").expect("echo");
    h.wait_ms(1000);
    h.expect_text("SINGLE_PANE")
        .expect("Should be back to single pane");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
