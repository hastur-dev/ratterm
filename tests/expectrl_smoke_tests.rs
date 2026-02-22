#![cfg(windows)]
//! Baseline expectrl smoke tests for the ratterm binary.
//!
//! These tests validate the test harness works and establish baseline behavior.
//! Run with: `cargo build --release && cargo test --test expectrl_smoke_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use std::time::Duration;

use helpers::tui_harness::TuiTestSession;

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_binary_version_flag() {
    let session = TuiTestSession::spawn_with_args(&["--version"])
        .expect("Failed to spawn binary with --version");

    let result = session.expect_text("ratterm v", Duration::from_secs(5));
    assert!(result.is_ok(), "Expected version output: {:?}", result);
}

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_binary_verify_flag() {
    let session = TuiTestSession::spawn_with_args(&["--verify"])
        .expect("Failed to spawn binary with --verify");

    let result = session.expect_text("verify-ok", Duration::from_secs(5));
    assert!(result.is_ok(), "Expected verify-ok output: {:?}", result);
}

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_binary_starts_and_shows_status_bar() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn binary");
    session.wait_startup();

    let result = session.expect_text("TERM", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "Expected TERM in status bar: {:?}", result);
}

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_ctrl_q_exits_cleanly() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn binary");
    session.wait_startup();

    let quit_result = session.quit();
    assert!(quit_result.is_ok(), "Ctrl+Q should exit cleanly");
}
