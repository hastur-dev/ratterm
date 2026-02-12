//! E2E tests: Terminal scrollback history.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Generate output, scroll up with Shift+PageUp, scroll down with Shift+PageDown.
#[test]
#[ignore = "Requires PTY"]
fn test_scrollback_history() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Generate lots of output to push things off screen (Windows-compatible)
    h.send_line("for /L %i in (1,1,100) do @echo LINE_%i")
        .expect("gen output");
    h.wait_ms(5000);

    // Scroll up
    h.send_text(keys::SHIFT_PAGE_UP)
        .expect("Shift+PageUp");
    h.wait_ms(500);

    // Scroll up more
    h.send_text(keys::SHIFT_PAGE_UP)
        .expect("Shift+PageUp again");
    h.wait_ms(500);

    // Scroll back down
    h.send_text(keys::SHIFT_PAGE_DOWN)
        .expect("Shift+PageDown");
    h.wait_ms(500);

    h.send_text(keys::SHIFT_PAGE_DOWN)
        .expect("Shift+PageDown again");
    h.wait_ms(500);

    // Verify we're back at the bottom by typing
    h.send_line("echo SCROLLBACK_DONE").expect("echo");
    h.wait_ms(1000);
    h.expect_text("SCROLLBACK_DONE")
        .expect("Should be at bottom after scrolling");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
