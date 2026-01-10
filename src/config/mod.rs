//! Configuration module for ratatui-full-ide.
//!
//! Handles loading and parsing the .ratrc configuration file.

mod keybindings;
pub mod platform;
pub mod shell;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub use keybindings::{KeyAction, KeyBinding, KeybindingMode, Keybindings};
pub use platform::{command_palette_hotkey, is_windows_11};
pub use shell::{ShellDetector, ShellInfo, ShellInstallInfo, ShellInstaller, ShellType};

use crate::logging::LogConfig;
use crate::ssh::StorageMode;
use crate::theme::{ThemeManager, ThemeSettings};

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

# Logging Configuration
# ---------------------
# Logs are stored in ~/.ratterm/logs/ with automatic cleanup.
#
# log_enabled = true       # Enable/disable file logging (true/false)
# log_level = info         # Log level: trace, debug, info, warn, error, off
# log_retention = 24       # Hours to keep log files (default: 24)
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
            ssh_storage_mode: StorageMode::Plaintext,
            set_ssh_tab: "ctrl".to_string(),
            ssh_number_setting: true,
            addon_commands: HashMap::new(),
            log_config: LogConfig::default(),
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

    /// Loads configuration from the default path, creating it if it doesn't exist.
    ///
    /// # Errors
    /// Returns error if config cannot be read or parsed.
    pub fn load() -> io::Result<Self> {
        let path = Self::default_config_path();
        Self::load_from(&path)
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
        let mut config = Self {
            config_path: path.clone(),
            ..Self::default()
        };
        config.parse(&content);

        // Re-initialize keybindings based on parsed mode
        config.keybindings = Keybindings::for_mode(config.mode);

        // Re-parse to apply any custom keybinding overrides
        config.parse_keybindings(&content);

        // Parse and apply theme settings
        let theme_settings = ThemeSettings::parse(&content);
        theme_settings.apply_to_manager(&mut config.theme_manager);

        Ok(config)
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
                if key != "mode" {
                    if let Some(action) = KeyAction::parse_action(key) {
                        if let Some(binding) = KeyBinding::parse(value) {
                            self.keybindings.set(action, binding);
                        }
                    }
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
            "log_enabled" | "logging" => {
                self.log_config.enabled =
                    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on");
            }
            _ => {
                // Check for addon.* pattern: addon.<name> = <hotkey>|<command>
                if let Some(addon_name) = key.strip_prefix("addon.") {
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
