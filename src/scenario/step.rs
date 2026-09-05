//! The steps a scenario is made of.
//!
//! Each step is one YAML mapping with a single key, which keeps a scenario
//! readable:
//!
//! ```yaml
//! name: docker manager opens and closes
//! steps:
//!   - key: F3
//!   - expect_text: Docker
//!   - snapshot: docker-manager-open
//!   - key: esc
//!   - expect_not_text: Docker
//! ```
//!
//! `deny_unknown_fields` is deliberate: a typo in a step name must fail the
//! run rather than being silently skipped, because a skipped assertion is a
//! test that passes for the wrong reason.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A scripted run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Name used in reports and in snapshot file names.
    #[serde(default)]
    pub name: String,

    /// Terminal size the run pretends to have.
    #[serde(default = "default_width")]
    pub width: u16,

    /// Terminal height.
    #[serde(default = "default_height")]
    pub height: u16,

    /// Enable the deterministic F1-F4 keys.
    ///
    /// On by default. The real shortcuts for the palette differ between
    /// Windows 11 and everything else, so a scenario written against them
    /// would not be the same test on every platform, which is the whole point
    /// of running these files everywhere.
    #[serde(default = "default_true")]
    pub test_keys: bool,

    /// Steps, run in order.
    pub steps: Vec<Step>,
}

fn default_width() -> u16 {
    120
}

fn default_height() -> u16 {
    40
}

fn default_true() -> bool {
    true
}

impl Scenario {
    /// Creates a scenario with the default geometry.
    #[must_use]
    pub fn new(name: impl Into<String>, steps: Vec<Step>) -> Self {
        Self {
            name: name.into(),
            width: default_width(),
            height: default_height(),
            test_keys: true,
            steps,
        }
    }
}

/// One action or assertion.
///
/// Written in a file as a one-entry mapping (`- key: F2`), or as a bare word
/// for the steps that take no argument (`- tick`). See the `Deserialize`
/// implementation below for why that is hand-written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Press a key, described as in `.ratrc`: `ctrl+q`, `F2`, `enter`.
    Key(String),

    /// Type a string, one character at a time.
    Type(String),

    /// Send a mouse event, described as `left_down@10,4`.
    Mouse(String),

    /// Assert the rendered frame contains this text.
    ExpectText(String),

    /// Assert the rendered frame does not contain this text.
    ExpectNotText(String),

    /// Assert the status bar message contains this text.
    ExpectStatus(String),

    /// Save a snapshot under this name.
    Snapshot(String),

    /// Resize the frame, as `120x40`.
    Resize(String),

    /// Open a file in the editor.
    OpenFile(String),

    /// Wait this many milliseconds.
    ///
    /// Use sparingly: a scenario that needs a sleep is usually asserting on
    /// something that should be observable instead.
    WaitMs(u64),

    /// Run one iteration of the application's update loop.
    Tick,

    /// A comment, so a scenario can explain itself.
    Note(String),
}

/// How a step is written in a file.
///
/// `serde_yaml` renders an externally tagged enum with a YAML tag (`!key F2`),
/// which is not the shape a person wants to write. This intermediate type
/// accepts the readable forms — a mapping with exactly one recognised key, or
/// a bare word for the steps that take no argument — and converts.
///
/// `deny_unknown_fields` on the mapping is what makes a misspelled step fail
/// the run: a silently skipped assertion is a test that passes for the wrong
/// reason.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StepRepr {
    /// A step with no argument, written as a bare word.
    Bare(String),
    /// A step written as a one-entry mapping.
    Mapping(Box<StepFields>),
}

/// Every field a step mapping may carry, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StepFields {
    key: Option<String>,
    #[serde(rename = "type")]
    type_text: Option<String>,
    mouse: Option<String>,
    expect_text: Option<String>,
    expect_not_text: Option<String>,
    expect_status: Option<String>,
    snapshot: Option<String>,
    resize: Option<String>,
    open_file: Option<String>,
    wait_ms: Option<u64>,
    note: Option<String>,
    /// Written as `tick: true` when the mapping form is preferred.
    tick: Option<bool>,
}

impl StepFields {
    /// Converts to a step, failing if zero or several fields are set.
    fn into_step(self) -> Result<Step, String> {
        let mut found: Vec<Step> = Vec::new();

        if let Some(v) = self.key {
            found.push(Step::Key(v));
        }
        if let Some(v) = self.type_text {
            found.push(Step::Type(v));
        }
        if let Some(v) = self.mouse {
            found.push(Step::Mouse(v));
        }
        if let Some(v) = self.expect_text {
            found.push(Step::ExpectText(v));
        }
        if let Some(v) = self.expect_not_text {
            found.push(Step::ExpectNotText(v));
        }
        if let Some(v) = self.expect_status {
            found.push(Step::ExpectStatus(v));
        }
        if let Some(v) = self.snapshot {
            found.push(Step::Snapshot(v));
        }
        if let Some(v) = self.resize {
            found.push(Step::Resize(v));
        }
        if let Some(v) = self.open_file {
            found.push(Step::OpenFile(v));
        }
        if let Some(v) = self.wait_ms {
            found.push(Step::WaitMs(v));
        }
        if let Some(v) = self.note {
            found.push(Step::Note(v));
        }
        if self.tick == Some(true) {
            found.push(Step::Tick);
        }

        match found.len() {
            1 => Ok(found.remove(0)),
            0 => Err("a step mapping must name one action".to_string()),
            n => Err(format!("a step must name one action, found {n}")),
        }
    }
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        match StepRepr::deserialize(deserializer)? {
            StepRepr::Bare(word) => match word.trim() {
                "tick" => Ok(Self::Tick),
                other => Err(D::Error::custom(format!(
                    "unknown step {other:?}; a step with no argument must be `tick`"
                ))),
            },
            StepRepr::Mapping(fields) => fields.into_step().map_err(D::Error::custom),
        }
    }
}

impl Serialize for Step {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap as _;

        let (name, value): (&str, StepValue<'_>) = match self {
            Self::Key(v) => ("key", StepValue::Text(v)),
            Self::Type(v) => ("type", StepValue::Text(v)),
            Self::Mouse(v) => ("mouse", StepValue::Text(v)),
            Self::ExpectText(v) => ("expect_text", StepValue::Text(v)),
            Self::ExpectNotText(v) => ("expect_not_text", StepValue::Text(v)),
            Self::ExpectStatus(v) => ("expect_status", StepValue::Text(v)),
            Self::Snapshot(v) => ("snapshot", StepValue::Text(v)),
            Self::Resize(v) => ("resize", StepValue::Text(v)),
            Self::OpenFile(v) => ("open_file", StepValue::Text(v)),
            Self::WaitMs(v) => ("wait_ms", StepValue::Number(*v)),
            Self::Note(v) => ("note", StepValue::Text(v)),
            Self::Tick => ("tick", StepValue::Flag),
        };

        let mut map = serializer.serialize_map(Some(1))?;
        match value {
            StepValue::Text(text) => map.serialize_entry(name, text)?,
            StepValue::Number(n) => map.serialize_entry(name, &n)?,
            StepValue::Flag => map.serialize_entry(name, &true)?,
        }
        map.end()
    }
}

/// The value side of a serialised step.
enum StepValue<'a> {
    Text(&'a str),
    Number(u64),
    Flag,
}

impl Step {
    /// Returns a one-line description for reports.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Key(k) => format!("key {k}"),
            Self::Type(t) => format!("type {t:?}"),
            Self::Mouse(m) => format!("mouse {m}"),
            Self::ExpectText(t) => format!("expect text {t:?}"),
            Self::ExpectNotText(t) => format!("expect no text {t:?}"),
            Self::ExpectStatus(t) => format!("expect status {t:?}"),
            Self::Snapshot(n) => format!("snapshot {n}"),
            Self::Resize(s) => format!("resize {s}"),
            Self::OpenFile(p) => format!("open file {p}"),
            Self::WaitMs(ms) => format!("wait {ms}ms"),
            Self::Tick => "tick".to_string(),
            Self::Note(n) => format!("note: {n}"),
        }
    }

    /// Returns true if the step asserts something.
    ///
    /// A scenario made entirely of actions proves nothing, and the runner says
    /// so rather than reporting a pass.
    #[must_use]
    pub const fn is_assertion(&self) -> bool {
        matches!(
            self,
            Self::ExpectText(_) | Self::ExpectNotText(_) | Self::ExpectStatus(_)
        )
    }
}

/// Why a step failed.
#[derive(Debug, Error)]
pub enum StepError {
    /// A key, mouse event or size could not be parsed.
    #[error("{0}")]
    BadStep(String),

    /// An assertion did not hold.
    #[error("{0}")]
    Assertion(String),

    /// Something underneath failed.
    #[error("{0}")]
    Internal(String),
}

/// Parses a `WIDTHxHEIGHT` string.
///
/// # Errors
/// Returns a message if the string is not two positive numbers separated by
/// `x`.
pub fn parse_size(text: &str) -> Result<(u16, u16), String> {
    let lowered = text.trim().to_ascii_lowercase();
    let (w, h) = lowered
        .split_once('x')
        .ok_or_else(|| format!("size {text:?} must look like 120x40"))?;

    let width: u16 = w
        .trim()
        .parse()
        .map_err(|_| format!("bad width in {text:?}"))?;
    let height: u16 = h
        .trim()
        .parse()
        .map_err(|_| format!("bad height in {text:?}"))?;

    if width == 0 || height == 0 {
        return Err(format!("size {text:?} must be positive in both dimensions"));
    }

    Ok((width, height))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_scenario_defaults_to_a_usable_geometry() {
        let scenario: Scenario = serde_yaml::from_str("steps:\n  - key: F2\n").expect("parse");
        assert_eq!(scenario.width, 120);
        assert_eq!(scenario.height, 40);
        assert!(scenario.name.is_empty());
        assert!(
            scenario.test_keys,
            "F1-F4 are on unless a scenario opts out"
        );
    }

    #[test]
    fn geometry_can_be_set_per_scenario() {
        let scenario: Scenario =
            serde_yaml::from_str("width: 80\nheight: 24\nsteps:\n  - tick\n").expect("parse");
        assert_eq!((scenario.width, scenario.height), (80, 24));
    }

    #[test]
    fn every_step_kind_parses() {
        let yaml = r#"
name: all steps
steps:
  - key: ctrl+q
  - type: hello
  - mouse: left_down@1,2
  - expect_text: something
  - expect_not_text: absent
  - expect_status: saved
  - snapshot: named
  - resize: 100x30
  - open_file: /tmp/x.txt
  - wait_ms: 5
  - tick
  - note: explaining myself
"#;
        let scenario: Scenario = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(scenario.steps.len(), 12);
        assert_eq!(scenario.steps[0], Step::Key("ctrl+q".to_string()));
        assert_eq!(scenario.steps[10], Step::Tick);
    }

    #[test]
    fn an_unknown_step_is_rejected() {
        let err = serde_yaml::from_str::<Scenario>("steps:\n  - wobble: 3\n");
        assert!(err.is_err(), "a misspelled step must not be skipped");
    }

    #[test]
    fn an_unknown_top_level_field_is_rejected() {
        let err = serde_yaml::from_str::<Scenario>("stpes:\n  - tick\n");
        assert!(err.is_err());
    }

    #[test]
    fn steps_describe_themselves() {
        assert_eq!(Step::Key("F2".into()).describe(), "key F2");
        assert_eq!(Step::Tick.describe(), "tick");
        assert!(Step::Type("hi".into()).describe().contains("hi"));
    }

    #[test]
    fn only_expectation_steps_count_as_assertions() {
        assert!(Step::ExpectText("x".into()).is_assertion());
        assert!(Step::ExpectNotText("x".into()).is_assertion());
        assert!(Step::ExpectStatus("x".into()).is_assertion());
        assert!(!Step::Key("F2".into()).is_assertion());
        assert!(!Step::Snapshot("s".into()).is_assertion());
        assert!(!Step::Tick.is_assertion());
    }

    #[test]
    fn sizes_parse() {
        assert_eq!(parse_size("120x40").expect("parse"), (120, 40));
        assert_eq!(parse_size(" 80 X 24 ").expect("parse"), (80, 24));
    }

    #[test]
    fn bad_sizes_are_reported() {
        assert!(parse_size("120").is_err());
        assert!(parse_size("axb").is_err());
        assert!(parse_size("0x40").is_err());
        assert!(parse_size("120x0").is_err());
        assert!(parse_size("").is_err());
    }

    #[test]
    fn a_bare_tick_is_accepted() {
        let scenario: Scenario = serde_yaml::from_str("steps:\n  - tick\n").expect("parse");
        assert_eq!(scenario.steps, vec![Step::Tick]);
    }

    #[test]
    fn a_tick_mapping_is_also_accepted() {
        let scenario: Scenario = serde_yaml::from_str("steps:\n  - tick: true\n").expect("parse");
        assert_eq!(scenario.steps, vec![Step::Tick]);
    }

    #[test]
    fn an_unknown_bare_word_is_rejected() {
        let err = serde_yaml::from_str::<Scenario>("steps:\n  - wobble\n").expect_err("must fail");
        assert!(err.to_string().contains("wobble"), "{err}");
    }

    #[test]
    fn a_step_naming_two_actions_is_rejected() {
        let err = serde_yaml::from_str::<Scenario>("steps:\n  - key: F2\n    tick: true\n")
            .expect_err("must fail");
        assert!(err.to_string().contains("one action"), "{err}");
    }

    #[test]
    fn an_empty_step_mapping_is_rejected() {
        let err = serde_yaml::from_str::<Scenario>("steps:\n  - {}\n").expect_err("must fail");
        assert!(err.to_string().contains("one action"), "{err}");
    }

    #[test]
    fn a_scenario_round_trips_through_yaml() {
        let scenario = Scenario::new(
            "round trip",
            vec![Step::Key("F2".into()), Step::ExpectText("SSH".into())],
        );
        let encoded = serde_yaml::to_string(&scenario).expect("serialise");
        let decoded: Scenario = serde_yaml::from_str(&encoded).expect("deserialise");
        assert_eq!(decoded, scenario);
    }
}
