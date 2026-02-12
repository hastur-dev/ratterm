//! Custom assertion helpers for TUI testing.
//!
//! On Windows, text rendered through the TUI via ConPTY may have each
//! character doubled (e.g., "HELLO" becomes "HHEELLLLOO") due to
//! incremental ratatui re-renders. The helpers here account for this.

use super::harness::RattermHarness;

/// Build a regex pattern that matches text even if every character is doubled.
///
/// For input "HELLO", produces "H+E+L+L+O+" which matches both "HELLO"
/// and "HHEELLLLOO" and any mix.
pub fn doubled_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() * 3);
    for ch in text.chars() {
        // Escape regex-special characters
        if "\\^$.|?*+()[]{}".contains(ch) {
            pattern.push('\\');
        }
        pattern.push(ch);
        pattern.push('+');
    }
    pattern
}

/// Assert that the harness can find the given text within the timeout.
/// Accounts for ConPTY character doubling on Windows.
///
/// # Panics
///
/// Panics with a descriptive message on failure.
pub fn assert_screen_contains(harness: &mut RattermHarness, text: &str, context: &str) {
    let pattern = doubled_pattern(text);
    harness.expect_regex(&pattern).unwrap_or_else(|e| {
        panic!(
            "Expected screen to contain '{}' (pattern: '{}') ({}): {:?}",
            text, pattern, context, e
        )
    });
}

/// Assert a regex pattern appears on screen.
///
/// # Panics
///
/// Panics with a descriptive message on failure.
pub fn assert_screen_matches(harness: &mut RattermHarness, pattern: &str, context: &str) {
    harness.expect_regex(pattern).unwrap_or_else(|e| {
        panic!(
            "Expected screen to match '{}' ({}): {:?}",
            pattern, context, e
        )
    });
}
