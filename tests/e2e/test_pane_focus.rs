//! E2E tests: Switching focus between terminal and editor panes.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Alt+Right moves focus to editor, Alt+Left moves back to terminal.
#[test]
#[ignore = "Requires PTY"]
fn test_focus_switch_alt_arrows() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor pane
    h.send_text(keys::ALT_RIGHT).expect("Alt+Right to editor");
    h.wait_ms(500);

    // Focus back to terminal
    h.send_text(keys::ALT_LEFT).expect("Alt+Left to terminal");
    h.wait_ms(500);

    // Verify terminal is focused by typing a command
    h.send_line("echo TERMINAL_FOCUSED").expect("echo");
    h.wait_ms(1000);
    h.expect_text("TERMINAL_FOCUSED")
        .expect("Terminal should be focused");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Alt+Tab toggles between panes.
#[test]
#[ignore = "Requires PTY"]
fn test_focus_toggle_alt_tab() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Toggle to editor
    h.send_text(keys::ALT_TAB).expect("Alt+Tab");
    h.wait_ms(500);

    // Toggle back to terminal
    h.send_text(keys::ALT_TAB).expect("Alt+Tab again");
    h.wait_ms(500);

    // Verify terminal works
    h.send_line("echo TOGGLE_WORKS").expect("echo");
    h.wait_ms(1000);
    h.expect_text("TOGGLE_WORKS").expect("Toggle should work");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+I toggles IDE pane visibility.
#[test]
#[ignore = "Requires PTY"]
fn test_toggle_ide_pane() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Hide IDE pane
    h.send_text(keys::CTRL_I).expect("Ctrl+I hide");
    h.wait_ms(500);

    // Show IDE pane again
    h.send_text(keys::CTRL_I).expect("Ctrl+I show");
    h.wait_ms(500);

    // Terminal should still work
    h.send_line("echo IDE_TOGGLE_OK").expect("echo");
    h.wait_ms(1000);
    h.expect_text("IDE_TOGGLE_OK")
        .expect("IDE toggle should work");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
