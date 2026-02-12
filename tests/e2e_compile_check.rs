//! Minimal test to verify the e2e common modules compile correctly.

mod e2e;

use e2e::common::harness::RattermHarness;
use e2e::common::keys;

#[test]
fn test_harness_module_compiles() {
    // Verify types and constants are accessible.
    let _ = keys::CTRL_Q;
    let _ = keys::ALT_LEFT;
    let _ = keys::key_sequence(&[keys::CTRL_S, keys::ENTER]);

    // Confirm RattermHarness has the expected API by referencing it.
    fn _check_api_exists() {
        // These closures are never called — they just verify the API compiles.
        let _spawn = || RattermHarness::spawn();
        let _spawn_file = || RattermHarness::spawn_with_file("test.rs");
        let _spawn_args = || RattermHarness::spawn_with_args(&["--version"]);
    }
}
