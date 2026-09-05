//! The one description of every setting ratterm accepts.
//!
//! Settings used to exist only as arms of a `match` in `Config::apply_setting`.
//! Nothing else knew what a key was called, what values it took, or that it
//! existed at all, so a misspelled key was silently ignored, the shipped
//! `.ratrc` could document a setting the parser did not have, and there was no
//! way to write the settings back out in another format.
//!
//! This table is that missing description. [`crate::config::validate`] checks a
//! file against it, [`crate::config::toml_file`] reads and writes TOML from it,
//! and the parser still does the applying — the table says what is legal, not
//! what it means.

use crate::config::KeyAction;
// Colour keys come from the theme layer rather than being repeated here: a
// colour `.ratrc` accepts that the schema does not know would be reported as a
// typo, and one the schema knows that the theme layer does not would be
// silently ignored.
use crate::theme::custom::COLOR_KEYS;

/// What values a setting accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueKind {
    /// `true`, `false`, `yes`, `no`, `1`, `0`, `on`, `off`.
    Flag,
    /// One of a fixed set of words.
    Choice(&'static [&'static str]),
    /// A whole number within a range, inclusive.
    Integer {
        /// Smallest accepted value.
        min: i64,
        /// Largest accepted value.
        max: i64,
    },
    /// A number within a range, inclusive. Zero always means "off".
    Number {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// Free text, such as a program name or a path.
    Text,
    /// A colour name or `#rrggbb`.
    Color,
    /// A key combination, such as `ctrl+shift+p`.
    Binding,
    /// A modifier prefix, such as `ctrl` or `ctrl+shift`.
    Modifiers,
    /// A window position, such as `center` or `12,4`.
    Position,
    /// A comma-separated list of names.
    List,
}

/// Which part of the interface a setting belongs to.
///
/// Used as the section name when settings are written as TOML, and to group
/// them in a report, so a related pair does not end up on opposite ends of a
/// list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// Shell, mode and startup behaviour.
    General,
    /// Editor behaviour.
    Editor,
    /// Language servers.
    Lsp,
    /// Git integration.
    Git,
    /// SSH hosts and credentials.
    Ssh,
    /// Container log streaming.
    DockerLogs,
    /// Fleet metric history.
    Metrics,
    /// Alert thresholds.
    Alerts,
    /// File logging.
    Logging,
    /// Colours and themes.
    Theme,
}

impl Group {
    /// The section name this group uses in a TOML file.
    #[must_use]
    pub const fn section(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Editor => "editor",
            Self::Lsp => "lsp",
            Self::Git => "git",
            Self::Ssh => "ssh",
            Self::DockerLogs => "docker_logs",
            Self::Metrics => "metrics",
            Self::Alerts => "alerts",
            Self::Logging => "logging",
            Self::Theme => "theme",
        }
    }

    /// Every group, in the order a written file should present them.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::General,
            Self::Editor,
            Self::Lsp,
            Self::Git,
            Self::Ssh,
            Self::DockerLogs,
            Self::Metrics,
            Self::Alerts,
            Self::Logging,
            Self::Theme,
        ]
    }
}

/// One setting.
///
/// Two settings are the same when they have the same key; the rest of the row
/// is a description of that key, so comparing it would be comparing prose.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// The name used in `.ratrc`.
    pub key: &'static str,
    /// Other spellings accepted for the same setting.
    pub aliases: &'static [&'static str],
    /// The name used inside this setting's TOML section.
    pub toml_key: &'static str,
    /// Which section it belongs to.
    pub group: Group,
    /// What values it accepts.
    pub kind: ValueKind,
    /// One line, for a report or a generated file.
    pub summary: &'static str,
}

impl PartialEq for Setting {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Setting {}

impl Setting {
    /// True if `key` names this setting, by its own name or an alias.
    #[must_use]
    pub fn matches(&self, key: &str) -> bool {
        self.key == key || self.aliases.contains(&key)
    }
}

/// Words accepted for a true flag.
const TRUE_WORDS: [&str; 4] = ["true", "yes", "1", "on"];

/// Words accepted for a false flag.
const FALSE_WORDS: [&str; 4] = ["false", "no", "0", "off"];

/// Keybinding modes.
const MODES: &[&str] = &["vim", "emacs", "default"];

/// Shells the terminal can start.
const SHELLS: &[&str] = &["system", "powershell", "pwsh", "ps", "bash", "cmd", "zsh", "fish"];

/// Credential storage backends.
const STORAGE_MODES: &[&str] = &["keychain", "encrypted", "plaintext"];

/// Log levels.
const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error", "off"];

/// Every setting ratterm accepts by a fixed name.
///
/// Settings whose names are patterns rather than fixed words — `alert.cpu`,
/// `addon.<name>`, `<popup>_position`, and the keybinding actions — are
/// recognised by [`classify`] instead.
pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "mode",
        aliases: &[],
        toml_key: "mode",
        group: Group::General,
        kind: ValueKind::Choice(MODES),
        summary: "Keybinding mode",
    },
    Setting {
        key: "shell",
        aliases: &[],
        toml_key: "shell",
        group: Group::General,
        kind: ValueKind::Choice(SHELLS),
        summary: "Shell started in new terminal tabs",
    },
    Setting {
        key: "auto_close_tabs_on_shell_change",
        aliases: &[],
        toml_key: "auto_close_tabs_on_shell_change",
        group: Group::General,
        kind: ValueKind::Flag,
        summary: "Close existing tabs when the shell changes",
    },
    Setting {
        key: "ide_always",
        aliases: &["ide-always"],
        toml_key: "ide_always",
        group: Group::General,
        kind: ValueKind::Flag,
        summary: "Always show the IDE pane",
    },
    Setting {
        key: "git_gutter",
        aliases: &["git-gutter"],
        toml_key: "gutter",
        group: Group::Git,
        kind: ValueKind::Flag,
        summary: "Show git change marks beside the editor",
    },
    Setting {
        key: "git_blame",
        aliases: &["git-blame"],
        toml_key: "blame",
        group: Group::Git,
        kind: ValueKind::Flag,
        summary: "Enable the blame view",
    },
    Setting {
        key: "lsp-rust",
        aliases: &["lsp_rust"],
        toml_key: "rust",
        group: Group::Lsp,
        kind: ValueKind::Text,
        summary: "Language server to use for Rust",
    },
    Setting {
        key: "lsp-python",
        aliases: &["lsp_python"],
        toml_key: "python",
        group: Group::Lsp,
        kind: ValueKind::Text,
        summary: "Language server to use for Python",
    },
    Setting {
        key: "lsp-format-on-save",
        aliases: &["lsp_format_on_save"],
        toml_key: "format_on_save",
        group: Group::Lsp,
        kind: ValueKind::Flag,
        summary: "Format through the language server when saving",
    },
    Setting {
        key: "ssh_storage_mode",
        aliases: &[],
        toml_key: "storage_mode",
        group: Group::Ssh,
        kind: ValueKind::Choice(STORAGE_MODES),
        summary: "Where SSH credentials are kept",
    },
    Setting {
        key: "set_ssh_tab",
        aliases: &[],
        toml_key: "quick_connect_prefix",
        group: Group::Ssh,
        kind: ValueKind::Modifiers,
        summary: "Modifier prefix for SSH quick connect",
    },
    Setting {
        key: "ssh_number_setting",
        aliases: &[],
        toml_key: "quick_connect_numbers",
        group: Group::Ssh,
        kind: ValueKind::Flag,
        summary: "Enable SSH quick connect on the number keys",
    },
    Setting {
        key: "log_level",
        aliases: &[],
        toml_key: "level",
        group: Group::Logging,
        kind: ValueKind::Choice(LOG_LEVELS),
        summary: "Lowest level written to the log file",
    },
    Setting {
        key: "log_retention",
        aliases: &["log_retention_hours"],
        toml_key: "retention_hours",
        group: Group::Logging,
        kind: ValueKind::Integer { min: 1, max: 8760 },
        summary: "Hours of log files to keep",
    },
    Setting {
        key: "log_enabled",
        aliases: &["logging"],
        toml_key: "enabled",
        group: Group::Logging,
        kind: ValueKind::Flag,
        summary: "Write a log file at all",
    },
    Setting {
        key: "metrics_history",
        aliases: &["metrics-history"],
        toml_key: "history",
        group: Group::Metrics,
        kind: ValueKind::Flag,
        summary: "Keep fleet metric history on disk",
    },
    Setting {
        key: "metrics_raw_days",
        aliases: &["metrics-raw-days"],
        toml_key: "raw_days",
        group: Group::Metrics,
        kind: ValueKind::Integer { min: 1, max: 90 },
        summary: "Days of raw samples before per-minute averaging",
    },
    Setting {
        key: "theme",
        aliases: &[],
        toml_key: "preset",
        group: Group::Theme,
        kind: ValueKind::Text,
        summary: "Named theme preset",
    },
    Setting {
        key: "tab_theme_pattern",
        aliases: &[],
        toml_key: "tab_pattern",
        group: Group::Theme,
        kind: ValueKind::Choice(&["same", "none", "sequential", "cycle", "random"]),
        summary: "How new tabs pick a theme",
    },
    Setting {
        key: "tab_themes",
        aliases: &[],
        toml_key: "tab_themes",
        group: Group::Theme,
        kind: ValueKind::List,
        summary: "Themes cycled through for new tabs",
    },
];

/// Alert metrics and the range each accepts.
const ALERT_METRICS: &[(&str, f64)] = &[
    ("cpu", 100.0),
    ("memory", 100.0),
    ("mem", 100.0),
    ("ram", 100.0),
    ("disk", 100.0),
    ("temperature", 150.0),
    ("temp", 150.0),
];


/// What a key in a config file turns out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum Classified {
    /// A setting from [`SETTINGS`].
    Known(&'static Setting),
    /// An alert threshold, `alert.<metric>`, with its ceiling.
    Alert {
        /// The metric named after the dot.
        metric: String,
        /// Largest value that can sensibly fire.
        ceiling: f64,
    },
    /// A colour, from [`COLOR_KEYS`].
    Color,
    /// A window position, `<popup>_position`.
    Position,
    /// An addon, `addon.<name>`, whose value is `<hotkey>|<command>`.
    Addon,
    /// A container log setting, `docker_log_<name>`.
    DockerLog,
    /// A keybinding override, named after an action.
    Binding,
    /// Not a setting this build understands.
    Unknown,
}

/// Decides what a key is.
///
/// Fixed names are looked up in [`SETTINGS`]; the rest are patterns, which is
/// why this is a function rather than one more table.
#[must_use]
pub fn classify(key: &str) -> Classified {
    if let Some(setting) = SETTINGS.iter().find(|s| s.matches(key)) {
        return Classified::Known(setting);
    }

    if let Some(metric) = key.strip_prefix("alert.") {
        let metric = metric.trim();
        if let Some((_, ceiling)) = ALERT_METRICS.iter().find(|(name, _)| *name == metric) {
            return Classified::Alert {
                metric: metric.to_string(),
                ceiling: *ceiling,
            };
        }
        return Classified::Unknown;
    }

    if COLOR_KEYS.contains(&key) {
        return Classified::Color;
    }

    if key.ends_with("_position") {
        return Classified::Position;
    }

    if key.starts_with("addon.") {
        return Classified::Addon;
    }

    if key.starts_with("docker_log_") {
        return Classified::DockerLog;
    }

    if KeyAction::parse_action(key).is_some() {
        return Classified::Binding;
    }

    Classified::Unknown
}

/// Every key name this build accepts, for suggesting a correction.
#[must_use]
pub fn known_keys() -> Vec<String> {
    let mut keys: Vec<String> = SETTINGS
        .iter()
        .flat_map(|s| std::iter::once(s.key).chain(s.aliases.iter().copied()))
        .map(str::to_string)
        .collect();

    keys.extend(ALERT_METRICS.iter().map(|(name, _)| format!("alert.{name}")));
    keys.extend(COLOR_KEYS.iter().map(|k| (*k).to_string()));
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Reads a flag, accepting every spelling the parser accepts.
#[must_use]
pub fn parse_flag(value: &str) -> Option<bool> {
    let lowered = value.trim().to_lowercase();
    if TRUE_WORDS.contains(&lowered.as_str()) {
        Some(true)
    } else if FALSE_WORDS.contains(&lowered.as_str()) {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_has_a_unique_key() {
        let mut seen: Vec<&str> = Vec::new();
        for setting in SETTINGS {
            for name in std::iter::once(setting.key).chain(setting.aliases.iter().copied()) {
                assert!(!seen.contains(&name), "`{name}` is declared twice");
                seen.push(name);
            }
        }
    }

    #[test]
    fn every_setting_has_a_summary_and_a_toml_name() {
        for setting in SETTINGS {
            assert!(!setting.summary.is_empty(), "{} has no summary", setting.key);
            assert!(
                !setting.toml_key.is_empty(),
                "{} has no TOML name",
                setting.key
            );
            assert!(
                !setting.toml_key.contains('-'),
                "{}: a TOML key should use underscores",
                setting.key
            );
        }
    }

    #[test]
    fn a_toml_name_is_unique_within_its_section() {
        for group in Group::all() {
            let mut seen: Vec<&str> = Vec::new();
            for setting in SETTINGS.iter().filter(|s| s.group == *group) {
                assert!(
                    !seen.contains(&setting.toml_key),
                    "[{}] has two `{}` keys",
                    group.section(),
                    setting.toml_key
                );
                seen.push(setting.toml_key);
            }
        }
    }

    #[test]
    fn a_fixed_setting_is_recognised_by_name_and_by_alias() {
        assert!(matches!(classify("mode"), Classified::Known(s) if s.key == "mode"));
        assert!(matches!(classify("ide-always"), Classified::Known(s) if s.key == "ide_always"));
        assert!(
            matches!(classify("log_retention_hours"), Classified::Known(s) if s.key == "log_retention")
        );
    }

    #[test]
    fn alert_keys_carry_their_ceiling() {
        match classify("alert.cpu") {
            Classified::Alert { metric, ceiling } => {
                assert_eq!(metric, "cpu");
                assert!((ceiling - 100.0).abs() < f64::EPSILON);
            }
            other => panic!("{other:?}"),
        }

        match classify("alert.temperature") {
            Classified::Alert { ceiling, .. } => {
                assert!((ceiling - 150.0).abs() < f64::EPSILON);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_alert_for_a_metric_that_is_not_collected_is_unknown() {
        // Silently ignoring this is how a threshold ends up never firing.
        assert_eq!(classify("alert.gpu"), Classified::Unknown);
        assert_eq!(classify("alert."), Classified::Unknown);
    }

    #[test]
    fn the_pattern_keys_are_recognised() {
        assert_eq!(classify("terminal.background"), Classified::Color);
        assert_eq!(classify("ssh_manager_position"), Classified::Position);
        assert_eq!(classify("addon.formatter"), Classified::Addon);
        assert_eq!(classify("docker_log_buffer_size"), Classified::DockerLog);
    }

    #[test]
    fn a_keybinding_action_is_recognised() {
        // The action names come from KeyAction, not from this table, so this
        // pins the two together.
        assert_eq!(classify("quit"), Classified::Binding);
    }

    #[test]
    fn nonsense_is_unknown() {
        assert_eq!(classify("mmode"), Classified::Unknown);
        assert_eq!(classify(""), Classified::Unknown);
        assert_eq!(classify("terminal.chartreuse"), Classified::Unknown);
    }

    #[test]
    fn known_keys_covers_the_table() {
        let keys = known_keys();
        assert!(keys.contains(&"mode".to_string()));
        assert!(keys.contains(&"alert.cpu".to_string()));
        assert!(keys.contains(&"terminal.background".to_string()));
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "sorted");
    }

    #[test]
    fn flags_accept_every_documented_spelling() {
        for word in TRUE_WORDS {
            assert_eq!(parse_flag(word), Some(true), "{word}");
            assert_eq!(parse_flag(&word.to_uppercase()), Some(true), "{word}");
        }
        for word in FALSE_WORDS {
            assert_eq!(parse_flag(word), Some(false), "{word}");
        }
        assert_eq!(parse_flag("maybe"), None);
        assert_eq!(parse_flag(""), None);
    }

    #[test]
    fn a_group_names_a_section_once() {
        let mut sections: Vec<&str> = Group::all().iter().map(|g| g.section()).collect();
        let before = sections.len();
        sections.sort_unstable();
        sections.dedup();
        assert_eq!(sections.len(), before, "two groups share a section name");
    }

    #[test]
    fn every_group_is_listed_in_all() {
        // A group missing from `all()` would silently drop its settings when a
        // TOML file is written.
        let listed = Group::all().len();
        let used: std::collections::BTreeSet<Group> =
            SETTINGS.iter().map(|s| s.group).collect();
        for group in used {
            assert!(Group::all().contains(&group), "{group:?} is not in all()");
        }
        assert!(listed >= 10, "unexpectedly few groups: {listed}");
    }
}
