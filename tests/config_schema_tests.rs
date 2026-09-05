//! End-to-end tests for the settings schema.
//!
//! The unit tests in `src/config` cover the pieces. These cover the promise a
//! user relies on: that a setting the documentation describes is a setting the
//! parser accepts, that a mistake is reported rather than ignored, and that
//! converting to TOML and reading it back changes nothing.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use ratterm::config::schema::{self, Classified};
use ratterm::config::{Config, Problem, toml_file, validate};

/// Keys in the documentation that describe a syntax rather than name a setting.
///
/// `action = modifier+key` tells the reader how a keybinding line is written.
/// Listed explicitly, and kept short, so a real setting cannot hide here.
const TEMPLATE_KEYS: &[&str] = &["action"];

/// Writes a settings file in the temp directory and returns its path.
fn write_settings(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ratterm-{name}.ratrc"));
    std::fs::write(&path, body).expect("write the settings file");
    path
}

#[test]
fn a_documented_setting_is_one_the_parser_accepts() {
    // Every `key = value` line in the .ratrc documentation, commented or not,
    // must name a setting this build understands. A documented setting that
    // does nothing is worse than an undocumented one.
    let docs = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/ratrc_docs.md"),
    )
    .expect("the settings documentation");

    let mut unknown: Vec<String> = Vec::new();
    let mut inside_code_block = false;

    for line in docs.lines() {
        let text = line.trim();
        if text.starts_with("```") {
            inside_code_block = !inside_code_block;
            continue;
        }
        if !inside_code_block {
            continue;
        }

        let text = text.strip_prefix("# ").unwrap_or(text);
        let Some((key, _)) = text.split_once(" = ") else {
            continue;
        };
        let key = key.trim();

        // Skip prose, shell lines and TOML section keys.
        if key.contains(' ') || key.starts_with('#') || key.is_empty() {
            continue;
        }

        // Skip the syntax templates, which describe a shape rather than name a
        // setting: `action = modifier+key` says how a keybinding line is
        // written, and there is no setting called `action`.
        if TEMPLATE_KEYS.contains(&key) {
            continue;
        }

        if matches!(schema::classify(key), Classified::Unknown) {
            unknown.push(key.to_string());
        }
    }

    unknown.sort_unstable();
    unknown.dedup();
    assert!(
        unknown.is_empty(),
        "documented settings this build does not accept: {unknown:?}"
    );
}

#[test]
fn a_settings_file_with_a_typo_reports_it_and_still_loads() {
    let path = write_settings(
        "typo",
        "mode = emacs\nmetrics_hisory = true\nshell = bash\n",
    );

    let config = Config::load_from(&path).expect("a config still loads");

    // The settings around the mistake are applied.
    assert_eq!(config.mode, ratterm::config::KeybindingMode::Emacs);

    let issues = config.issues();
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].line, 2);
    assert_eq!(
        issues[0].problem,
        Problem::UnknownKey {
            suggestion: Some("metrics_history".to_string())
        }
    );

    let summary = config.issue_summary().expect("a summary");
    assert!(summary.contains("metrics_history"), "{summary}");

    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_clean_settings_file_has_no_summary_to_show() {
    let path = write_settings("clean", "mode = vim\nmetrics_history = true\n");
    let config = Config::load_from(&path).expect("a config");

    assert!(config.issues().is_empty());
    assert_eq!(
        config.issue_summary(),
        None,
        "nothing to say, so say nothing"
    );

    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn several_problems_are_summarised_as_one_line_with_a_count() {
    let path = write_settings("several", "nonsense = 1\nmode = vi\nmetrics_raw_days = 0\n");
    let config = Config::load_from(&path).expect("a config");

    assert_eq!(config.issues().len(), 3);
    let summary = config.issue_summary().expect("a summary");
    assert!(summary.contains("+2 more"), "{summary}");

    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_missing_settings_file_is_created_and_validates_clean() {
    let path = std::env::temp_dir().join("ratterm-created.ratrc");
    let _ = std::fs::remove_file(&path);

    let config = Config::load_from(&path).expect("a config is created");
    assert!(path.exists(), "the file should have been written");
    assert!(
        config.issues().is_empty(),
        "a fresh install must not start with a warning: {:#?}",
        config.issues()
    );

    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_file_converted_to_toml_and_read_back_means_the_same_thing() {
    let original = "mode = emacs\n\
                    shell = bash\n\
                    ide_always = true\n\
                    metrics_history = true\n\
                    metrics_raw_days = 7\n\
                    alert.cpu = 88\n\
                    alert.temperature = 79\n\
                    log_level = debug\n\
                    log_retention = 48\n\
                    git_gutter = false\n\
                    lsp-format-on-save = true\n";

    let ratrc_path = write_settings("roundtrip", original);
    let from_ratrc = Config::load_from(&ratrc_path).expect("a config");

    let toml_text = toml_file::render(original);
    let settings = toml_file::parse(&toml_text).expect("valid TOML");
    assert!(settings.issues().is_empty(), "{:#?}", settings.issues());

    let toml_path = write_settings("roundtrip-back", &settings.to_ratrc());
    let from_toml = Config::load_from(&toml_path).expect("a config");

    assert_eq!(from_toml.mode, from_ratrc.mode);
    assert_eq!(from_toml.shell, from_ratrc.shell);
    assert_eq!(from_toml.ide_always, from_ratrc.ide_always);
    assert_eq!(from_toml.metrics_history, from_ratrc.metrics_history);
    assert_eq!(from_toml.metrics_raw_days, from_ratrc.metrics_raw_days);
    assert_eq!(from_toml.alerts, from_ratrc.alerts);
    assert_eq!(from_toml.git_gutter, from_ratrc.git_gutter);
    assert_eq!(from_toml.lsp_format_on_save, from_ratrc.lsp_format_on_save);
    assert_eq!(from_toml.log_config.level, from_ratrc.log_config.level);
    assert_eq!(
        from_toml.log_config.retention_hours,
        from_ratrc.log_config.retention_hours
    );

    std::fs::remove_file(&ratrc_path).expect("clean up");
    std::fs::remove_file(&toml_path).expect("clean up");
}

#[test]
fn every_setting_in_the_schema_can_be_written_and_read_back() {
    // Guards against a setting added to the table with a TOML name that has no
    // home, which would silently drop it on conversion.
    for setting in schema::SETTINGS {
        let canonical = toml_file::canonical_key(setting.group, setting.toml_key);
        assert_eq!(
            canonical.as_deref(),
            Some(setting.key),
            "`{}` does not survive the section round trip",
            setting.key
        );
    }
}

#[test]
fn a_documented_colour_actually_changes_the_theme() {
    // `popup.background` and a dozen others were documented, accepted by a
    // custom theme file, and silently ignored in `.ratrc`. Validation calling
    // them valid while the parser dropped them would be the worse version of
    // the same bug, so this checks the colour arrives.
    let path = write_settings(
        "colours",
        "popup.background = #112233\n\
         filebrowser.directory = #445566\n\
         statusbar.mode_normal = #778899\n\
         terminal.background = #aabbcc\n",
    );

    let config = Config::load_from(&path).expect("a config");
    assert!(config.issues().is_empty(), "{:#?}", config.issues());

    let theme = config.theme().current();
    assert_eq!(
        theme.popup.background,
        ratatui::style::Color::Rgb(0x11, 0x22, 0x33)
    );
    assert_eq!(
        theme.file_browser.directory,
        ratatui::style::Color::Rgb(0x44, 0x55, 0x66)
    );
    assert_eq!(
        theme.statusbar.mode_normal,
        ratatui::style::Color::Rgb(0x77, 0x88, 0x99)
    );
    assert_eq!(
        theme.terminal.background,
        ratatui::style::Color::Rgb(0xaa, 0xbb, 0xcc),
        "the colours that always worked still do"
    );

    std::fs::remove_file(&path).expect("clean up");
}

#[test]
fn every_colour_the_theme_layer_knows_is_accepted_by_the_settings_file() {
    // The schema and the theme layer read the same list; this pins that they
    // are in fact the same list rather than two that happen to agree today.
    for key in ratterm::theme::custom::COLOR_KEYS {
        assert!(
            !matches!(schema::classify(key), Classified::Unknown),
            "`{key}` is a theme colour the settings schema does not know"
        );
    }
}

#[test]
fn a_repeated_setting_is_reported_because_one_of_them_does_nothing() {
    let issues = validate::validate("mode = vim\nmode = emacs\n");
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0].problem, Problem::Repeated { .. }));
}

#[test]
fn validation_never_refuses_to_load() {
    // A file made entirely of mistakes must still start ratterm: refusing over
    // an unrecognised key would strand anyone who downgraded a version.
    let path = write_settings(
        "all-wrong",
        "aaa = 1\nbbb = 2\nmode = nonsense\nalert.cpu = -5\n",
    );

    let config = Config::load_from(&path).expect("it still loads");
    assert_eq!(config.issues().len(), 4, "{:#?}", config.issues());
    assert_eq!(
        config.mode,
        ratterm::config::KeybindingMode::Default,
        "an unreadable mode falls back rather than failing"
    );

    std::fs::remove_file(&path).expect("clean up");
}
