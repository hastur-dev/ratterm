//! E2E tests verifying all Command Palette UI hints are visible.
//!
//! Bottom hint line: ↑↓ Navigate │ Enter Select │ Esc Close
//!
//! Run with: `cargo build --release && cargo test --test expectrl_command_palette_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

// ============================================================================
// Command Palette opens and displays correctly
// ============================================================================

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_command_palette_opens_with_f1() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Command Palette", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "F1 should open Command Palette: {:?}",
        result
    );
}

// ============================================================================
// Bottom hint line: all keys and descriptions
// ============================================================================

#[test]
#[ignore]
fn test_command_palette_shows_navigate_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Navigate", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should show 'Navigate' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_command_palette_shows_enter_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Enter", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should show 'Enter' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_command_palette_shows_select_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Select", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should show 'Select' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_command_palette_shows_esc_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Esc", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should show 'Esc' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_command_palette_shows_close_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("Close", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should show 'Close' hint: {:?}",
        result
    );
}

// ============================================================================
// Command Palette interaction
// ============================================================================

#[test]
#[ignore]
fn test_command_palette_closes_with_esc() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f1();
    session.wait_render();

    // Verify palette is open
    let result = session.expect_text("Command Palette", Duration::from_secs(5));
    assert!(result.is_ok(), "Palette should be open: {:?}", result);

    // Send Escape to close
    let _ = session.send_escape();
    session.wait_render();

    // After closing, the help bar should be visible again with default hints
    let result = session.expect_text("Palette", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Palette' after closing popup: {:?}",
        result
    );
}
