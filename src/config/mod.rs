//! Configuration module for ratatui-full-ide.
//!
//! Handles loading and parsing the .ratrc configuration file.

mod keybindings;
pub mod platform;
pub mod schema;
pub mod shell;
pub mod toml_file;
pub mod validate;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tracing::warn;

pub use keybindings::{KeyAction, KeyBinding, KeybindingMode, Keybindings};
pub use platform::{PlatformKeys, command_palette_hotkey, is_windows_11};
pub use schema::{Group, Setting, ValueKind};
pub use shell::{ShellDetector, ShellInfo, ShellInstallInfo, ShellInstaller, ShellType};
pub use validate::{Issue, Problem};

use crate::docker_logs::config::LogStreamConfig;
use crate::logging::LogConfig;
use crate::ssh::StorageMode;
use crate::telemetry::AlertSettings;
use crate::theme::{ThemeManager, ThemeSettings};

/// Days of raw metric samples kept before they are averaged per minute.
const DEFAULT_METRICS_RAW_DAYS: u32 = 1;

/// Most raw days accepted from the config file.
///
/// A raw sample every five seconds is about 17,000 rows per host per day, so a
/// year of raw data is a database nobody asked for. Beyond this the answer is
/// to raise `metrics_raw_days` deliberately in code, not by typo.
const MAX_METRICS_RAW_DAYS: u32 = 90;

/// Default .ratrc file content with all commands documented.
const DEFAULT_RATRC: &str = r#"# Ratatui Full IDE Configuration File
# =====================================
# This file is read on application startup.
# Lines starting with '#' are comments.
#
# Shell Configuration
# -------------------
# Set the preferred shell: powershell, bash, cmd, zsh, fish, or system
# Windows: powershell (default), bash (requires Git Bash), cmd
# Linux: bash (default), zsh, fish, powershell (requires PowerShell Core)
# macOS: zsh (default), bash, fish, powershell (requires PowerShell Core)
# shell = system
shell = system

# Auto-close tabs when changing shell (true/false)
# When enabled, all existing terminal tabs are closed when you select a new shell
# auto_close_tabs_on_shell_change = false

# IDE Configuration
# -----------------
# Set to true to always show the IDE pane (editor) alongside terminals
# When false (default), only terminals are shown until 'open' command or Ctrl+I
# ide-always = false

# Keybinding Mode
# ---------------
# Set the keybinding mode: vim, emacs, or default
# mode = default
mode = vim

# Global Keybindings
# ------------------
# Format: action = modifier+key
# Modifiers: ctrl, alt, shift (combine with +)
#
# quit                  = ctrl+q           # Quit the application
# focus_terminal        = alt+left         # Focus terminal pane
# focus_editor          = alt+right        # Focus editor pane
# toggle_focus          = alt+tab          # Toggle focus between panes
# split_left            = alt+[            # Move split divider left
# split_right           = alt+]            # Move split divider right

# File Browser
# ------------
# open_file_browser     = ctrl+o           # Open file browser
# next_file             = alt+shift+right  # Switch to next open file
# prev_file             = alt+shift+left   # Switch to previous open file

# Search & Create
# ---------------
# find_in_file          = ctrl+f           # Find in current file
# find_in_files         = ctrl+shift+f     # Find in all files
# search_directories    = ctrl+shift+d     # Search for directories
# search_files          = ctrl+shift+e     # Search for files
# new_file              = ctrl+n           # Create new file
# new_folder            = ctrl+shift+n     # Create new folder

# Clipboard
# ---------
# copy                  = ctrl+shift+c     # Copy selection or line
# paste                 = ctrl+v           # Paste from clipboard

# Terminal
# --------
# terminal_new_tab      = ctrl+t           # New terminal tab
# terminal_split        = ctrl+s           # Split terminal horizontally
# terminal_next_tab     = ctrl+right       # Next terminal tab
# terminal_prev_tab     = ctrl+left        # Previous terminal tab
# terminal_close_tab    = ctrl+w           # Close current terminal tab
# terminal_interrupt    = ctrl+c           # Send interrupt (Ctrl+C)
# terminal_scroll_up    = shift+pageup     # Scroll terminal up
# terminal_scroll_down  = shift+pagedown   # Scroll terminal down

# Editor (Normal Mode - Vim)
# --------------------------
# editor_insert         = i                # Enter insert mode
# editor_append         = a                # Append after cursor
# editor_visual         = v                # Enter visual mode
# editor_command        = :                # Enter command mode
# editor_left           = h                # Move cursor left
# editor_right          = l                # Move cursor right
# editor_up             = k                # Move cursor up
# editor_down           = j                # Move cursor down
# editor_line_start     = 0                # Move to line start
# editor_line_end       = $                # Move to line end
# editor_word_right     = w                # Move to next word
# editor_word_left      = b                # Move to previous word
# editor_buffer_start   = g                # Move to buffer start
# editor_buffer_end     = G                # Move to buffer end
# editor_delete         = x                # Delete character
# editor_undo           = u                # Undo
# editor_redo           = ctrl+r           # Redo
# editor_save           = ctrl+s           # Save file

# Extension/Addon Hotkeys
# -----------------------
# Format: addon.<name> = <hotkey>|<command>
# Bind a hotkey to launch an extension or external command in a new terminal tab.
# The command will run in the configured shell.
#
# Examples:
# addon.my-tool = f3|/path/to/my-tool
# addon.rat-squad = f2|~/.ratterm/extensions/rat-squad/rat-squad

# Window Positions
# ----------------
# Control where popups and overlays appear on screen.
# Grid positions: top-left, top-center, top-right,
#                 middle-left, middle-center, middle-right,
#                 bottom-left, bottom-center, bottom-right
# Pixel offsets: "X x Y" (cell distance from top-left corner)
#
# hotkey_overlay_position = middle-center
# command_palette_position = top-center
# ssh_manager_position = middle-center
# docker_manager_position = middle-center
# health_dashboard_position = middle-center

# LSP (Language Server Protocol)
# ------------------------------
# Override default language servers:
# lsp-rust = rust-analyzer
# lsp-python = pyright
#
# Format file on save via LSP:
# lsp-format-on-save = false

# Logging Configuration
# ---------------------
# Logs are stored in ~/.ratterm/logs/ with automatic cleanup.
#
# log_enabled = true       # Enable/disable file logging (true/false)
# log_level = info         # Log level: trace, debug, info, warn, error, off
# log_retention = 24       # Hours to keep log files (default: 24)

# Fleet Metrics
# -------------
# Health dashboard samples are kept in memory. Turn history on to also write
# them to ~/.ratterm/metrics.db, which survives a restart and is what the
# sparkline and the "offline since" column read from.
#
# metrics_history = false  # Keep metric history on disk (true/false)
# metrics_raw_days = 1     # Days of raw samples before per-minute averaging
#
# Alert thresholds. A sample crossing one is recorded against the host and
# shown in the dashboard. Leave a line out, or set it to 0, for no rule.
#
# alert.cpu = 90           # Percent
# alert.memory = 85        # Percent
# alert.disk = 90          # Percent
# alert.temperature = 85   # Degrees Celsius
"#;

/// Addon/extension command configuration.
#[derive(Debug, Clone)]
pub struct AddonCommand {
    /// Name of the addon.
    pub name: String,
    /// Command to execute.
    pub command: String,
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Keybinding mode (vim, emacs, default).
    pub mode: KeybindingMode,
    /// Preferred shell type.
    pub shell: ShellType,
    /// Custom keybindings.
    pub keybindings: Keybindings,
    /// Path to config file.
    pub config_path: PathBuf,
    /// Auto-close existing tabs when changing shell.
    pub auto_close_tabs_on_shell_change: bool,
    /// Theme manager for UI customization.
    pub theme_manager: ThemeManager,
    /// Whether to always show the IDE pane (false = terminal-first mode).
    pub ide_always: bool,
    /// SSH credential storage mode.
    pub ssh_storage_mode: StorageMode,
    /// SSH quick connect hotkey prefix (e.g., "ctrl", "ctrl+shift").
    pub set_ssh_tab: String,
    /// Enable SSH quick connect with numbers (set_ssh_tab + 1-9).
    pub ssh_number_setting: bool,
    /// Addon hotkey bindings (keybinding -> command).
    pub addon_commands: HashMap<KeyBinding, AddonCommand>,
    /// Logging configuration.
    pub log_config: LogConfig,
    /// Docker log streaming configuration.
    pub docker_log_config: LogStreamConfig,
    /// Window position overrides for specific popups/overlays.
    pub window_positions: HashMap<String, crate::ui::window_position::WindowPosition>,
    /// Enable git gutter indicators in the editor.
    pub git_gutter: bool,
    /// Enable git blame view.
    pub git_blame: bool,
    /// LSP server override for Rust (e.g., "rust-analyzer").
    pub lsp_rust: Option<String>,
    /// LSP server override for Python (e.g., "pyright", "pylsp").
    pub lsp_python: Option<String>,
    /// Format file on save via LSP.
    pub lsp_format_on_save: bool,
    /// Alert thresholds evaluated on every metric sample.
    pub alerts: AlertSettings,
    /// Keep a durable metric history on disk.
    ///
    /// Off means the dashboard still works, from memory, and nothing is
    /// written. That is the right default for a machine whose owner did not
    /// ask for a database to appear in their home directory.
    pub metrics_history: bool,
    /// How many days of raw samples to keep before averaging them per minute.
    pub metrics_raw_days: u32,
    /// Problems found while reading the configuration file.
    ///
    /// Kept rather than only logged so the interface can say so: a warning in
    /// a log file nobody opens is the same as no warning.
    issues: Vec<Issue>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: KeybindingMode::Vim,
            shell: ShellType::System,
            keybindings: Keybindings::default(),
            config_path: Self::default_config_path(),
            auto_close_tabs_on_shell_change: false,
            theme_manager: ThemeManager::default(),
            ide_always: false, // Terminal-first by default
            // The OS keychain, not the host file. Plaintext is now opt-in.
            ssh_storage_mode: StorageMode::default(),
            set_ssh_tab: "ctrl".to_string(),
            ssh_number_setting: true,
            addon_commands: HashMap::new(),
            log_config: LogConfig::default(),
            docker_log_config: LogStreamConfig::default(),
            window_positions: HashMap::new(),
            git_gutter: true,
            git_blame: true,
            lsp_rust: None,
            lsp_python: None,
            lsp_format_on_save: false,
            alerts: AlertSettings::default(),
            metrics_history: false,
            metrics_raw_days: DEFAULT_METRICS_RAW_DAYS,
            issues: Vec::new(),
        }
    }
}

impl Config {
    /// Returns the default config file path (~/.ratrc).
    #[must_use]
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ratrc")
    }

    /// Loads configuration, preferring the consolidated TOML file.
    ///
    /// `~/.ratterm/config.toml` wins when it exists, because writing one is a
    /// deliberate act; otherwise `~/.ratrc` is used and created if missing.
    /// A TOML file that cannot be read is reported and skipped rather than
    /// being a reason to refuse to start.
    ///
    /// # Errors
    /// Returns error if config cannot be read or parsed.
    pub fn load() -> io::Result<Self> {
        let toml_path = toml_file::default_path();
        if toml_path.exists() {
            match toml_file::load(&toml_path) {
                Ok(settings) => {
                    let mut config = Self::from_content(&settings.to_ratrc(), &toml_path);
                    for (key, _) in &settings.unrecognised {
                        warn!("{}: `{key}` is not a setting this build knows", toml_path.display());
                    }
                    config.issues = settings.issues();
                    config.report_issues();
                    return Ok(config);
                }
                Err(e) => warn!("{e}"),
            }
        }

        Self::load_from(&Self::default_config_path())
    }

    /// Loads configuration from a specific path.
    ///
    /// # Errors
    /// Returns error if config cannot be read or parsed.
    pub fn load_from(path: &PathBuf) -> io::Result<Self> {
        // Create default config if it doesn't exist
        if !path.exists() {
            Self::create_default_config(path)?;
        }

        let content = fs::read_to_string(path)?;
        let mut config = Self::from_content(&content, path);
        config.issues = validate::validate(&content);
        config.report_issues();
        Ok(config)
    }

    /// Builds a configuration from file content already in hand.
    fn from_content(content: &str, path: &Path) -> Self {
        let mut config = Self {
            config_path: path.to_path_buf(),
            ..Self::default()
        };
        config.parse(content);

        // Re-initialize keybindings based on parsed mode
        config.keybindings = Keybindings::for_mode(config.mode);

        // Re-parse to apply any custom keybinding overrides
        config.parse_keybindings(content);

        // Parse and apply theme settings
        let theme_settings = ThemeSettings::parse(content);
        theme_settings.apply_to_manager(&mut config.theme_manager);

        config
    }

    /// Writes any configuration problems to the log.
    ///
    /// A setting that does nothing and says nothing is worse than one that
    /// fails, so these are never silent — but they are never fatal either,
    /// since an unrecognised key may simply belong to a newer build.
    fn report_issues(&self) {
        for issue in &self.issues {
            warn!("{}: {issue}", self.config_path.display());
        }
    }

    /// Problems found in the configuration file, in the order they appear.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// A one-line summary of the configuration problems, if there are any.
    ///
    /// Shown in the status bar at start-up: a warning in a log file nobody
    /// opens is the same as no warning.
    #[must_use]
    pub fn issue_summary(&self) -> Option<String> {
        let first = self.issues.first()?;
        Some(if self.issues.len() == 1 {
            format!("{}: {first}", self.config_name())
        } else {
            format!(
                "{}: {first} (+{} more)",
                self.config_name(),
                self.issues.len() - 1
            )
        })
    }

    /// The configuration file's name, without its directory.
    fn config_name(&self) -> String {
        self.config_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "config".to_string())
    }

    /// Writes the settings as `~/.ratterm/config.toml`.
    ///
    /// Returns the path written. The `.ratrc` file is left alone: a migration
    /// that deletes the file it read from is one the user cannot undo.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn migrate_to_toml(&self) -> io::Result<PathBuf> {
        let content = fs::read_to_string(&self.config_path)?;
        let path = toml_file::default_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, toml_file::render(&content))?;

        Ok(path)
    }

    /// Parses only keybinding settings from content.
    fn parse_keybindings(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key = value (only keybindings, not mode)
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Remove inline comments
                let value = value.split('#').next().unwrap_or(value).trim();

                // Only apply keybinding settings (not mode)
                if key != "mode"
                    && let Some(action) = KeyAction::parse_action(key)
                    && let Some(binding) = KeyBinding::parse(value)
                {
                    self.keybindings.set(action, binding);
                }
            }
        }
    }

    /// Creates the default config file.
    fn create_default_config(path: &PathBuf) -> io::Result<()> {
        let mut file = fs::File::create(path)?;
        file.write_all(DEFAULT_RATRC.as_bytes())?;
        Ok(())
    }

    /// Parses the config file content.
    fn parse(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key = value
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Remove inline comments
                let value = value.split('#').next().unwrap_or(value).trim();

                self.apply_setting(key, value);
            }
        }
    }

    /// Applies a single setting.
    fn apply_setting(&mut self, key: &str, value: &str) {
        match key {
            "mode" => {
                self.mode = match value.to_lowercase().as_str() {
                    "vim" => KeybindingMode::Vim,
                    "emacs" => KeybindingMode::Emacs,
                    _ => KeybindingMode::Default,
                };
            }
            "shell" => {
                self.shell = match value.to_lowercase().as_str() {
                    "powershell" | "pwsh" | "ps" => ShellType::PowerShell,
                    "bash" => ShellType::Bash,
                    "cmd" | "command" => ShellType::Cmd,
                    "zsh" => ShellType::Zsh,
                    "fish" => ShellType::Fish,
                    _ => ShellType::System,
                };
            }
            "auto_close_tabs_on_shell_change" => {
                self.auto_close_tabs_on_shell_change =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "ide_always" | "ide-always" => {
                self.ide_always =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "git_gutter" | "git-gutter" => {
                self.git_gutter =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "git_blame" | "git-blame" => {
                self.git_blame =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "lsp-rust" | "lsp_rust" => {
                self.lsp_rust = Some(value.to_string());
            }
            "lsp-python" | "lsp_python" => {
                self.lsp_python = Some(value.to_string());
            }
            "lsp-format-on-save" | "lsp_format_on_save" => {
                self.lsp_format_on_save =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "ssh_storage_mode" => {
                self.ssh_storage_mode = StorageMode::parse(value);
            }
            "set_ssh_tab" => {
                // Store the prefix for SSH quick connect (e.g., "ctrl", "ctrl+shift")
                self.set_ssh_tab = value.to_lowercase();
            }
            "ssh_number_setting" => {
                self.ssh_number_setting =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "log_level" => {
                self.log_config.level = LogConfig::parse_level(value);
            }
            "log_retention" | "log_retention_hours" => {
                self.log_config.retention_hours = LogConfig::parse_retention(value);
            }
            "metrics_history" | "metrics-history" => {
                self.metrics_history =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            "metrics_raw_days" | "metrics-raw-days" => {
                self.metrics_raw_days = value
                    .parse::<u32>()
                    .ok()
                    .filter(|days| *days > 0 && *days <= MAX_METRICS_RAW_DAYS)
                    .unwrap_or(DEFAULT_METRICS_RAW_DAYS);
            }
            k if k.starts_with("alert.") => {
                if !self.alerts.apply(k, value) {
                    tracing::warn!("unknown alert setting '{}'", k);
                }
            }
            "log_enabled" | "logging" => {
                self.log_config.enabled =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            // Docker log streaming settings
            k if k.starts_with("docker_log_") => {
                self.docker_log_config.apply_setting(key, value);
            }
            _ => {
                // Check for window position settings: *_position = <value>
                if key.ends_with("_position") {
                    match crate::ui::window_position::WindowPosition::parse(value) {
                        Ok(pos) => {
                            self.window_positions.insert(key.to_string(), pos);
                        }
                        Err(e) => {
                            tracing::warn!("Invalid window position '{}': {}", value, e);
                        }
                    }
                // Check for addon.* pattern: addon.<name> = <hotkey>|<command>
                } else if let Some(addon_name) = key.strip_prefix("addon.") {
                    if let Some((hotkey, command)) = value.split_once('|') {
                        let hotkey = hotkey.trim();
                        let command = command.trim();
                        if let Some(binding) = KeyBinding::parse(hotkey) {
                            self.addon_commands.insert(
                                binding,
                                AddonCommand {
                                    name: addon_name.to_string(),
                                    command: command.to_string(),
                                },
                            );
                        }
                    }
                } else if let Some(action) = KeyAction::parse_action(key) {
                    // Try to parse as keybinding
                    if let Some(binding) = KeyBinding::parse(value) {
                        self.keybindings.set(action, binding);
                    }
                }
            }
        }
    }

    /// Returns the configured position for a window, or the default.
    ///
    /// Looks up `{name}_position` in the window_positions map.
    #[must_use]
    pub fn window_position(&self, name: &str) -> crate::ui::window_position::WindowPosition {
        self.window_positions
            .get(&format!("{name}_position"))
            .cloned()
            .unwrap_or_default()
    }

    /// Reloads the configuration from disk.
    ///
    /// # Errors
    /// Returns error if config cannot be read.
    pub fn reload(&mut self) -> io::Result<()> {
        let path = self.config_path.clone();
        let new_config = Self::load_from(&path)?;
        *self = new_config;
        Ok(())
    }

    /// Saves a single setting to the config file.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn save_setting(&self, key: &str, value: &str) -> io::Result<()> {
        crate::theme::save_setting(&self.config_path, key, value)
    }

    /// Saves the current theme preset to the config file.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn save_theme(&self) -> io::Result<()> {
        if let Some(preset) = self.theme_manager.current_preset() {
            crate::theme::save_theme_preset(&self.config_path, preset)
        } else {
            Ok(())
        }
    }

    /// Saves a color setting to the config file.
    ///
    /// # Errors
    /// Returns error if file cannot be written.
    pub fn save_color(&self, key: &str, color: ratatui::style::Color) -> io::Result<()> {
        crate::theme::save_color_setting(&self.config_path, key, color)
    }

    /// Returns a reference to the theme manager.
    #[must_use]
    pub fn theme(&self) -> &ThemeManager {
        &self.theme_manager
    }

    /// Returns a mutable reference to the theme manager.
    pub fn theme_mut(&mut self) -> &mut ThemeManager {
        &mut self.theme_manager
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod telemetry_settings_tests {
    use super::*;

    /// Parses a config body without touching the user's real `.ratrc`.
    fn parse(body: &str) -> Config {
        let mut config = Config::default();
        config.parse(body);
        config
    }

    #[test]
    fn metric_history_is_off_unless_asked_for() {
        let config = Config::default();
        assert!(
            !config.metrics_history,
            "a database should not appear without being asked for"
        );
        assert_eq!(config.metrics_raw_days, DEFAULT_METRICS_RAW_DAYS);
        assert!(config.alerts.is_empty());
    }

    #[test]
    fn metric_history_can_be_turned_on() {
        for value in ["true", "yes", "1", "on", "ON"] {
            let config = parse(&format!("metrics_history = {value}\n"));
            assert!(config.metrics_history, "value was {value}");
        }
    }

    #[test]
    fn the_dashed_spelling_works_too() {
        let config = parse("metrics-history = true\nmetrics-raw-days = 7\n");
        assert!(config.metrics_history);
        assert_eq!(config.metrics_raw_days, 7);
    }

    #[test]
    fn alert_lines_reach_the_settings() {
        let config = parse("alert.cpu = 90\nalert.memory = 85\nalert.temperature = 80\n");
        assert_eq!(config.alerts.cpu_percent, Some(90.0));
        assert_eq!(config.alerts.memory_percent, Some(85.0));
        assert_eq!(config.alerts.temperature_c, Some(80.0));
        assert_eq!(config.alerts.to_rules().len(), 3);
    }

    #[test]
    fn an_inline_comment_does_not_become_part_of_the_threshold() {
        let config = parse("alert.cpu = 90   # shout at me\n");
        assert_eq!(config.alerts.cpu_percent, Some(90.0));
    }

    #[test]
    fn an_out_of_range_raw_window_falls_back_to_the_default() {
        for body in [
            "metrics_raw_days = 0",
            "metrics_raw_days = 4000",
            "metrics_raw_days = lots",
        ] {
            let config = parse(body);
            assert_eq!(
                config.metrics_raw_days, DEFAULT_METRICS_RAW_DAYS,
                "body was {body}"
            );
        }
    }

    #[test]
    fn the_largest_accepted_raw_window_is_kept() {
        let config = parse(&format!("metrics_raw_days = {MAX_METRICS_RAW_DAYS}\n"));
        assert_eq!(config.metrics_raw_days, MAX_METRICS_RAW_DAYS);
    }

    #[test]
    fn an_unknown_alert_key_does_not_disturb_the_rest_of_the_file() {
        let config = parse("alert.gpu = 90\nalert.cpu = 70\nmode = emacs\n");
        assert_eq!(config.alerts.cpu_percent, Some(70.0));
        assert_eq!(config.mode, KeybindingMode::Emacs);
    }

    #[test]
    fn the_shipped_default_file_documents_the_new_settings() {
        // A setting nobody can discover may as well not exist.
        for key in [
            "metrics_history",
            "metrics_raw_days",
            "alert.cpu",
            "alert.memory",
            "alert.disk",
            "alert.temperature",
        ] {
            assert!(DEFAULT_RATRC.contains(key), "{key} is undocumented");
        }
    }

    #[test]
    fn the_documented_defaults_parse_as_written() {
        // Every commented line in the shipped file should be valid if
        // uncommented; otherwise the documentation teaches a syntax error.
        let uncommented: String = DEFAULT_RATRC
            .lines()
            .filter_map(|line| line.strip_prefix("# "))
            .filter(|line| line.contains(" = ") && !line.contains("  #"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = parse(&uncommented);
        assert!(!config.metrics_history, "documented default is false");
    }
}
