#![cfg(windows)]
//! E2E tests verifying all help bar hotkey hints appear on screen.
//!
//! Terminal focused hints: Palette, SSH, Docker, New Tab, Split, Switch Pane, Quit
//! Editor focused hints: Palette, Open, Save, Find, Switch Pane, Quit
//!
//! Run with: `cargo build --release && cargo test --test expectrl_help_bar_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

// ============================================================================
// Terminal-focused help bar hints
// ============================================================================

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_help_bar_shows_palette_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Palette", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Palette': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_ctrl_shift_p_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Ctrl+Shift+P", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Ctrl+Shift+P': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_ssh_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("SSH", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "Help bar should show 'SSH': {:?}", result);
}

#[test]
#[ignore]
fn test_help_bar_shows_ctrl_shift_u_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Ctrl+Shift+U", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Ctrl+Shift+U': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_docker_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Docker", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Docker': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_ctrl_shift_d_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Ctrl+Shift+D", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Ctrl+Shift+D': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_new_tab_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("New Tab", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'New Tab': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_ctrl_t_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Ctrl+T", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Ctrl+T': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_split_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Split", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "Help bar should show 'Split': {:?}", result);
}

#[test]
#[ignore]
fn test_help_bar_shows_switch_pane_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Switch Pane", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Switch Pane': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_alt_tab_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Alt+Tab", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Alt+Tab': {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_help_bar_shows_quit_hint() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Quit", Duration::from_secs(5));
    let _ = session.quit();
    assert!(result.is_ok(), "Help bar should show 'Quit': {:?}", result);
}

#[test]
#[ignore]
fn test_help_bar_shows_ctrl_q_key() {
    let mut session = TuiTestSession::spawn().expect("Failed to spawn");
    session.wait_startup();

    let result = session.expect_text("Ctrl+Q", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help bar should show 'Ctrl+Q': {:?}",
        result
    );
}

// ============================================================================
// Note: Editor-focused help bar hints (Open, Save, Find) would require
// switching focus to the editor pane first. These are covered by unit tests
// in src/app/render.rs which verify the hint vectors for both modes.
// ============================================================================
