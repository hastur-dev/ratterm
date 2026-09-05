//! Runs a scenario against an application instance.
//!
//! Every step is recorded, including the ones that passed, so a failure report
//! shows what had already happened. Snapshots are written to the results
//! directory whether or not the run passes: the snapshot for the step *before*
//! a failure is usually the one that explains it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tracing::{debug, info};

use super::step::{Scenario, Step, StepError, parse_size};
use crate::app::App;
use crate::app::snapshot::{Snapshot, SnapshotOptions, parse_mouse};

/// Longest a single scenario may take before the runner gives up.
///
/// A scenario that hangs must fail rather than stall CI.
pub const SCENARIO_BUDGET: Duration = Duration::from_secs(120);

/// The result of one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    /// Position in the scenario, from 1.
    pub index: usize,
    /// What the step was.
    pub description: String,
    /// Whether it succeeded.
    pub passed: bool,
    /// Why it failed.
    pub message: Option<String>,
}

/// The result of a whole scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioOutcome {
    /// Scenario name.
    pub name: String,
    /// Whether every step passed.
    pub passed: bool,
    /// One entry per step attempted.
    pub steps: Vec<StepOutcome>,
    /// Snapshot files written.
    pub snapshots: Vec<PathBuf>,
    /// How long the run took.
    pub duration: Duration,
    /// A warning about the scenario itself, such as having no assertions.
    pub warning: Option<String>,
}

impl ScenarioOutcome {
    /// Returns how many steps passed.
    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.passed).count()
    }

    /// Returns the first failure, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<&StepOutcome> {
        self.steps.iter().find(|s| !s.passed)
    }

    /// Renders a report for a terminal or a CI log.
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        let verdict = if self.passed { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "{verdict} {} ({}/{} steps, {:?})\n",
            self.name,
            self.passed_count(),
            self.steps.len(),
            self.duration
        ));

        if let Some(warning) = &self.warning {
            out.push_str(&format!("  warning: {warning}\n"));
        }

        for step in &self.steps {
            let mark = if step.passed { "ok  " } else { "FAIL" };
            out.push_str(&format!(
                "  {mark} {:>3}. {}\n",
                step.index, step.description
            ));
            if let Some(message) = &step.message {
                out.push_str(&format!("       {message}\n"));
            }
        }

        for path in &self.snapshots {
            out.push_str(&format!("  snapshot: {}\n", path.display()));
        }

        out
    }
}

/// Drives an application through a scenario.
pub struct ScenarioRunner {
    results_dir: PathBuf,
    budget: Duration,
    write_snapshots: bool,
}

impl Default for ScenarioRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ScenarioRunner {
    /// Creates a runner writing to the default results directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            results_dir: PathBuf::from(super::DEFAULT_RESULTS_DIR),
            budget: SCENARIO_BUDGET,
            write_snapshots: true,
        }
    }

    /// Sets where snapshots and reports are written.
    #[must_use]
    pub fn with_results_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.results_dir = dir.into();
        self
    }

    /// Sets the time budget for one scenario.
    #[must_use]
    pub const fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    /// Turns snapshot writing off, for a run that only wants assertions.
    #[must_use]
    pub const fn without_snapshots(mut self) -> Self {
        self.write_snapshots = false;
        self
    }

    /// Returns the results directory.
    #[must_use]
    pub fn results_dir(&self) -> &Path {
        &self.results_dir
    }

    /// Runs `scenario` against `app`.
    ///
    /// Stops at the first failure: later steps assume the earlier ones worked,
    /// so continuing produces noise rather than information.
    pub fn run(&self, scenario: &Scenario, app: &mut App) -> ScenarioOutcome {
        let started = Instant::now();
        let mut steps = Vec::with_capacity(scenario.steps.len());
        let mut snapshots = Vec::new();
        let mut passed = true;

        app.resize(scenario.width.max(1), scenario.height.max(1));
        if scenario.test_keys {
            app.enable_test_keys();
        }
        info!("running scenario {}", scenario.name);

        for (index, step) in scenario.steps.iter().enumerate() {
            let index = index + 1;
            let description = step.describe();
            debug!("step {index}: {description}");

            if started.elapsed() > self.budget {
                steps.push(StepOutcome {
                    index,
                    description,
                    passed: false,
                    message: Some(format!("scenario exceeded its {:?} budget", self.budget)),
                });
                passed = false;
                break;
            }

            let result = self.run_step(scenario, step, app, &mut snapshots);
            let failed = result.is_err();
            steps.push(StepOutcome {
                index,
                description,
                passed: !failed,
                message: result.err().map(|e| e.to_string()),
            });

            if failed {
                passed = false;
                // Capture what the screen looked like when it went wrong.
                if self.write_snapshots {
                    let name = format!("{}-failure", slug(&scenario.name));
                    if let Ok(path) = self.write_snapshot(app, scenario, &name) {
                        snapshots.push(path);
                    }
                }
                break;
            }
        }

        let warning = if scenario.steps.iter().any(Step::is_assertion) {
            None
        } else {
            Some(
                "this scenario contains no assertions, so it can only fail on an error".to_string(),
            )
        };

        ScenarioOutcome {
            name: scenario.name.clone(),
            passed,
            steps,
            snapshots,
            duration: started.elapsed(),
            warning,
        }
    }

    /// Runs every scenario, returning one outcome each.
    pub fn run_all(
        &self,
        scenarios: &[Scenario],
        make_app: &dyn Fn() -> Option<App>,
    ) -> Vec<ScenarioOutcome> {
        scenarios
            .iter()
            .map(|scenario| match make_app() {
                Some(mut app) => self.run(scenario, &mut app),
                None => ScenarioOutcome {
                    name: scenario.name.clone(),
                    passed: false,
                    steps: vec![StepOutcome {
                        index: 0,
                        description: "create application".to_string(),
                        passed: false,
                        message: Some("could not create an application instance".to_string()),
                    }],
                    snapshots: Vec::new(),
                    duration: Duration::ZERO,
                    warning: None,
                },
            })
            .collect()
    }

    /// Runs one step.
    fn run_step(
        &self,
        scenario: &Scenario,
        step: &Step,
        app: &mut App,
        snapshots: &mut Vec<PathBuf>,
    ) -> Result<(), StepError> {
        match step {
            Step::Note(_) => Ok(()),

            Step::Key(description) => app.inject_key_str(description).map_err(StepError::BadStep),

            Step::Type(text) => {
                app.inject_text(text);
                Ok(())
            }

            Step::Mouse(description) => {
                let event = parse_mouse(description).map_err(StepError::BadStep)?;
                app.inject_mouse(event);
                Ok(())
            }

            Step::Resize(size) => {
                let (width, height) = parse_size(size).map_err(StepError::BadStep)?;
                app.resize(width, height);
                Ok(())
            }

            Step::OpenFile(path) => app
                .open_file(PathBuf::from(path))
                .map_err(|e| StepError::Internal(format!("could not open {path}: {e}"))),

            Step::WaitMs(ms) => {
                std::thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }

            Step::Tick => {
                app.tick();
                Ok(())
            }

            Step::ExpectText(needle) => {
                let snapshot = self.snapshot(app, scenario)?;
                if snapshot.contains(needle) {
                    Ok(())
                } else {
                    Err(StepError::Assertion(format!(
                        "{needle:?} is not on screen. Frame was:\n{}",
                        indent(&snapshot.text())
                    )))
                }
            }

            Step::ExpectNotText(needle) => {
                let snapshot = self.snapshot(app, scenario)?;
                match snapshot.find_line(needle) {
                    None => Ok(()),
                    Some(line) => Err(StepError::Assertion(format!(
                        "{needle:?} is still on screen at line {line}: {:?}",
                        snapshot.lines.get(line).map(String::as_str).unwrap_or("")
                    ))),
                }
            }

            Step::ExpectStatus(needle) => {
                let status = app.status().to_string();
                if status.contains(needle) {
                    Ok(())
                } else {
                    Err(StepError::Assertion(format!(
                        "status is {status:?}, expected it to contain {needle:?}"
                    )))
                }
            }

            Step::Snapshot(name) => {
                if !self.write_snapshots {
                    return Ok(());
                }
                let path = self.write_snapshot(app, scenario, name)?;
                snapshots.push(path);
                Ok(())
            }
        }
    }

    /// Renders a snapshot at the scenario's geometry.
    fn snapshot(&self, app: &mut App, scenario: &Scenario) -> Result<Snapshot, StepError> {
        app.snapshot(SnapshotOptions::sized(scenario.width, scenario.height))
            .map_err(|e| StepError::Internal(format!("could not render a snapshot: {e}")))
    }

    /// Writes a snapshot as text and as JSON, returning the text path.
    fn write_snapshot(
        &self,
        app: &mut App,
        scenario: &Scenario,
        name: &str,
    ) -> Result<PathBuf, StepError> {
        let snapshot = app
            .snapshot(SnapshotOptions::sized(scenario.width, scenario.height))
            .map_err(|e| StepError::Internal(format!("could not render a snapshot: {e}")))?;

        std::fs::create_dir_all(&self.results_dir)
            .map_err(|e| StepError::Internal(format!("could not create the results dir: {e}")))?;

        let stem = format!("{}-{}", slug(&scenario.name), slug(name));
        let text_path = self.results_dir.join(format!("{stem}.txt"));
        let json_path = self.results_dir.join(format!("{stem}.json"));

        std::fs::write(&text_path, snapshot.text())
            .map_err(|e| StepError::Internal(format!("could not write the snapshot: {e}")))?;

        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| StepError::Internal(format!("could not encode the snapshot: {e}")))?;
        std::fs::write(&json_path, json)
            .map_err(|e| StepError::Internal(format!("could not write the snapshot: {e}")))?;

        Ok(text_path)
    }
}

/// Turns a name into something safe for a file name.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "scenario".to_string()
    } else {
        out
    }
}

/// Indents a block so it reads as one message.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated(120, 40).expect("app")
    }

    fn runner(dir: &Path) -> ScenarioRunner {
        ScenarioRunner::new().with_results_dir(dir)
    }

    #[test]
    fn a_scenario_of_actions_passes_and_warns_about_having_no_assertions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("actions only", vec![Step::Tick, Step::Key("F2".into())]);
        let outcome = runner(dir.path()).run(&scenario, &mut app());

        assert!(outcome.passed, "{}", outcome.report());
        assert_eq!(outcome.passed_count(), 2);
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn an_assertion_that_holds_passes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.set_status("SCENARIO MARKER");

        let scenario = Scenario::new(
            "status shows",
            vec![
                Step::ExpectStatus("SCENARIO MARKER".into()),
                Step::ExpectText("SCENARIO MARKER".into()),
            ],
        );
        let outcome = runner(dir.path()).run(&scenario, &mut app);
        assert!(outcome.passed, "{}", outcome.report());
        assert!(outcome.warning.is_none());
    }

    #[test]
    fn an_assertion_that_fails_stops_the_run_and_says_why() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new(
            "missing text",
            vec![
                Step::ExpectText("DEFINITELY NOT ON SCREEN".into()),
                Step::Tick,
            ],
        );
        let outcome = runner(dir.path()).run(&scenario, &mut app());

        assert!(!outcome.passed);
        assert_eq!(outcome.steps.len(), 1, "the run stops at the first failure");
        let failure = outcome.first_failure().expect("a failure");
        assert!(
            failure
                .message
                .as_deref()
                .is_some_and(|m| m.contains("DEFINITELY NOT ON SCREEN")),
            "{failure:?}"
        );
    }

    #[test]
    fn a_failure_writes_a_snapshot_of_what_was_on_screen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("fails", vec![Step::ExpectText("NOPE".into())]);
        let outcome = runner(dir.path()).run(&scenario, &mut app());

        assert!(!outcome.passed);
        assert_eq!(outcome.snapshots.len(), 1);
        assert!(outcome.snapshots[0].exists());
    }

    #[test]
    fn expect_not_text_holds_when_the_text_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new(
            "absent",
            vec![Step::ExpectNotText("DEFINITELY NOT ON SCREEN".into())],
        );
        assert!(runner(dir.path()).run(&scenario, &mut app()).passed);
    }

    #[test]
    fn expect_not_text_fails_when_the_text_is_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.set_status("PRESENT");
        let scenario = Scenario::new("present", vec![Step::ExpectNotText("PRESENT".into())]);
        let outcome = runner(dir.path()).run(&scenario, &mut app);
        assert!(!outcome.passed);
    }

    #[test]
    fn a_snapshot_step_writes_text_and_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("snap", vec![Step::Snapshot("start".into())]);
        let outcome = runner(dir.path()).run(&scenario, &mut app());

        assert!(outcome.passed, "{}", outcome.report());
        assert_eq!(outcome.snapshots.len(), 1);
        let text = &outcome.snapshots[0];
        assert!(text.exists());
        assert!(text.with_extension("json").exists());
    }

    #[test]
    fn snapshots_can_be_turned_off() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("snap", vec![Step::Snapshot("start".into())]);
        let outcome = runner(dir.path())
            .without_snapshots()
            .run(&scenario, &mut app());

        assert!(outcome.passed);
        assert!(outcome.snapshots.is_empty());
    }

    #[test]
    fn a_bad_key_description_fails_the_step() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("bad key", vec![Step::Key("nonsense".into())]);
        let outcome = runner(dir.path()).run(&scenario, &mut app());
        assert!(!outcome.passed);
        assert!(
            outcome
                .first_failure()
                .and_then(|f| f.message.as_deref())
                .is_some_and(|m| m.contains("nonsense"))
        );
    }

    #[test]
    fn a_bad_mouse_description_fails_the_step() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("bad mouse", vec![Step::Mouse("left_down".into())]);
        assert!(!runner(dir.path()).run(&scenario, &mut app()).passed);
    }

    #[test]
    fn a_bad_size_fails_the_step() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("bad size", vec![Step::Resize("wide".into())]);
        assert!(!runner(dir.path()).run(&scenario, &mut app()).passed);
    }

    #[test]
    fn opening_a_missing_file_fails_the_step() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new(
            "missing file",
            vec![Step::OpenFile("definitely-not-here-3f9a.txt".into())],
        );
        assert!(!runner(dir.path()).run(&scenario, &mut app()).passed);
    }

    #[test]
    fn a_resize_changes_the_frame_geometry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        let scenario = Scenario::new("resize", vec![Step::Resize("64x18".into())]);
        assert!(runner(dir.path()).run(&scenario, &mut app).passed);
        assert_eq!(app.screen_size(), (64, 18));
    }

    #[test]
    fn typing_reaches_the_editor() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut app = app();
        app.new_editor_tab();
        app.editor_mut().set_mode(crate::editor::EditorMode::Insert);
        app.layout_mut()
            .set_focused(crate::ui::layout::FocusedPane::Editor);

        let scenario = Scenario::new("typing", vec![Step::Type("typed".into())]);
        assert!(runner(dir.path()).run(&scenario, &mut app).passed);
        assert_eq!(app.editor().buffer().text(), "typed");
    }

    #[test]
    fn a_zero_budget_fails_rather_than_running_forever() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new(
            "slow",
            vec![Step::WaitMs(20), Step::Tick, Step::Tick, Step::Tick],
        );
        let outcome = runner(dir.path())
            .with_budget(Duration::from_millis(1))
            .run(&scenario, &mut app());

        assert!(!outcome.passed);
        assert!(
            outcome
                .first_failure()
                .and_then(|f| f.message.as_deref())
                .is_some_and(|m| m.contains("budget"))
        );
    }

    #[test]
    fn the_report_names_the_scenario_and_the_verdict() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario = Scenario::new("named run", vec![Step::Tick]);
        let report = runner(dir.path()).run(&scenario, &mut app()).report();
        assert!(report.contains("PASS"), "{report}");
        assert!(report.contains("named run"), "{report}");
    }

    #[test]
    fn run_all_reports_one_outcome_per_scenario() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = vec![
            Scenario::new("first", vec![Step::Tick]),
            Scenario::new("second", vec![Step::Tick]),
        ];
        let outcomes = runner(dir.path()).run_all(&scenarios, &|| App::isolated(80, 24).ok());
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.passed));
    }

    #[test]
    fn run_all_reports_a_failure_to_create_the_application() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = vec![Scenario::new("first", vec![Step::Tick])];
        let outcomes = runner(dir.path()).run_all(&scenarios, &|| None);
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].passed);
    }

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(slug("Docker manager opens"), "docker-manager-opens");
        assert_eq!(slug("a/b\\c"), "a-b-c");
        assert_eq!(slug("   "), "scenario");
        assert_eq!(slug(""), "scenario");
        assert_eq!(slug("trailing!!!"), "trailing");
    }
}
