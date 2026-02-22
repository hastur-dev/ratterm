//! E2E ConPTY tests for the SSH Health Dashboard.
//!
//! Verifies that the health dashboard remains responsive (navigation, close)
//! while SSH timeout errors are occurring in background collection threads.
//!
//! ## Testing Strategy
//!
//! These tests use **timing-based assertions** because ConPTY on Windows 10
//! does not pass alternate screen buffer content through the output pipe.
//! Since what we're testing is _responsiveness_ (not visual correctness),
//! we measure elapsed time for the full interaction cycle:
//!
//! - If the dashboard freezes due to blocking SSH timeouts (5s each × 5 hosts),
//!   the cycle takes 25-30+ seconds.
//! - With non-blocking fixes, the full cycle completes in < 5 seconds.
//!
//! ## What these tests verify
//!
//! 1. `DaemonManager::start()` is non-blocking (no 5-second busy-wait)
//! 2. SSH `cmd.output()` uses `spawn_with_timeout()` (threads don't hang forever)
//! 3. `DaemonManager::stop()` is non-blocking (no thread join blocking)
//! 4. Navigation keys (j/k/arrows) are processed immediately during collection
//! 5. Close keys (Escape/q) are processed immediately during collection
//!
//! Run with: `cargo build --release && cargo test --test expectrl_health_dashboard_tests -- --ignored`

#![allow(clippy::expect_used)]

mod helpers;

use helpers::tui_harness::TuiTestSession;
use std::time::{Duration, Instant};

/// Maximum allowed time for a dashboard open+interact+close+quit cycle.
/// Before the non-blocking fixes, this would take 25-30+ seconds.
/// After the fixes, it completes in 2-3 seconds.
const MAX_CYCLE_SECS: u64 = 8;

/// Maximum allowed time for a single dashboard open+close operation.
/// DaemonManager::start() alone used to block for 5 seconds.
const MAX_OPEN_CLOSE_SECS: u64 = 5;

// ============================================================================
// Dashboard opens without blocking
// ============================================================================

#[test]
#[ignore] // Requires `cargo build --release` first
fn test_health_dashboard_opens_without_blocking() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    // Open health dashboard — this triggers DaemonManager::start()
    // which previously blocked for up to 5 seconds with a busy-wait loop.
    let _ = session.send_f4();
    session.wait_render();

    // Close dashboard — triggers DaemonManager::stop()
    // which previously blocked on thread join.
    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();

    // Quit the app cleanly
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_OPEN_CLOSE_SECS,
        "Dashboard open+close took {}s — DaemonManager::start() or stop() is likely blocking \
         (threshold: {}s)",
        elapsed.as_secs(),
        MAX_OPEN_CLOSE_SECS
    );
}

// ============================================================================
// Navigation remains responsive during collection
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_j_key_responsive_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    // Open health dashboard — starts SSH collection threads
    let _ = session.send_f4();
    session.wait_render();

    // Navigate with j — must NOT freeze
    let _ = session.send_key('j');
    session.wait_render();

    // Close
    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "'j' navigation likely froze the dashboard ({}s, threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

#[test]
#[ignore]
fn test_dashboard_k_key_responsive_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    let _ = session.send_key('k');
    session.wait_render();

    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "'k' navigation likely froze the dashboard ({}s, threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

#[test]
#[ignore]
fn test_dashboard_arrow_keys_responsive_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    // Arrow down then up
    let _ = session.send_arrow_down();
    std::thread::sleep(Duration::from_millis(100));
    let _ = session.send_arrow_up();
    session.wait_render();

    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Arrow navigation likely froze the dashboard ({}s, threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

#[test]
#[ignore]
fn test_dashboard_rapid_navigation_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    // Rapid j/k navigation — verifies no accumulated freeze
    for _ in 0..5 {
        let _ = session.send_key('j');
        std::thread::sleep(Duration::from_millis(50));
    }
    for _ in 0..5 {
        let _ = session.send_key('k');
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Rapid navigation likely froze the dashboard ({}s, threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

// ============================================================================
// Close is responsive during collection
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_escape_closes_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f4();
    session.wait_render();

    // Press Escape — must NOT block while SSH threads are timing out
    let close_start = Instant::now();
    let _ = session.send_escape();
    session.wait_render();
    let close_elapsed = close_start.elapsed();

    let _ = session.quit();

    assert!(
        close_elapsed.as_secs() < 3,
        "Escape close took {}s — likely blocked by DaemonManager::stop() (threshold: 3s)",
        close_elapsed.as_secs()
    );
}

#[test]
#[ignore]
fn test_dashboard_q_closes_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let _ = session.send_f4();
    session.wait_render();

    // Press 'q' to close — must not freeze
    let close_start = Instant::now();
    let _ = session.send_key('q');
    session.wait_render();
    let close_elapsed = close_start.elapsed();

    let _ = session.quit();

    assert!(
        close_elapsed.as_secs() < 3,
        "'q' close took {}s — likely blocked by DaemonManager::stop() (threshold: 3s)",
        close_elapsed.as_secs()
    );
}

// ============================================================================
// Re-open after close verifies no stale blocking state
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_reopen_after_close_not_blocked() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    // First open
    let _ = session.send_f4();
    session.wait_render();

    // Close
    let _ = session.send_escape();
    session.wait_render();

    // Re-open — verifies DaemonManager::start() is non-blocking on second call
    let _ = session.send_f4();
    session.wait_render();

    // Close again
    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Re-open cycle took {}s — likely blocked by stale state (threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

// ============================================================================
// Navigation + close combined — stress test
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_navigate_then_close_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    // Navigate while collection is running
    let _ = session.send_key('j');
    std::thread::sleep(Duration::from_millis(100));
    let _ = session.send_key('j');
    std::thread::sleep(Duration::from_millis(100));
    let _ = session.send_key('k');
    std::thread::sleep(Duration::from_millis(100));

    // Then close — must not freeze
    let _ = session.send_key('q');
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Navigate-then-close took {}s — likely froze (threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

// ============================================================================
// Help overlay responsive during collection
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_help_overlay_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    // Press '?' to open help overlay
    let _ = session.send_key('?');
    session.wait_render();

    // Close help overlay
    let _ = session.send_escape();
    session.wait_render();

    // Close dashboard
    let _ = session.send_key('q');
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Help overlay interaction took {}s — likely froze (threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

// ============================================================================
// Refresh key responsive during collection
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_refresh_responsive_during_collection() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    let start = Instant::now();

    let _ = session.send_f4();
    session.wait_render();

    // Press 'r' to refresh — must not block
    let _ = session.send_key('r');
    session.wait_render();

    let _ = session.send_escape();
    session.wait_render();

    let elapsed = start.elapsed();
    let _ = session.quit();

    assert!(
        elapsed.as_secs() < MAX_CYCLE_SECS,
        "Refresh took {}s — likely froze (threshold: {}s)",
        elapsed.as_secs(),
        MAX_CYCLE_SECS
    );
}

// ============================================================================
// Process exits cleanly after dashboard interaction
// ============================================================================

#[test]
#[ignore]
fn test_dashboard_process_exits_cleanly() {
    let mut session = TuiTestSession::spawn_with_args(&["--test-keys"]).expect("Failed to spawn");
    session.wait_startup();

    // Full dashboard interaction cycle
    let _ = session.send_f4();
    session.wait_render();

    let _ = session.send_key('j');
    std::thread::sleep(Duration::from_millis(100));
    let _ = session.send_key('k');
    std::thread::sleep(Duration::from_millis(100));

    let _ = session.send_escape();
    session.wait_render();

    let _ = session.quit();

    // Give process extra time to fully exit
    std::thread::sleep(Duration::from_secs(1));

    assert!(
        !session.is_alive(),
        "Process should have exited after Ctrl+Q"
    );
}
