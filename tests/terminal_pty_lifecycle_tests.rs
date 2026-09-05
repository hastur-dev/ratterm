//! Regression tests for PTY teardown.
//!
//! Before these existed, `Pty` did not keep the spawned child, so `kill` and
//! `shutdown` did nothing, and dropping a PTY blocked the calling thread for
//! over three minutes on Windows while the pseudo-console waited for a shell
//! that nobody had terminated. Measured on this machine: `drop` took
//! 198.76 s, and `cargo test --all-targets` never finished.
//!
//! These tests spawn real shells, so they assert on bounds rather than exact
//! timings: a regression reintroduces a multi-minute stall, which any generous
//! bound catches.

#![allow(clippy::expect_used)]

use std::time::{Duration, Instant};

use ratterm::terminal::pty::{Pty, PtyConfig, PtyError};

/// Teardown must finish well inside this. The defect was ~200 s.
const TEARDOWN_BUDGET: Duration = Duration::from_secs(15);

#[test]
fn drop_returns_promptly() {
    let pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    let start = Instant::now();
    drop(pty);
    let elapsed = start.elapsed();
    assert!(
        elapsed < TEARDOWN_BUDGET,
        "dropping a PTY took {elapsed:?}, budget is {TEARDOWN_BUDGET:?}"
    );
}

#[test]
fn drop_returns_promptly_after_output() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");

    // Produce output nobody drains: this is the shape that used to stall.
    #[cfg(unix)]
    let cmd: &[u8] = b"seq 1 2000\n";
    #[cfg(windows)]
    let cmd: &[u8] = b"for /L %i in (1,1,2000) do @echo %i\r\n";
    pty.write(cmd).expect("write command");
    std::thread::sleep(Duration::from_millis(300));

    let start = Instant::now();
    drop(pty);
    let elapsed = start.elapsed();
    assert!(
        elapsed < TEARDOWN_BUDGET,
        "dropping a PTY with pending output took {elapsed:?}"
    );
}

#[test]
fn kill_terminates_the_shell() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    assert!(pty.is_running(), "a fresh PTY reports running");
    assert!(
        !pty.child_has_exited(),
        "a fresh PTY has a live shell process"
    );

    pty.kill().expect("kill");

    assert!(!pty.is_running(), "kill clears the running flag");
    assert!(pty.child_has_exited(), "kill terminates the shell process");
}

#[test]
fn shutdown_terminates_the_shell() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    pty.shutdown().expect("shutdown");
    assert!(pty.child_has_exited(), "shutdown terminates the shell");
}

#[test]
fn kill_is_idempotent() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    pty.kill().expect("first kill");
    pty.kill().expect("second kill");
    pty.shutdown().expect("shutdown after kill");
    assert!(pty.child_has_exited());
}

#[test]
fn writing_after_shutdown_reports_closed() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    pty.shutdown().expect("shutdown");

    match pty.write(b"echo late\n") {
        Err(PtyError::Closed) => {}
        other => panic!("expected PtyError::Closed, got {other:?}"),
    }
}

#[test]
fn reading_after_shutdown_reports_closed() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    pty.shutdown().expect("shutdown");

    match pty.read() {
        Err(PtyError::Closed) => {}
        other => panic!(
            "expected PtyError::Closed, got {:?}",
            other.map(|v| v.len())
        ),
    }
}

#[test]
fn resizing_after_shutdown_reports_closed() {
    let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
    pty.shutdown().expect("shutdown");

    match pty.resize(100, 40) {
        Err(PtyError::Closed) => {}
        other => panic!("expected PtyError::Closed, got {other:?}"),
    }
}

#[test]
fn opening_and_closing_many_ptys_stays_bounded() {
    // Eight is enough to expose an O(minutes) teardown while staying quick
    // when teardown is correct.
    const ROUNDS: usize = 8;

    let start = Instant::now();
    for _ in 0..ROUNDS {
        let mut pty = Pty::new(PtyConfig::default()).expect("spawn pty");
        let _ = pty.write(b"\n");
        pty.shutdown().expect("shutdown");
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < TEARDOWN_BUDGET,
        "{ROUNDS} PTY lifecycles took {elapsed:?}"
    );
}
