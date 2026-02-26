#![cfg(windows)]
//! E2E tests for the Docker Logs viewer sub-mode.
//!
//! Tests the full flow: F3 to open Docker Manager → 'l' to open Docker Logs →
//! verify UI elements (container list, footer hints, navigation, mode transitions).
//!
//! Run with: `cargo build --release && cargo test --test expectrl_docker_logs_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::Duration;

// ============================================================================
// Helper: open Docker Logs from Docker Manager
// ============================================================================

/// Opens Docker Manager (F3) then presses 'l' to enter Docker Logs mode.
fn open_docker_logs() -> TuiTestSession {
    let mut session =
        TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let _ = session.send_key('l');
    session.wait_render();

    session
}

// ============================================================================
// Docker Logs opens correctly from Docker Manager
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_opens_with_l_key() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Pressing 'l' in Docker Manager should open Docker Logs: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_shows_container_list_header() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Select a container", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Logs should show container selection header: {:?}",
        result
    );
}

// ============================================================================
// Container list footer hints
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_enter_hint() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Enter", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show 'Enter' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_stream_hint() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Stream", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show 'Stream' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_esc_hint() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Esc", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show 'Esc' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_back_hint() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Back", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show 'Back' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_help_hint() {
    let mut session = open_docker_logs();

    let result = session.expect_text("Help", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show 'Help' hint: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_container_list_shows_question_mark_key() {
    let mut session = open_docker_logs();

    let result = session.expect_text("[?]", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Container list should show '[?]' key for help: {:?}",
        result
    );
}

// ============================================================================
// Navigation: Escape goes back to Docker Manager
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_escape_returns_to_docker_manager() {
    let mut session = open_docker_logs();

    // Verify we're in Docker Logs
    let in_logs = session.expect_text("Docker Logs", Duration::from_secs(5));
    assert!(in_logs.is_ok(), "Should be in Docker Logs first");

    // Press Escape to go back
    let _ = session.send_escape();
    session.wait_render();

    // Should be back in Docker Manager list mode showing section tabs
    let result = session.expect_text("Docker Manager", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Escape should return to Docker Manager: {:?}",
        result
    );
}

// ============================================================================
// Navigation: 'q' key also closes Docker Logs
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_q_returns_to_docker_manager() {
    let mut session = open_docker_logs();

    let in_logs = session.expect_text("Docker Logs", Duration::from_secs(5));
    assert!(in_logs.is_ok(), "Should be in Docker Logs first");

    // Press 'q' to go back
    let _ = session.send_key('q');
    session.wait_render();

    let result = session.expect_text("Docker Manager", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "'q' should return to Docker Manager: {:?}",
        result
    );
}

// ============================================================================
// Help overlay toggling
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_help_overlay_opens_with_question_mark() {
    let mut session = open_docker_logs();

    // Press '?' to open help overlay
    let _ = session.send_key('?');
    session.wait_render();

    // Help overlay should show hotkey descriptions
    let result = session.expect_text("Scroll logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "'?' should open help overlay with hotkey descriptions: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_help_overlay_shows_navigation_category() {
    let mut session = open_docker_logs();

    let _ = session.send_key('?');
    session.wait_render();

    let result = session.expect_text("Navigation", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help overlay should show 'Navigation' category: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_help_overlay_closes_on_second_press() {
    let mut session = open_docker_logs();

    // Open help
    let _ = session.send_key('?');
    session.wait_render();

    let open = session.expect_text("Scroll logs", Duration::from_secs(5));
    assert!(open.is_ok(), "Help should be open");

    // Close help by pressing '?' again
    let _ = session.send_key('?');
    session.wait_render();

    // After closing, should still see Docker Logs container list
    let result = session.expect_text("Select a container", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Help overlay should close and show container list: {:?}",
        result
    );
}

// ============================================================================
// Docker Manager [l] Logs footer hint is visible
// ============================================================================

#[test]
#[ignore]
fn test_docker_manager_shows_logs_hint() {
    let mut session =
        TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f3();
    session.wait_render();

    let result = session.expect_text("Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Docker Manager footer should show 'Logs' hint: {:?}",
        result
    );
}

// ============================================================================
// Roundtrip: Docker Manager → Docker Logs → Docker Manager → quit
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_full_roundtrip() {
    let mut session =
        TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    // Open Docker Manager
    let _ = session.send_f3();
    session.wait_render();
    let step1 = session.expect_text("Docker Manager", Duration::from_secs(5));
    assert!(step1.is_ok(), "Step 1: Docker Manager should open");

    // Open Docker Logs
    let _ = session.send_key('l');
    session.wait_render();
    let step2 = session.expect_text("Docker Logs", Duration::from_secs(5));
    assert!(step2.is_ok(), "Step 2: Docker Logs should open");

    // Go back to Docker Manager
    let _ = session.send_escape();
    session.wait_render();
    let step3 = session.expect_text("Docker Manager", Duration::from_secs(5));
    assert!(step3.is_ok(), "Step 3: Should return to Docker Manager");

    // Close Docker Manager
    let _ = session.send_escape();
    session.wait_render();

    // Quit
    let _ = session.quit();
    assert!(
        !session.is_alive(),
        "App should exit after quit"
    );
}

// ============================================================================
// Docker Logs stays open after navigation keys (no crash)
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_arrow_keys_no_crash() {
    let mut session = open_docker_logs();

    // Send arrow keys to navigate the empty container list
    let _ = session.send_arrow_down();
    session.wait_render();
    let _ = session.send_arrow_up();
    session.wait_render();
    let _ = session.send_arrow_down();
    session.wait_render();

    // Should still be in Docker Logs without crash
    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Arrow key navigation should not crash: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_j_k_navigation_no_crash() {
    let mut session = open_docker_logs();

    // Send vim-style navigation keys
    let _ = session.send_key('j');
    session.wait_render();
    let _ = session.send_key('k');
    session.wait_render();
    let _ = session.send_key('j');
    session.wait_render();

    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "j/k navigation should not crash: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_home_end_keys_no_crash() {
    let mut session = open_docker_logs();

    let _ = session.send_home();
    session.wait_render();
    let _ = session.send_end();
    session.wait_render();

    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Home/End keys should not crash: {:?}",
        result
    );
}

#[test]
#[ignore]
fn test_docker_logs_g_and_shift_g_no_crash() {
    let mut session = open_docker_logs();

    let _ = session.send_key('g');
    session.wait_render();
    let _ = session.send_key('G');
    session.wait_render();

    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "g/G navigation should not crash: {:?}",
        result
    );
}

// ============================================================================
// Multiple open/close cycles without crash
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_multiple_open_close_cycles() {
    let mut session =
        TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    for _ in 0..3 {
        // Open Docker Manager
        let _ = session.send_f3();
        session.wait_render();

        // Open Docker Logs
        let _ = session.send_key('l');
        session.wait_render();

        // Close Docker Logs
        let _ = session.send_escape();
        session.wait_render();

        // Close Docker Manager
        let _ = session.send_escape();
        session.wait_render();
    }

    // App should still be alive and responsive
    assert!(session.is_alive(), "App should still be alive after 3 open/close cycles");

    let _ = session.quit();
}

// ============================================================================
// Enter key on empty container list doesn't crash
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_enter_on_empty_list_no_crash() {
    let mut session = open_docker_logs();

    // Press Enter on potentially empty container list
    let _ = session.send_enter();
    session.wait_render();

    // Should not crash — still in Docker Logs
    let result = session.expect_text("Docker Logs", Duration::from_secs(5));
    let _ = session.quit();
    assert!(
        result.is_ok(),
        "Enter on empty list should not crash: {:?}",
        result
    );
}

// ============================================================================
// App quits cleanly from Docker Logs view
// ============================================================================

#[test]
#[ignore]
fn test_docker_logs_ctrl_q_quits_from_logs() {
    let mut session = open_docker_logs();

    let _ = session.quit();

    // Give it a moment
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !session.is_alive(),
        "Ctrl+Q should quit the app from Docker Logs view"
    );
}
