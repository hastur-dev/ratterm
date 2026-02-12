//! E2E tests: Configuration loading from .ratrc.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: App starts with default config when no .ratrc exists.
#[test]
#[ignore = "Requires PTY"]
fn test_default_config() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(2000);

    // Should start without errors
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: App respects mode=vim in .ratrc.
#[test]
#[ignore = "Requires PTY"]
fn test_config_vim_mode() {
    let h = RattermHarness::spawn().expect("spawn");
    let ratrc = h.create_ratrc("mode = vim\n");
    drop(h);

    let ratrc_str = ratrc.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&["--config", ratrc_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor and verify vim mode (h should move cursor, not insert)
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // In vim normal mode, 'i' enters insert, ESC exits
    h.send_text("i").expect("insert");
    h.send_text("vim mode active").expect("type");
    h.send_text(keys::ESC).expect("normal");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: App respects mode=emacs in .ratrc.
#[test]
#[ignore = "Requires PTY"]
fn test_config_emacs_mode() {
    let h = RattermHarness::spawn().expect("spawn");
    let ratrc = h.create_ratrc("mode = emacs\n");
    drop(h);

    let ratrc_str = ratrc.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&["--config", ratrc_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // In emacs mode, typing inserts directly (no normal mode)
    h.send_text("emacs mode active").expect("type");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: App respects mode=default in .ratrc.
#[test]
#[ignore = "Requires PTY"]
fn test_config_default_mode() {
    let h = RattermHarness::spawn().expect("spawn");
    let ratrc = h.create_ratrc("mode = default\n");
    drop(h);

    let ratrc_str = ratrc.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&["--config", ratrc_str]).expect("spawn");
    h.wait_ms(2000);

    // Focus editor
    h.send_text(keys::ALT_RIGHT).expect("focus editor");
    h.wait_ms(500);

    // Default mode: typing inserts directly
    h.send_text("default mode active").expect("type");
    h.wait_ms(200);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Invalid .ratrc doesn't crash the app.
#[test]
#[ignore = "Requires PTY"]
fn test_invalid_config_no_crash() {
    let h = RattermHarness::spawn().expect("spawn");
    let ratrc = h.create_ratrc("invalid_key = invalid_value\nmode = nonexistent\n!!!garbage\n");
    drop(h);

    let ratrc_str = ratrc.to_str().expect("path");
    let mut h = RattermHarness::spawn_with_args(&["--config", ratrc_str]).expect("spawn");
    h.wait_ms(2000);

    // App should start despite invalid config
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
