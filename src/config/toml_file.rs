//! Settings as one schema-validated TOML file.
//!
//! Ratterm's settings live in `~/.ratrc`, a hand-parsed INI-ish format, while
//! everything around it — hosts, quick-connect slots, breakpoints, saved
//! searches — is TOML or JSON. `~/.ratterm/config.toml` is the consolidated
//! form: the same settings, in sections named by
//! [`crate::config::schema::Group`], with the same validation.
//!
//! Both formats stay readable. `.ratrc` is what the documentation, the shipped
//! default and every existing installation use, and breaking it to gain a file
//! extension would be a poor trade. When both exist the TOML file wins, since
//! writing one is a deliberate act.
//!
//! Conversion is exact in one direction: every `.ratrc` setting has a TOML
//! home, so [`render`] loses nothing. A key this build does not recognise is
//! carried into an `[unknown]` section rather than dropped, so migrating a file
//! from a newer build and back does not silently delete settings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::schema::{self, Classified, Group, SETTINGS};
use super::validate::{Issue, validate};

/// Section holding keys this build does not recognise.
const UNKNOWN_SECTION: &str = "unknown";

/// Largest config file read, in bytes.
///
/// A settings file is a few kilobytes. Anything past this is a mistake — a log
/// redirected over it, a wrong path — and reading it whole would be the least
/// useful possible response.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// The consolidated settings file, if the user has one.
#[must_use]
pub fn default_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ratterm")
        .join("config.toml")
}

/// Reading a TOML settings file failed.
#[derive(Debug, thiserror::Error)]
pub enum TomlError {
    /// The file could not be read.
    #[error("could not read {path}: {source}. Check the file exists and is readable.")]
    Read {
        /// The path that failed.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file is not TOML.
    #[error("{path} is not valid TOML: {message}. Fix the syntax, or delete the file to fall back to ~/.ratrc.")]
    Syntax {
        /// The path that failed.
        path: String,
        /// What the parser said.
        message: String,
    },
    /// The file is implausibly large.
    #[error("{path} is {size} bytes, larger than the {max} byte limit. This is probably not a settings file.")]
    TooLarge {
        /// The path that failed.
        path: String,
        /// Its size.
        size: u64,
        /// The limit.
        max: u64,
    },
}

/// Settings read from a TOML file, as canonical `.ratrc` key and value pairs.
///
/// Returning pairs rather than a populated `Config` means one parser applies
/// them, so the two formats cannot drift in what a value means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TomlSettings {
    /// Canonical key to value, in file order within each section.
    pub pairs: Vec<(String, String)>,
    /// Keys this build did not recognise, with their section.
    pub unrecognised: Vec<(String, String)>,
}

impl TomlSettings {
    /// Renders the pairs as `.ratrc` lines, which the existing parser reads.
    #[must_use]
    pub fn to_ratrc(&self) -> String {
        let mut text = String::new();
        for (key, value) in &self.pairs {
            text.push_str(key);
            text.push_str(" = ");
            text.push_str(value);
            text.push('\n');
        }
        text
    }

    /// Problems with the settings, checked the same way `.ratrc` is.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        validate(&self.to_ratrc())
    }
}

/// Reads a TOML settings file.
///
/// # Errors
/// Returns an error if the file cannot be read, is implausibly large, or is
/// not valid TOML.
pub fn load(path: &Path) -> Result<TomlSettings, TomlError> {
    let display = path.display().to_string();

    let size = std::fs::metadata(path)
        .map_err(|source| TomlError::Read {
            path: display.clone(),
            source,
        })?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(TomlError::TooLarge {
            path: display,
            size,
            max: MAX_FILE_BYTES,
        });
    }

    let text = std::fs::read_to_string(path).map_err(|source| TomlError::Read {
        path: display.clone(),
        source,
    })?;

    parse(&text).map_err(|message| TomlError::Syntax {
        path: display,
        message,
    })
}

/// Parses TOML text into canonical settings.
///
/// # Errors
/// Returns the parser's message if the text is not valid TOML.
pub fn parse(text: &str) -> Result<TomlSettings, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut settings = TomlSettings::default();

    for section in Group::all() {
        let name = section.section();
        let Some(values) = table.get(name).and_then(toml::Value::as_table) else {
            continue;
        };

        for (toml_key, value) in values {
            let rendered = render_value(value);
            match canonical_key(*section, toml_key) {
                Some(key) => settings.pairs.push((key, rendered)),
                None => settings
                    .unrecognised
                    .push((format!("{name}.{toml_key}"), rendered)),
            }
        }
    }

    // Sections that are not groups: colours, alerts by metric, keybindings and
    // anything a newer build wrote.
    for (name, value) in &table {
        if Group::all().iter().any(|g| g.section() == name) {
            continue;
        }
        let Some(values) = value.as_table() else {
            settings
                .unrecognised
                .push((name.clone(), render_value(value)));
            continue;
        };
        for (leaf, leaf_value) in values {
            let key = format!("{name}.{leaf}");
            let rendered = render_value(leaf_value);
            if matches!(schema::classify(&key), Classified::Unknown) {
                if name == UNKNOWN_SECTION {
                    settings.unrecognised.push((leaf.clone(), rendered));
                } else {
                    settings.unrecognised.push((key, rendered));
                }
            } else {
                settings.pairs.push((key, rendered));
            }
        }
    }

    Ok(settings)
}

/// The `.ratrc` name for a section and key, if this build has one.
#[must_use]
pub fn canonical_key(group: Group, toml_key: &str) -> Option<String> {
    SETTINGS
        .iter()
        .find(|s| s.group == group && s.toml_key == toml_key)
        .map(|s| s.key.to_string())
}

/// Renders a TOML value the way the `.ratrc` parser expects to read it.
fn render_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Float(n) => n.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(items) => items
            .iter()
            .map(render_value)
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

/// Renders `.ratrc` content as the consolidated TOML file.
///
/// Used by the migration path: read the existing file, write the new one, and
/// the settings are identical because both go through the same schema.
#[must_use]
pub fn render(ratrc: &str) -> String {
    let mut sections: BTreeMap<&'static str, Vec<(String, String)>> = BTreeMap::new();
    let mut extras: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    for line in ratrc.lines() {
        let text = line.trim();
        if text.is_empty() || text.starts_with('#') || text.starts_with('[') {
            continue;
        }
        let Some((key, value)) = text.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = strip_comment(key, value.trim());

        match schema::classify(key) {
            Classified::Known(setting) => {
                sections
                    .entry(setting.group.section())
                    .or_default()
                    .push((setting.toml_key.to_string(), value.to_string()));
            }
            Classified::Alert { metric, .. } => {
                extras
                    .entry("alert".to_string())
                    .or_default()
                    .push((metric, value.to_string()));
            }
            Classified::Color | Classified::Addon => {
                if let Some((head, leaf)) = key.split_once('.') {
                    extras
                        .entry(head.to_string())
                        .or_default()
                        .push((leaf.to_string(), value.to_string()));
                }
            }
            Classified::Position | Classified::DockerLog | Classified::Binding => {
                let section = match schema::classify(key) {
                    Classified::Position => "window",
                    Classified::DockerLog => "docker_logs",
                    _ => "keys",
                };
                extras
                    .entry(section.to_string())
                    .or_default()
                    .push((key.to_string(), value.to_string()));
            }
            Classified::Unknown => {
                extras
                    .entry(UNKNOWN_SECTION.to_string())
                    .or_default()
                    .push((key.to_string(), value.to_string()));
            }
        }
    }

    let mut out = String::from(
        "# Ratterm settings.\n\
         #\n\
         # Written from ~/.ratrc. Both files are read; this one wins when both\n\
         # exist. Every key is checked against the same schema, so a misspelled\n\
         # name is reported rather than ignored.\n",
    );

    for group in Group::all() {
        let Some(entries) = sections.get(group.section()) else {
            continue;
        };
        out.push_str(&format!("\n[{}]\n", group.section()));
        for (key, value) in entries {
            out.push_str(&format!("{key} = {}\n", quote(group_kind(*group, key), value)));
        }
    }

    for (section, entries) in &extras {
        out.push_str(&format!("\n[{section}]\n"));
        for (key, value) in entries {
            out.push_str(&format!("{key} = {}\n", quote(None, value)));
        }
    }

    out
}

/// The kind of a setting, for deciding how to quote its value.
fn group_kind(group: Group, toml_key: &str) -> Option<schema::ValueKind> {
    SETTINGS
        .iter()
        .find(|s| s.group == group && s.toml_key == toml_key)
        .map(|s| s.kind)
}

/// Writes a value as TOML: a bare number or boolean where that is what it is,
/// a quoted string otherwise.
fn quote(kind: Option<schema::ValueKind>, value: &str) -> String {
    if let Some(flag) = schema::parse_flag(value)
        && matches!(kind, Some(schema::ValueKind::Flag))
    {
        return flag.to_string();
    }

    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        // A colour is `#1e1e2e`, never a number, and a version-like value must
        // stay a string; both fail the parse above.
        return value.to_string();
    }

    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Removes a trailing comment, leaving a hex colour alone.
fn strip_comment<'a>(key: &str, value: &'a str) -> &'a str {
    if value.starts_with('#') && matches!(schema::classify(key), Classified::Color) {
        return value;
    }
    value.split('#').next().unwrap_or(value).trim()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_document_yields_no_settings() {
        let settings = parse("").expect("valid TOML");
        assert!(settings.pairs.is_empty());
        assert!(settings.unrecognised.is_empty());
    }

    #[test]
    fn sections_map_to_canonical_keys() {
        let settings = parse(
            "[general]\nmode = \"vim\"\n\n[metrics]\nhistory = true\nraw_days = 3\n",
        )
        .expect("valid TOML");

        let pairs: Vec<(&str, &str)> = settings
            .pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        assert!(pairs.contains(&("mode", "vim")), "{pairs:?}");
        assert!(pairs.contains(&("metrics_history", "true")), "{pairs:?}");
        assert!(pairs.contains(&("metrics_raw_days", "3")), "{pairs:?}");
    }

    #[test]
    fn the_result_reads_back_through_the_ratrc_parser() {
        let settings = parse("[general]\nmode = \"emacs\"\n").expect("valid TOML");
        let mut config = super::super::Config::default();
        config.parse(&settings.to_ratrc());
        assert_eq!(config.mode, super::super::KeybindingMode::Emacs);
    }

    #[test]
    fn alerts_and_colours_keep_their_dotted_names() {
        let settings =
            parse("[alert]\ncpu = 90\n\n[terminal]\nbackground = \"#1e1e2e\"\n").expect("valid");
        let pairs: Vec<&str> = settings.pairs.iter().map(|(k, _)| k.as_str()).collect();
        assert!(pairs.contains(&"alert.cpu"), "{pairs:?}");
        assert!(pairs.contains(&"terminal.background"), "{pairs:?}");
    }

    #[test]
    fn an_unrecognised_key_is_carried_rather_than_dropped() {
        // A file written by a newer build must survive a round trip.
        let settings = parse("[general]\nfuture_setting = \"yes\"\n").expect("valid");
        assert!(settings.pairs.is_empty());
        assert_eq!(
            settings.unrecognised,
            vec![("general.future_setting".to_string(), "yes".to_string())]
        );
    }

    #[test]
    fn malformed_toml_is_an_error_with_the_parser_message() {
        let error = parse("[general\nmode = vim").expect_err("not TOML");
        assert!(!error.is_empty());
    }

    #[test]
    fn a_list_is_rendered_as_the_parser_expects() {
        let settings = parse("[theme]\ntab_themes = [\"dark\", \"nord\"]\n").expect("valid");
        assert_eq!(
            settings.pairs,
            vec![("tab_themes".to_string(), "dark, nord".to_string())]
        );
    }

    #[test]
    fn rendering_ratrc_produces_sectioned_toml() {
        let toml = render("mode = vim\nmetrics_history = true\nalert.cpu = 90\n");
        assert!(toml.contains("[general]"), "{toml}");
        assert!(toml.contains("mode = \"vim\""), "{toml}");
        assert!(toml.contains("[metrics]"), "{toml}");
        assert!(toml.contains("history = true"), "{toml}");
        assert!(toml.contains("[alert]"), "{toml}");
        assert!(toml.contains("cpu = 90"), "{toml}");
    }

    #[test]
    fn a_rendered_file_parses_back_to_the_same_settings() {
        let original = "mode = vim\nshell = bash\nmetrics_history = true\n\
                        metrics_raw_days = 7\nalert.cpu = 90\nalert.temperature = 85\n\
                        terminal.background = #1e1e2e\nlog_level = debug\n";

        let toml = render(original);
        let settings = parse(&toml).expect("what we wrote is valid TOML");

        let mut from_toml = super::super::Config::default();
        from_toml.parse(&settings.to_ratrc());

        let mut from_ratrc = super::super::Config::default();
        from_ratrc.parse(original);

        assert_eq!(from_toml.mode, from_ratrc.mode);
        assert_eq!(from_toml.shell, from_ratrc.shell);
        assert_eq!(from_toml.metrics_history, from_ratrc.metrics_history);
        assert_eq!(from_toml.metrics_raw_days, from_ratrc.metrics_raw_days);
        assert_eq!(from_toml.alerts, from_ratrc.alerts);
        assert_eq!(from_toml.log_config.level, from_ratrc.log_config.level);
    }

    #[test]
    fn the_shipped_default_survives_the_round_trip() {
        let toml = render(super::super::DEFAULT_RATRC);
        let settings = parse(&toml).expect("the shipped default renders to valid TOML");
        assert!(settings.issues().is_empty(), "{:#?}", settings.issues());
    }

    #[test]
    fn an_unknown_ratrc_key_lands_in_its_own_section() {
        let toml = render("frobnicate = 3\n");
        assert!(toml.contains("[unknown]"), "{toml}");
        assert!(toml.contains("frobnicate = 3"), "{toml}");

        // ...and comes back out as unrecognised rather than as a setting.
        let settings = parse(&toml).expect("valid");
        assert_eq!(
            settings.unrecognised,
            vec![("frobnicate".to_string(), "3".to_string())]
        );
    }

    #[test]
    fn a_comment_does_not_become_part_of_a_value() {
        let toml = render("mode = vim   # my preference\n");
        assert!(toml.contains("mode = \"vim\""), "{toml}");
    }

    #[test]
    fn a_hex_colour_is_quoted_rather_than_treated_as_a_comment() {
        let toml = render("terminal.background = #1e1e2e\n");
        assert!(toml.contains("background = \"#1e1e2e\""), "{toml}");
    }

    #[test]
    fn a_value_with_a_quote_in_it_is_escaped() {
        let toml = render("lsp-rust = say \"hello\"\n");
        let settings = parse(&toml).expect("escaping produced valid TOML");
        assert_eq!(
            settings.pairs,
            vec![("lsp-rust".to_string(), "say \"hello\"".to_string())]
        );
    }

    #[test]
    fn loading_a_missing_file_says_which_file() {
        let path = std::env::temp_dir().join("ratterm-no-such-config-file.toml");
        let _ = std::fs::remove_file(&path);
        let error = load(&path).expect_err("no file");
        assert!(error.to_string().contains("ratterm-no-such-config-file"));
    }

    #[test]
    fn loading_an_oversized_file_is_refused_rather_than_read() {
        let path = std::env::temp_dir().join("ratterm-oversized-config.toml");
        let filler = "# ".repeat(MAX_FILE_BYTES as usize);
        std::fs::write(&path, filler).expect("write");

        let error = load(&path).expect_err("too large");
        assert!(matches!(error, TomlError::TooLarge { .. }), "{error}");
        assert!(error.to_string().contains("probably not a settings file"));

        std::fs::remove_file(&path).expect("clean up");
    }

    #[test]
    fn loading_a_real_file_works_end_to_end() {
        let path = std::env::temp_dir().join("ratterm-config-roundtrip.toml");
        std::fs::write(&path, "[general]\nmode = \"emacs\"\n").expect("write");

        let settings = load(&path).expect("a valid file");
        assert_eq!(
            settings.pairs,
            vec![("mode".to_string(), "emacs".to_string())]
        );
        assert!(settings.issues().is_empty());

        std::fs::remove_file(&path).expect("clean up");
    }

    #[test]
    fn the_default_path_is_under_the_ratterm_directory() {
        let path = default_path();
        assert!(path.ends_with("config.toml"), "{}", path.display());
        assert!(
            path.to_string_lossy().contains(".ratterm"),
            "{}",
            path.display()
        );
    }
}
