//! Checks a configuration file against [`crate::config::schema`].
//!
//! Before this, a misspelled key was silently ignored: `metrics_hisory = true`
//! left history off, `alert.cpuu = 90` left the threshold unset, and neither
//! said anything. A setting that does nothing and reports nothing is worse
//! than one that fails, because the user believes it worked.
//!
//! Validation never rejects a file. A key it does not recognise might belong
//! to a newer build, and refusing to start over one would be a poor trade. It
//! reports, with the line number and — for a near miss — the name that was
//! probably meant.

use std::fmt;

use super::schema::{self, Classified, ValueKind};

/// How different two keys may be and still be treated as a typo.
///
/// Two edits catches a transposition and a doubled letter without suggesting
/// `mode` for `disk`.
const MAX_SUGGESTION_DISTANCE: usize = 2;

/// Longest value echoed back in a message.
const MAX_ECHO: usize = 40;

/// What is wrong with one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The key is not a setting this build knows.
    UnknownKey {
        /// A known key close enough to be the intended one.
        suggestion: Option<String>,
    },
    /// The key is known but the value is not one it accepts.
    BadValue {
        /// What the setting does accept, in words.
        expected: String,
    },
    /// The same key is set more than once; the last one wins.
    Repeated {
        /// Line where the key was first set.
        first_line: usize,
    },
}

/// One complaint about one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Line number, counting from one, as an editor shows it.
    pub line: usize,
    /// The key as written.
    pub key: String,
    /// What is wrong.
    pub problem: Problem,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.problem {
            Problem::UnknownKey {
                suggestion: Some(s),
            } => {
                write!(f, "`{}` is not a setting. Did you mean `{s}`?", self.key)
            }
            Problem::UnknownKey { suggestion: None } => {
                write!(f, "`{}` is not a setting.", self.key)
            }
            Problem::BadValue { expected } => {
                write!(f, "`{}` expects {expected}.", self.key)
            }
            Problem::Repeated { first_line } => write!(
                f,
                "`{}` is set again here; the value on line {first_line} is overridden.",
                self.key
            ),
        }
    }
}

/// Checks a whole file, returning every problem in the order they appear.
#[must_use]
pub fn validate(content: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut seen: Vec<(String, usize)> = Vec::new();

    for (index, raw) in content.lines().enumerate() {
        let line = index + 1;
        let text = raw.trim();

        if text.is_empty() || text.starts_with('#') || text.starts_with('[') {
            continue;
        }

        let Some((key, value)) = text.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = strip_comment(key, value.trim());

        if let Some((_, first)) = seen.iter().find(|(name, _)| name == key) {
            issues.push(Issue {
                line,
                key: key.to_string(),
                problem: Problem::Repeated { first_line: *first },
            });
        } else {
            seen.push((key.to_string(), line));
        }

        if let Some(problem) = check(key, value) {
            issues.push(Issue {
                line,
                key: key.to_string(),
                problem,
            });
        }
    }

    issues
}

/// Removes a trailing comment, leaving a hex colour alone.
///
/// `#` starts a comment everywhere except in a colour, where it starts the
/// value. The parser makes the same exception; if the two disagree, validation
/// complains about a file that works.
fn strip_comment<'a>(key: &str, value: &'a str) -> &'a str {
    if value.starts_with('#') && matches!(schema::classify(key), Classified::Color) {
        return value;
    }
    value.split('#').next().unwrap_or(value).trim()
}

/// Checks one key and value.
fn check(key: &str, value: &str) -> Option<Problem> {
    match schema::classify(key) {
        Classified::Known(setting) => check_value(setting.kind, value),
        Classified::Alert { ceiling, .. } => check_value(
            ValueKind::Number {
                min: 0.0,
                max: ceiling,
            },
            value,
        ),
        Classified::Color => check_value(ValueKind::Color, value),
        Classified::Position => check_value(ValueKind::Position, value),
        Classified::Binding => check_value(ValueKind::Binding, value),
        Classified::Addon => (!value.contains('|')).then(|| Problem::BadValue {
            expected: "a hotkey and a command separated by `|`".to_string(),
        }),
        // The container-log settings own their parsing and accept anything
        // they can make sense of; second-guessing them here would duplicate it.
        Classified::DockerLog => None,
        Classified::Unknown => Some(Problem::UnknownKey {
            suggestion: suggest(key),
        }),
    }
}

/// Checks a value against one kind.
fn check_value(kind: ValueKind, value: &str) -> Option<Problem> {
    let bad = |expected: &str| {
        Some(Problem::BadValue {
            expected: expected.to_string(),
        })
    };

    match kind {
        ValueKind::Flag => (schema::parse_flag(value).is_none()).then(|| Problem::BadValue {
            expected: format!("true or false, not {}", echo(value)),
        }),
        ValueKind::Choice(options) => {
            let lowered = value.to_lowercase();
            if options.contains(&lowered.as_str()) {
                None
            } else {
                bad(&format!("one of {}", options.join(", ")))
            }
        }
        ValueKind::Integer { min, max } => match value.parse::<i64>() {
            Ok(n) if (min..=max).contains(&n) => None,
            Ok(n) => bad(&format!("a number from {min} to {max}, not {n}")),
            Err(_) => bad(&format!("a whole number, not {}", echo(value))),
        },
        ValueKind::Number { min, max } => match value.parse::<f64>() {
            Ok(n) if n.is_finite() && (min..=max).contains(&n) => None,
            Ok(n) => bad(&format!("a number from {min} to {max}, not {n}")),
            Err(_) => bad(&format!("a number, not {}", echo(value))),
        },
        ValueKind::Text | ValueKind::List => value.is_empty().then(|| Problem::BadValue {
            expected: "a value".to_string(),
        }),
        ValueKind::Color => {
            (crate::theme::parse_color(value).is_none()).then(|| Problem::BadValue {
                expected: format!("a colour name or #rrggbb, not {}", echo(value)),
            })
        }
        ValueKind::Binding => {
            (super::KeyBinding::parse(value).is_none()).then(|| Problem::BadValue {
                expected: format!("a key combination, not {}", echo(value)),
            })
        }
        ValueKind::Modifiers => {
            let ok = !value.is_empty()
                && value
                    .split('+')
                    .all(|part| matches!(part.trim(), "ctrl" | "alt" | "shift" | "super" | "cmd"));
            (!ok).then(|| Problem::BadValue {
                expected: format!("modifiers such as ctrl or ctrl+shift, not {}", echo(value)),
            })
        }
        ValueKind::Position => crate::ui::window_position::WindowPosition::parse(value)
            .err()
            .map(|e| Problem::BadValue {
                expected: format!("a position: {e}"),
            }),
    }
}

/// Shortens a value for a message, so a pasted paragraph does not become one.
fn echo(value: &str) -> String {
    if value.is_empty() {
        return "an empty value".to_string();
    }
    if value.chars().count() <= MAX_ECHO {
        return format!("`{value}`");
    }
    let head: String = value.chars().take(MAX_ECHO).collect();
    format!("`{head}...`")
}

/// Finds the known key closest to `key`, if one is close enough.
#[must_use]
pub fn suggest(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }

    let mut best: Option<(usize, String)> = None;

    for candidate in schema::known_keys() {
        let distance = edit_distance(key, &candidate);
        if distance > MAX_SUGGESTION_DISTANCE {
            continue;
        }
        if best.as_ref().is_none_or(|(d, _)| distance < *d) {
            best = Some((distance, candidate));
        }
    }

    best.map(|(_, candidate)| candidate)
}

/// Levenshtein distance between two strings.
///
/// Two rows rather than a full matrix: the longest key here is about thirty
/// characters and this runs once per line of a config file, but there is no
/// reason to allocate the square.
#[must_use]
pub fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();

    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (i, l) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, r) in right.iter().enumerate() {
            let cost = usize::from(l != r);
            current[j + 1] = (current[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_correct_file_has_nothing_to_say() {
        let issues = validate(
            "# a comment\n\
             mode = vim\n\
             shell = bash\n\
             metrics_history = true\n\
             alert.cpu = 90\n\
             terminal.background = #1e1e2e\n",
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn an_empty_file_has_nothing_to_say() {
        assert!(validate("").is_empty());
        assert!(validate("\n\n   \n").is_empty());
    }

    #[test]
    fn a_misspelled_key_is_reported_with_the_line_and_a_suggestion() {
        let issues = validate("mode = vim\nmetrics_hisory = true\n");
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].line, 2);
        assert_eq!(
            issues[0].problem,
            Problem::UnknownKey {
                suggestion: Some("metrics_history".to_string())
            }
        );
        assert!(issues[0].to_string().contains("Did you mean"));
    }

    #[test]
    fn a_key_with_no_near_match_is_reported_without_a_guess() {
        let issues = validate("frobnicate_the_widget = 3\n");
        assert_eq!(
            issues[0].problem,
            Problem::UnknownKey { suggestion: None },
            "a wild guess is worse than none"
        );
    }

    #[test]
    fn a_bad_flag_names_what_it_wanted() {
        let issues = validate("metrics_history = sometimes\n");
        assert_eq!(issues.len(), 1);
        match &issues[0].problem {
            Problem::BadValue { expected } => {
                assert!(expected.contains("true or false"), "{expected}");
                assert!(expected.contains("sometimes"), "{expected}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_choice_lists_the_options() {
        let issues = validate("mode = vi\n");
        match &issues[0].problem {
            Problem::BadValue { expected } => {
                assert!(expected.contains("vim"), "{expected}");
                assert!(expected.contains("emacs"), "{expected}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_number_outside_its_range_is_reported() {
        let issues = validate("metrics_raw_days = 4000\n");
        assert_eq!(issues.len(), 1);
        match &issues[0].problem {
            Problem::BadValue { expected } => assert!(expected.contains("1 to 90"), "{expected}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_alert_above_its_ceiling_is_reported() {
        let issues = validate("alert.cpu = 150\n");
        assert_eq!(issues.len(), 1, "a rule that can never fire is a mistake");

        // The same number is fine for a temperature.
        assert!(validate("alert.temperature = 150\n").is_empty());
    }

    #[test]
    fn a_repeated_key_is_reported_once_and_points_at_the_first() {
        let issues = validate("mode = vim\nshell = bash\nmode = emacs\n");
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].line, 3);
        assert_eq!(issues[0].problem, Problem::Repeated { first_line: 1 });
        assert!(issues[0].to_string().contains("line 1"));
    }

    #[test]
    fn a_hex_colour_is_not_mistaken_for_a_comment() {
        assert!(validate("terminal.background = #1e1e2e\n").is_empty());
    }

    #[test]
    fn a_trailing_comment_is_not_part_of_the_value() {
        assert!(validate("alert.cpu = 90   # shout at me\n").is_empty());
    }

    #[test]
    fn a_bad_colour_is_reported() {
        let issues = validate("editor.background = chartreuse\n");
        assert_eq!(issues.len(), 1, "{issues:?}");
    }

    #[test]
    fn an_addon_without_a_command_is_reported() {
        let issues = validate("addon.formatter = ctrl+alt+f\n");
        assert_eq!(issues.len(), 1);
        match &issues[0].problem {
            Problem::BadValue { expected } => assert!(expected.contains('|'), "{expected}"),
            other => panic!("{other:?}"),
        }

        assert!(validate("addon.formatter = ctrl+alt+f|rustfmt %f\n").is_empty());
    }

    #[test]
    fn a_section_header_is_skipped_rather_than_reported() {
        // A TOML file validated by mistake should not produce noise per line.
        assert!(validate("[general]\nmode = vim\n").is_empty());
    }

    #[test]
    fn a_line_with_no_equals_sign_is_left_alone() {
        assert!(validate("this is prose, not a setting\n").is_empty());
    }

    #[test]
    fn a_long_value_is_shortened_in_the_message() {
        let long = "x".repeat(200);
        let issues = validate(&format!("metrics_history = {long}\n"));
        let message = issues[0].to_string();
        assert!(message.contains("..."), "{message}");
        assert!(
            message.len() < 150,
            "the message is {} chars",
            message.len()
        );
    }

    #[test]
    fn edit_distance_is_what_it_says() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("abc", "abd"), 1);
        assert_eq!(edit_distance("abc", "acb"), 2);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn a_distant_key_is_not_suggested() {
        assert_eq!(suggest("disk"), None, "`disk` is not a typo of `mode`");
        assert_eq!(suggest(""), None);
    }

    #[test]
    fn a_close_key_is_suggested() {
        assert_eq!(suggest("mdoe").as_deref(), Some("mode"));
        assert_eq!(suggest("alert.cpuu").as_deref(), Some("alert.cpu"));
    }

    #[test]
    fn several_problems_are_reported_in_file_order() {
        let issues = validate("nonsense = 1\nmode = vi\nmetrics_raw_days = 0\n");
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert_eq!(issues[0].line, 1);
        assert_eq!(issues[1].line, 2);
        assert_eq!(issues[2].line, 3);
    }

    #[test]
    fn the_shipped_default_file_validates_clean() {
        // If the file ratterm writes on first run has a problem in it, every
        // user starts with a warning.
        let issues = validate(super::super::DEFAULT_RATRC);
        assert!(issues.is_empty(), "{issues:#?}");
    }
}
