#![cfg(windows)]
//! E2E tests verifying the status bar renders correctly.
//!
//! Run with: `cargo build --release && cargo test --test expectrl_status_bar_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_status_bar_shows_term_mode() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("TERM", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Status bar should show 'TERM': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_status_bar_shows_vim_mode() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("VIM", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "Status bar should show 'VIM': {:?}", result);
}

#[test]
#[ignore]
fn test_status_bar_has_separator() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    // The separator character │ (U+2502) should appear in the status bar
    let result = session.expect_text("\u{2502}", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Status bar should have separator: {:?}",
        result
    );
}
