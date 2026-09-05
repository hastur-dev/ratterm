//! Runs the checked-in scenarios as part of `cargo test`.
//!
//! The same files run from the command line with
//! `rat --scenario-dir tests/scenarios --fixtures tests/fixtures/fleet`, and
//! from CI on Windows, Linux and macOS. Running them here as well means a
//! change that breaks the interface fails the ordinary test command, not only
//! a separate job somebody has to remember to look at.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use ratterm::app::{App, AppOptions};
use ratterm::scenario::{Scenario, ScenarioRunner, load_dir};

/// Where the scenarios live, relative to the crate root.
const SCENARIO_DIR: &str = "tests/scenarios";
/// Fixtures that make the fleet the same on every machine.
const FIXTURE_DIR: &str = "tests/fixtures/fleet";

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Builds an instance in the same shape the command-line runner uses.
fn make_app() -> App {
    let options = AppOptions::interactive().with_fixtures(crate_root().join(FIXTURE_DIR));
    let mut options = options;
    // No shell and no IPC endpoint: several tests run at once, and the
    // endpoint is a single system-wide name.
    options.without_terminals = true;
    options.without_api = true;

    App::with_options(120, 40, options).expect("create an application")
}

fn scenarios() -> Vec<Scenario> {
    load_dir(&crate_root().join(SCENARIO_DIR)).expect("load the scenario directory")
}

fn results_dir() -> PathBuf {
    crate_root().join("test-results")
}

#[test]
fn the_scenario_directory_is_not_empty() {
    let scenarios = scenarios();
    assert!(
        scenarios.len() >= 10,
        "expected at least ten scenarios, found {}",
        scenarios.len()
    );
}

#[test]
fn every_scenario_has_at_least_one_assertion() {
    for scenario in scenarios() {
        assert!(
            scenario
                .steps
                .iter()
                .any(ratterm::scenario::Step::is_assertion),
            "{} has no assertions, so it can only fail on an error",
            scenario.name
        );
    }
}

#[test]
fn every_scenario_passes() {
    let runner = ScenarioRunner::new().with_results_dir(results_dir());
    let mut failures = Vec::new();

    for scenario in scenarios() {
        let mut app = make_app();
        let outcome = runner.run(&scenario, &mut app);
        if !outcome.passed {
            failures.push(outcome.report());
        }
        app.shutdown();
    }

    assert!(
        failures.is_empty(),
        "{} scenario(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn scenarios_write_their_snapshots() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = ScenarioRunner::new().with_results_dir(dir.path());

    let scenario = scenarios()
        .into_iter()
        .find(|s| {
            s.steps
                .iter()
                .any(|step| matches!(step, ratterm::scenario::Step::Snapshot(_)))
        })
        .expect("at least one scenario takes a snapshot");

    let mut app = make_app();
    let outcome = runner.run(&scenario, &mut app);
    app.shutdown();

    assert!(outcome.passed, "{}", outcome.report());
    assert!(!outcome.snapshots.is_empty());
    for path in &outcome.snapshots {
        assert!(path.exists(), "{} was not written", path.display());
        assert!(
            path.with_extension("json").exists(),
            "the machine-readable snapshot is missing next to {}",
            path.display()
        );
    }
}

#[test]
fn the_fixture_fleet_replaces_the_users_real_hosts() {
    // The guarantee that makes these runs safe: fixture state, and no way to
    // reach a real machine.
    let app = make_app();
    assert!(app.is_fixture_mode());
    assert_eq!(app.host_registry().len(), 4);

    let names: Vec<String> = app
        .host_registry()
        .hosts()
        .filter_map(|h| h.display_name.clone())
        .collect();
    assert!(
        names.iter().all(|n| n.starts_with("fixture-")),
        "a fixture run must not show real hosts: {names:?}"
    );

    let reachable = ratterm::remote::with_shared(|executor| executor.known_hosts());
    assert_eq!(reachable, 0, "a fixture run must not be able to connect");
}

#[test]
fn the_scenario_directory_is_where_the_documentation_says() {
    let dir = crate_root().join(SCENARIO_DIR);
    assert!(dir.is_dir(), "{} is missing", dir.display());
    assert!(
        Path::new(&crate_root().join(FIXTURE_DIR)).is_dir(),
        "the fixture directory is missing"
    );
}
