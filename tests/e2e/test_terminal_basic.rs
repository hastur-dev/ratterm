//! E2E tests: Basic terminal input — type commands, see output.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;

/// Test: Type `echo hello` in the terminal and see "hello" in output.
#[test]
#[ignore = "Requires PTY"]
fn test_terminal_echo_command() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(3000); // Wait for shell prompt

    h.send_text("echo RATTERM_TEST_OK").expect("type cmd");
    h.send_text(keys::ENTER).expect("send enter");
    h.wait_ms(2000);

    // Use tolerant matching for ConPTY character doubling on Windows
    h.expect_text_tolerant("RATTERM_TEST_OK")
        .expect("Should see echo output");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Type `cd` and see the current directory in the output (Windows).
#[test]
#[ignore = "Requires PTY"]
fn test_terminal_cd_command() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(3000);

    h.send_text("cd").expect("type cd");
    h.send_text(keys::ENTER).expect("enter");
    h.wait_ms(2000);

    // `cd` on Windows prints the current directory. The initial prompt
    // already contains "C:\" so this text appears in the undoubled output.
    h.expect_text("C:\\").expect("Should see a path");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Test: Send Ctrl+C to interrupt a running command.
#[test]
#[ignore = "Requires PTY"]
fn test_terminal_ctrl_c_interrupt() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.wait_ms(3000);

    // Start a long-running command (Windows: ping with high count)
    h.send_text("ping -n 9999 127.0.0.1").expect("type cmd");
    h.send_text(keys::ENTER).expect("enter");
    h.wait_ms(2000);

    // Interrupt it
    h.send_text(keys::CTRL_C).expect("send Ctrl+C");
    h.wait_ms(2000);

    // Should be back at a prompt — verify by sending another command
    h.send_text("echo AFTER_INTERRUPT").expect("type cmd");
    h.send_text(keys::ENTER).expect("enter");
    h.wait_ms(2000);
    h.expect_text_tolerant("AFTER_INTERRUPT")
        .expect("Should see output after interrupt");

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
