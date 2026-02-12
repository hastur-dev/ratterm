//! E2E tests: Resizing the split pane divider.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Alt+] expands the split, Alt+[ shrinks it. Both panes still work after.
#[test]
#[ignore = "Requires PTY"]
fn test_resize_split() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Expand terminal pane
    h.send_text(keys::ALT_CLOSE_BRACKET)
        .expect("Alt+]");
    h.wait_ms(300);
    h.send_text(keys::ALT_CLOSE_BRACKET)
        .expect("Alt+] again");
    h.wait_ms(300);

    // Shrink terminal pane
    h.send_text(keys::ALT_OPEN_BRACKET)
        .expect("Alt+[");
    h.wait_ms(300);

    // Verify terminal still works after resize
    h.send_line("echo RESIZE_WORKS").expect("echo");
    h.wait_ms(1000);
    h.expect_text("RESIZE_WORKS")
        .expect("Terminal should work after resize");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Multiple resize operations don't crash.
#[test]
#[ignore = "Requires PTY"]
fn test_resize_multiple_times() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Rapidly resize back and forth
    for _ in 0..5 {
        h.send_text(keys::ALT_CLOSE_BRACKET)
            .expect("expand");
        h.wait_ms(100);
    }
    for _ in 0..5 {
        h.send_text(keys::ALT_OPEN_BRACKET)
            .expect("shrink");
        h.wait_ms(100);
    }

    h.wait_ms(500);

    // Verify app is still responsive
    h.send_line("echo STILL_ALIVE").expect("echo");
    h.wait_ms(1000);
    h.expect_text("STILL_ALIVE")
        .expect("App should survive rapid resizing");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
