//! Debug test to understand the expectrl output from ratterm.

#[path = "common/mod.rs"]
mod common;

use common::harness::RattermHarness;
use common::keys;
use expectrl::Expect;

/// Debug: check if expect can find the initial cmd.exe banner text.
#[test]
#[ignore = "Debug test"]
fn debug_expect_initial_text() {
    let mut h = RattermHarness::spawn().expect("spawn");

    // The initial cmd.exe banner includes "Microsoft Windows" which is
    // NOT doubled (bulk rendered). Try to expect this.
    h.set_timeout(15_000);
    match h.expect_text("Microsoft") {
        Ok(()) => eprintln!("FOUND 'Microsoft' in stream"),
        Err(e) => eprintln!("NOT FOUND 'Microsoft': {:?}", e),
    }

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Debug: check if expect can find echoed command output.
#[test]
#[ignore = "Debug test"]
fn debug_expect_echo_output() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.set_timeout(15_000);

    // First consume initial render by waiting for the prompt
    match h.expect_text("coding>") {
        Ok(()) => eprintln!("FOUND initial prompt"),
        Err(e) => eprintln!("NOT FOUND initial prompt: {:?}", e),
    }

    // Now send a command
    h.send_text("echo XYZ123").expect("type");
    h.send_text(keys::ENTER).expect("enter");

    // Try to find the doubled output
    match h.expect_regex("X+Y+Z+1+2+3+") {
        Ok(()) => eprintln!("FOUND doubled pattern"),
        Err(e) => eprintln!("NOT FOUND doubled pattern: {:?}", e),
    }

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}

/// Debug: raw read from session after command.
#[test]
#[ignore = "Debug test"]
fn debug_raw_read() {
    let mut h = RattermHarness::spawn().expect("spawn");
    h.set_timeout(15_000);

    // Use expect to consume initial output and find the prompt
    eprintln!("Waiting for initial prompt...");
    match h.session.expect("coding>") {
        Ok(captures) => {
            eprintln!("Got initial prompt. Captures before: {} bytes", captures.before().len());
        }
        Err(e) => {
            eprintln!("Failed to find prompt: {:?}", e);
            h.send_text(keys::CTRL_Q).expect("quit");
            return;
        }
    }

    // Send a command
    h.send_text("echo ABCDEF").expect("type");
    h.send_text(keys::ENTER).expect("enter");

    // Now try to read directly
    eprintln!("Waiting 3 seconds for output...");
    h.wait_ms(3000);

    // Try reading with check instead of expect
    match h.session.expect(expectrl::Regex("A+B+C+D+E+F+")) {
        Ok(captures) => {
            let before = String::from_utf8_lossy(captures.before());
            eprintln!("MATCH! Before match: {} bytes, text: {}",
                captures.before().len(),
                &before[before.len().saturating_sub(200)..]);
        }
        Err(e) => {
            eprintln!("No match: {:?}", e);
        }
    }

    h.send_text(keys::CTRL_Q).expect("quit");
    h.wait_ms(300);
}
