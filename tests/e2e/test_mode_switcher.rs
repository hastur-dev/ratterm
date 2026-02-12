//! E2E tests: Input mode switcher (Ctrl+Shift+Tab to cycle Vim/Emacs/Default).

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Ctrl+Shift+Tab opens the mode switcher popup.
#[test]
#[ignore = "Requires PTY"]
fn test_mode_switcher_opens() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open mode switcher
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);

    // Close with ESC
    h.send_text(keys::ESC).expect("close");
    h.wait_ms(300);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Switch from Vim to Emacs mode.
#[test]
#[ignore = "Requires PTY"]
fn test_switch_to_emacs() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open mode switcher
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);

    // Navigate to Emacs (second option)
    h.send_text(keys::DOWN).expect("navigate");
    h.wait_ms(100);
    h.send_text(keys::ENTER).expect("select emacs");
    h.wait_ms(500);

    // Should now be in Emacs mode — Ctrl+A should go to line start
    h.send_text("test text").expect("type");
    h.wait_ms(200);
    h.send_text(keys::CTRL_A).expect("Ctrl+A start");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Switch from Vim to Default mode.
#[test]
#[ignore = "Requires PTY"]
fn test_switch_to_default() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Open mode switcher
    h.send_text(keys::CTRL_SHIFT_TAB).expect("mode switcher");
    h.wait_ms(500);

    // Navigate to Default (third option)
    h.send_text(keys::DOWN).expect("past emacs");
    h.wait_ms(100);
    h.send_text(keys::DOWN).expect("to default");
    h.wait_ms(100);
    h.send_text(keys::ENTER).expect("select default");
    h.wait_ms(500);

    // Should now be in Default mode — typing inserts directly
    h.send_text("direct typing works").expect("type");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Cycle through all modes and back to Vim.
#[test]
#[ignore = "Requires PTY"]
fn test_cycle_all_modes() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Switch to Emacs
    h.send_text(keys::CTRL_SHIFT_TAB).expect("switcher");
    h.wait_ms(400);
    h.send_text(keys::DOWN).expect("emacs");
    h.send_text(keys::ENTER).expect("select");
    h.wait_ms(400);

    // Switch to Default
    h.send_text(keys::CTRL_SHIFT_TAB).expect("switcher");
    h.wait_ms(400);
    h.send_text(keys::DOWN).expect("skip");
    h.send_text(keys::DOWN).expect("default");
    h.send_text(keys::ENTER).expect("select");
    h.wait_ms(400);

    // Switch back to Vim
    h.send_text(keys::CTRL_SHIFT_TAB).expect("switcher");
    h.wait_ms(400);
    h.send_text(keys::ENTER).expect("select vim (first)");
    h.wait_ms(400);

    // Verify Vim works (h should move, not insert)
    h.send_text("i").expect("enter insert");
    h.send_text("vim is back").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
