//! E2E tests: Terminal tab management.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+T creates a new terminal tab.
#[test]
#[ignore = "Requires PTY"]
fn test_create_new_tab() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create a new tab
    h.send_text(keys::CTRL_T).expect("Ctrl+T");
    h.wait_ms(1500); // Wait for new shell to start

    // New tab should have a fresh shell — echo something unique
    h.send_line("echo NEW_TAB_WORKS").expect("send echo");
    h.wait_ms(1000);
    h.expect_text("NEW_TAB_WORKS").expect("New tab should work");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+Left/Right switches between terminal tabs.
#[test]
#[ignore = "Requires PTY"]
fn test_switch_tabs() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create second tab
    h.send_text(keys::CTRL_T).expect("new tab");
    h.wait_ms(1500);

    // Switch back to first tab
    h.send_text(keys::CTRL_LEFT).expect("switch left");
    h.wait_ms(500);

    // Switch forward to second tab
    h.send_text(keys::CTRL_RIGHT).expect("switch right");
    h.wait_ms(500);

    // Verify second tab works
    h.send_line("echo SECOND_TAB_ACTIVE").expect("echo");
    h.wait_ms(1000);
    h.expect_text("SECOND_TAB_ACTIVE")
        .expect("Second tab should be active");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Ctrl+W closes the current terminal tab.
#[test]
#[ignore = "Requires PTY"]
fn test_close_tab() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Create a second tab
    h.send_text(keys::CTRL_T).expect("new tab");
    h.wait_ms(1500);

    // Close the second tab
    h.send_text(keys::CTRL_W).expect("close tab");
    h.wait_ms(500);

    // Should be back on the first tab — verify by running a command
    h.send_line("echo BACK_ON_FIRST").expect("verify");
    h.wait_ms(1000);
    h.expect_text("BACK_ON_FIRST")
        .expect("Should be on first tab");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
