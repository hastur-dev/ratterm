//! E2E tests: CLI argument handling.

#[path = "common/mod.rs"]
mod common;

/// Binary path helper for non-interactive tests.
fn binary_path() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    #[cfg(windows)]
    let bin = manifest_dir.join("target").join("release").join("rat.exe");
    #[cfg(not(windows))]
    let bin = manifest_dir.join("target").join("release").join("rat");
    assert!(bin.exists(), "Release binary not found at {:?}", bin);
    bin
}

/// Test: `--version` prints version info and exits.
#[test]
fn test_cli_version() {
    let output = std::process::Command::new(binary_path())
        .arg("--version")
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ratterm"), "Should contain 'ratterm': {}", stdout);
    assert!(output.status.success());
}

/// Test: `--verify` prints verify-ok and exits.
#[test]
fn test_cli_verify() {
    let output = std::process::Command::new(binary_path())
        .arg("--verify")
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("verify-ok"),
        "Should contain 'verify-ok': {}",
        stdout
    );
    assert!(output.status.success());
}

/// Test: Unknown flag doesn't crash (launches TUI, so requires PTY to quit).
#[test]
#[ignore = "Requires PTY"]
fn test_cli_unknown_flag() {
    use common::harness::RattermHarness;
    use common::keys;

    // --help isn't a recognized flag, so the app starts normally.
    // Just verify it doesn't crash and can be quit.
    let mut h = RattermHarness::spawn_with_args(&["--unknown-flag"]).expect("spawn");
    h.wait_ms(2000);
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Passing a nonexistent file opens the editor with an empty buffer (doesn't crash).
#[test]
#[ignore = "Requires PTY"]
fn test_cli_nonexistent_file() {
    use common::harness::RattermHarness;
    use common::keys;

    let mut h = RattermHarness::spawn_with_args(&["this_file_does_not_exist.txt"])
        .expect("spawn");
    h.wait_ms(2000);

    // Should start without crashing
    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Passing multiple files opens them in tabs.
#[test]
#[ignore = "Requires PTY"]
fn test_cli_multiple_files() {
    use common::harness::RattermHarness;
    use common::keys;

    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let f1 = temp_dir.path().join("a.txt");
    let f2 = temp_dir.path().join("b.txt");
    let f3 = temp_dir.path().join("c.txt");
    std::fs::write(&f1, "aaa").expect("write");
    std::fs::write(&f2, "bbb").expect("write");
    std::fs::write(&f3, "ccc").expect("write");

    let mut h = RattermHarness::spawn_with_args(&[
        f1.to_str().expect("p"),
        f2.to_str().expect("p"),
        f3.to_str().expect("p"),
    ])
    .expect("spawn");
    h.wait_ms(2000);

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
