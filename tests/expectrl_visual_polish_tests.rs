//! E2E tests verifying visual consistency across popups.
//!
//! Run with: `cargo build --release && cargo test --test expectrl_visual_polish_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_all_popups_have_rounded_borders() {
    // This test verifies that the rounded border character ╭ (U+256D)
    // appears when any popup is opened, indicating rounded borders.
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    // Open Command Palette
    let _ = session.send_f1();
    session.wait_render();

    let result = session.expect_text("\u{256D}", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Command Palette should have rounded border (\u{256D}): {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_has_rounded_borders() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("\u{256D}", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should have rounded border (\u{256D}): {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_has_rounded_borders() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("\u{256D}", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should have rounded border (\u{256D}): {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_and_status_bar_both_visible() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    // Help bar should show keybinding hints
    let help_result = session.expect_text("Palette", Duration::from_secs(5));

    // Status bar should show mode info
    let status_result = session.expect_text("TERM", Duration::from_secs(5));

    let _ = session.quit();

    assert!(
        help_result.is_ok(),
        "Help bar should be visible with 'Palette': {:?}",
        help_result
    );
    assert!(
        status_result.is_ok(),
        "Status bar should be visible with 'TERM': {:?}",
        status_result
    );
}
