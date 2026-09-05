//! Scripted runs against the interface.
//!
//! A scenario is a list of steps — press a key, type text, assert some text is
//! on screen, save a snapshot — that an agent or CI can run without a
//! terminal. The same file runs unchanged on Windows, Linux and macOS, which
//! is the cross-platform check this project needs and could not previously
//! make: the existing harness spawned a real PTY and was compiled only on
//! Windows.
//!
//! Steps run against an [`App`](crate::app::App) in this process rather than
//! over IPC. That keeps a failure's stack in the same binary as the code under
//! test, and it means a scenario needs no port, socket or token.

pub mod runner;
pub mod step;

pub use runner::{ScenarioOutcome, ScenarioRunner, StepOutcome};
pub use step::{Scenario, Step, StepError};

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Where snapshots and reports are written.
///
/// Matches the directory the project's other test artefacts use.
pub const DEFAULT_RESULTS_DIR: &str = "test-results";

/// Errors raised while loading a scenario.
#[derive(Debug, Error)]
pub enum ScenarioError {
    /// The file could not be read.
    #[error("could not read {path}: {source}")]
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },

    /// The file is not valid YAML or JSON.
    #[error("could not parse {path}: {message}")]
    Parse {
        /// The file that could not be parsed.
        path: PathBuf,
        /// Parser message.
        message: String,
    },

    /// The scenario has no steps.
    #[error("{path} has no steps")]
    Empty {
        /// The file with no steps.
        path: PathBuf,
    },
}

/// Loads a scenario from a YAML or JSON file.
///
/// The extension decides the parser; anything that is not `.json` is treated
/// as YAML, which also parses JSON.
///
/// # Errors
/// Returns an error if the file cannot be read, cannot be parsed, or has no
/// steps.
pub fn load(path: &Path) -> Result<Scenario, ScenarioError> {
    let text = std::fs::read_to_string(path).map_err(|source| ScenarioError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let mut scenario: Scenario = serde_yaml::from_str(&text).map_err(|e| ScenarioError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    if scenario.steps.is_empty() {
        return Err(ScenarioError::Empty {
            path: path.to_path_buf(),
        });
    }

    if scenario.name.is_empty() {
        scenario.name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "scenario".to_string());
    }

    Ok(scenario)
}

/// Loads every scenario in a directory, sorted by file name.
///
/// # Errors
/// Returns an error if the directory cannot be read or a scenario is invalid.
pub fn load_dir(dir: &Path) -> Result<Vec<Scenario>, ScenarioError> {
    let entries = std::fs::read_dir(dir).map_err(|source| ScenarioError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "yaml" | "yml" | "json"))
        })
        .collect();
    paths.sort();

    paths.iter().map(|p| load(p)).collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write");
        path
    }

    #[test]
    fn a_yaml_scenario_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "open.yaml",
            "name: open editor\nsteps:\n  - key: F2\n  - expect_text: SSH\n",
        );

        let scenario = load(&path).expect("load");
        assert_eq!(scenario.name, "open editor");
        assert_eq!(scenario.steps.len(), 2);
    }

    #[test]
    fn a_json_scenario_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "open.json",
            r#"{"name":"json run","steps":[{"key":"F2"}]}"#,
        );

        let scenario = load(&path).expect("load");
        assert_eq!(scenario.name, "json run");
        assert_eq!(scenario.steps.len(), 1);
    }

    #[test]
    fn a_scenario_without_a_name_is_named_after_its_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "unnamed.yaml", "steps:\n  - key: F2\n");
        let scenario = load(&path).expect("load");
        assert_eq!(scenario.name, "unnamed");
    }

    #[test]
    fn a_scenario_with_no_steps_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "empty.yaml", "name: nothing\nsteps: []\n");
        assert!(matches!(load(&path), Err(ScenarioError::Empty { .. })));
    }

    #[test]
    fn a_missing_file_is_reported_with_its_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("absent.yaml");
        match load(&missing) {
            Err(ScenarioError::Io { path, .. }) => assert_eq!(path, missing),
            other => panic!("expected an Io error, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_file_is_reported_as_a_parse_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "bad.yaml", "steps: [ this is not valid");
        assert!(matches!(load(&path), Err(ScenarioError::Parse { .. })));
    }

    #[test]
    fn an_unknown_step_is_reported_rather_than_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "unknown.yaml", "steps:\n  - wobble: 3\n");
        assert!(
            load(&path).is_err(),
            "a typo in a step name must fail the run, not be skipped"
        );
    }

    #[test]
    fn a_directory_loads_every_scenario_in_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "b.yaml", "name: b\nsteps:\n  - key: F2\n");
        write(dir.path(), "a.yaml", "name: a\nsteps:\n  - key: F1\n");
        write(dir.path(), "notes.txt", "ignored");

        let scenarios = load_dir(dir.path()).expect("load dir");
        let names: Vec<&str> = scenarios.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn an_empty_directory_yields_no_scenarios() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(load_dir(dir.path()).expect("load dir").is_empty());
    }

    #[test]
    fn a_missing_directory_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(load_dir(&dir.path().join("absent")).is_err());
    }
}
