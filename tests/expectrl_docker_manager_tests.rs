#![cfg(windows)]
//! E2E tests verifying all Docker Manager hotkey hints are visible.
//!
//! Primary hints: Enter/Attach, s/Start, S/Stop, r/Restart, n/New Container, R/Refresh
//! Secondary hints: Tab/Switch Section, Ctrl+Alt+1-9/Quick Connect, Esc/Close
//!
//! Run with: `cargo build --release && cargo test --test expectrl_docker_manager_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

// ============================================================================
// Docker Manager opens correctly
// ============================================================================

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_docker_manager_opens_with_f3() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Docker", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "F3 should open Docker Manager: {:?}",
        result
    );
}

// ============================================================================
// Primary row hints (Enter, s, S, r, n, R)
// ============================================================================

#[test]
#[ignore]
fn test_docker_manager_shows_enter_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Enter", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Enter' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_attach_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Attach", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Attach' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_start_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Start", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Start' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_stop_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Stop", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Stop' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_restart_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Restart", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Restart' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_new_container_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("New Container", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'New Container' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_refresh_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Refresh", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Refresh' hint: {:?}",
        result
    );
}

// ============================================================================
// Secondary row hints (Tab, Ctrl+Alt+1-9, Esc)
// ============================================================================

#[test]
#[ignore]
fn test_docker_manager_shows_switch_section_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Switch Section", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Switch Section' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_tab_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Tab", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Tab' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_quick_connect_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Quick Connect", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Quick Connect' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_ctrl_alt_1_9_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Ctrl+Alt+1-9", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Ctrl+Alt+1-9' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_close_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Close", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Close' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_manager_shows_esc_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Esc", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager should show 'Esc' key: {:?}",
        result
    );
}
