//! E2E tests verifying all SSH Manager hotkey hints are visible.
//!
//! Primary hints: Enter/Connect, a/Add Host, e/Edit, d/Delete, s/Scan Network
//! Secondary hints: c/Credential Scan, Shift+S/Scan Subnet, Ctrl+1-9/Quick Connect, Esc/Close
//! Credential entry hints: Tab/Next Field, Space/Toggle Save, Enter/Connect, Esc/Cancel
//!
//! Run with: `cargo build --release && cargo test --test expectrl_ssh_manager_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

// ============================================================================
// SSH Manager opens correctly
// ============================================================================

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_ssh_manager_opens_with_f2() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("SSH Manager", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "F2 should open SSH Manager: {:?}", result);
}

// ============================================================================
// Primary row hints (Enter, a, e, d, s)
// ============================================================================

#[test]
#[ignore]
fn test_ssh_manager_shows_enter_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Enter", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Enter' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_connect_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Connect", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Connect' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_add_host_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Add Host", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Add Host' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_edit_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Edit", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Edit' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_delete_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Delete", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Delete' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_scan_network_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Scan Network", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Scan Network' hint: {:?}",
        result
    );
}

// ============================================================================
// Secondary row hints (c, Shift+S, Ctrl+1-9, Esc)
// ============================================================================

#[test]
#[ignore]
fn test_ssh_manager_shows_credential_scan_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Credential Scan", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Credential Scan' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_scan_subnet_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Scan Subnet", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Scan Subnet' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_quick_connect_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Quick Connect", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Quick Connect' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_close_hint() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Close", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Close' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_esc_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Esc", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Esc' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_shift_s_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Shift+S", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Shift+S' key: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_ssh_manager_shows_ctrl_1_9_key() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f2();
    session.wait_render();

    let result = session.expect_text("Ctrl+1-9", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "SSH Manager should show 'Ctrl+1-9' key: {:?}",
        result
    );
}
