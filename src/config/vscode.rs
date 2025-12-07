//! VSCode settings.json parser and importer.
//!
//! Reads VSCode settings from the user's settings.json and applies
//! compatible settings to the editor.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// VSCode settings that can be imported.
#[derive(Debug, Clone, Default)]
pub struct VsCodeSettings {
    /// Tab size (number of spaces).
    pub tab_size: Option<u8>,
    /// Use spaces instead of tabs.
    pub insert_spaces: Option<bool>,
    /// Word wrap mode.
    pub word_wrap: Option<WordWrapMode>,
    /// Auto-save mode.
    pub auto_save: Option<AutoSaveMode>,
    /// Show line numbers.
    pub line_numbers: Option<LineNumbersMode>,
    /// Cursor style.
    pub cursor_style: Option<CursorStyle>,
    /// Render whitespace.
    pub render_whitespace: Option<RenderWhitespace>,
    /// Font size.
    pub font_size: Option<u16>,
    /// Enable minimap.
    pub minimap_enabled: Option<bool>,
    /// Bracket pair colorization.
    pub bracket_pair_colorization: Option<bool>,
    /// Auto-closing brackets.
    pub auto_closing_brackets: Option<AutoClosingMode>,
    /// Auto-closing quotes.
    pub auto_closing_quotes: Option<AutoClosingMode>,
    /// Format on save.
    pub format_on_save: Option<bool>,
    /// Trim trailing whitespace.
    pub trim_trailing_whitespace: Option<bool>,
    /// Insert final newline.
    pub insert_final_newline: Option<bool>,
    /// Custom keybindings (from keybindings.json).
    pub keybindings: Vec<VsCodeKeybinding>,
}

/// Word wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrapMode {
    Off,
    On,
    WordWrapColumn,
    Bounded,
}

/// Auto-save mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSaveMode {
    Off,
    AfterDelay,
    OnFocusChange,
    OnWindowChange,
}

/// Line numbers mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumbersMode {
    Off,
    On,
    Relative,
    Interval,
}

/// Cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Line,
    Block,
    Underline,
    LineThin,
    BlockOutline,
    UnderlineThin,
}

/// Render whitespace mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderWhitespace {
    None,
    Boundary,
    Selection,
    Trailing,
    All,
}

/// Auto-closing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosingMode {
    Always,
    LanguageDefined,
    BeforeWhitespace,
    Never,
}

/// A VSCode keybinding.
#[derive(Debug, Clone)]
pub struct VsCodeKeybinding {
    /// The key combination.
    pub key: String,
    /// The command to execute.
    pub command: String,
    /// Optional when clause.
    pub when: Option<String>,
}

impl VsCodeSettings {
    /// Loads VSCode settings from the default location.
    ///
    /// Searches for settings.json in platform-specific locations:
    /// - Windows: %APPDATA%\Code\User\settings.json
    /// - macOS: ~/Library/Application Support/Code/User/settings.json
    /// - Linux: ~/.config/Code/User/settings.json
    pub fn load() -> Option<Self> {
        let settings_path = Self::find_settings_path()?;
        Self::load_from_path(&settings_path)
    }

    /// Loads VSCode settings from a specific path.
    pub fn load_from_path(path: &PathBuf) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        Self::parse(&content)
    }

    /// Parses VSCode settings from JSON content.
    pub fn parse(content: &str) -> Option<Self> {
        // VSCode settings.json can have comments, so we need to strip them
        let clean_content = strip_json_comments(content);
        let json: Value = serde_json::from_str(&clean_content).ok()?;

        let mut settings = Self::default();

        if let Value::Object(map) = json {
            for (key, value) in map {
                settings.apply_setting(&key, &value);
            }
        }

        Some(settings)
    }

    /// Applies a single setting from JSON.
    fn apply_setting(&mut self, key: &str, value: &Value) {
        match key {
            "editor.tabSize" => {
                if let Value::Number(n) = value {
                    self.tab_size = n.as_u64().map(|v| v as u8);
                }
            }
            "editor.insertSpaces" => {
                if let Value::Bool(b) = value {
                    self.insert_spaces = Some(*b);
                }
            }
            "editor.wordWrap" => {
                if let Value::String(s) = value {
                    self.word_wrap = Some(match s.as_str() {
                        "off" => WordWrapMode::Off,
                        "on" => WordWrapMode::On,
                        "wordWrapColumn" => WordWrapMode::WordWrapColumn,
                        "bounded" => WordWrapMode::Bounded,
                        _ => WordWrapMode::Off,
                    });
                }
            }
            "files.autoSave" => {
                if let Value::String(s) = value {
                    self.auto_save = Some(match s.as_str() {
                        "off" => AutoSaveMode::Off,
                        "afterDelay" => AutoSaveMode::AfterDelay,
                        "onFocusChange" => AutoSaveMode::OnFocusChange,
                        "onWindowChange" => AutoSaveMode::OnWindowChange,
                        _ => AutoSaveMode::Off,
                    });
                }
            }
            "editor.lineNumbers" => {
                if let Value::String(s) = value {
                    self.line_numbers = Some(match s.as_str() {
                        "off" => LineNumbersMode::Off,
                        "on" => LineNumbersMode::On,
                        "relative" => LineNumbersMode::Relative,
                        "interval" => LineNumbersMode::Interval,
                        _ => LineNumbersMode::On,
                    });
                }
            }
            "editor.cursorStyle" => {
                if let Value::String(s) = value {
                    self.cursor_style = Some(match s.as_str() {
                        "line" => CursorStyle::Line,
                        "block" => CursorStyle::Block,
                        "underline" => CursorStyle::Underline,
                        "line-thin" => CursorStyle::LineThin,
                        "block-outline" => CursorStyle::BlockOutline,
                        "underline-thin" => CursorStyle::UnderlineThin,
                        _ => CursorStyle::Line,
                    });
                }
            }
            "editor.renderWhitespace" => {
                if let Value::String(s) = value {
                    self.render_whitespace = Some(match s.as_str() {
                        "none" => RenderWhitespace::None,
                        "boundary" => RenderWhitespace::Boundary,
                        "selection" => RenderWhitespace::Selection,
                        "trailing" => RenderWhitespace::Trailing,
                        "all" => RenderWhitespace::All,
                        _ => RenderWhitespace::None,
                    });
                }
            }
            "editor.fontSize" => {
                if let Value::Number(n) = value {
                    self.font_size = n.as_u64().map(|v| v as u16);
                }
            }
            "editor.minimap.enabled" => {
                if let Value::Bool(b) = value {
                    self.minimap_enabled = Some(*b);
                }
            }
            "editor.bracketPairColorization.enabled" => {
                if let Value::Bool(b) = value {
                    self.bracket_pair_colorization = Some(*b);
                }
            }
            "editor.autoClosingBrackets" => {
                if let Value::String(s) = value {
                    self.auto_closing_brackets = Some(parse_auto_closing_mode(s));
                }
            }
            "editor.autoClosingQuotes" => {
                if let Value::String(s) = value {
                    self.auto_closing_quotes = Some(parse_auto_closing_mode(s));
                }
            }
            "editor.formatOnSave" => {
                if let Value::Bool(b) = value {
                    self.format_on_save = Some(*b);
                }
            }
            "files.trimTrailingWhitespace" => {
                if let Value::Bool(b) = value {
                    self.trim_trailing_whitespace = Some(*b);
                }
            }
            "files.insertFinalNewline" => {
                if let Value::Bool(b) = value {
                    self.insert_final_newline = Some(*b);
                }
            }
            _ => {}
        }
    }

    /// Finds the VSCode settings.json path for the current platform.
    fn find_settings_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|p| p.join("Code").join("User").join("settings.json"))
        }

        #[cfg(target_os = "macos")]
        {
            dirs::home_dir().map(|p| {
                p.join("Library")
                    .join("Application Support")
                    .join("Code")
                    .join("User")
                    .join("settings.json")
            })
        }

        #[cfg(target_os = "linux")]
        {
            dirs::config_dir().map(|p| p.join("Code").join("User").join("settings.json"))
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    /// Loads keybindings from VSCode's keybindings.json.
    pub fn load_keybindings() -> Vec<VsCodeKeybinding> {
        let keybindings_path = Self::find_keybindings_path();
        if let Some(path) = keybindings_path {
            if let Ok(content) = fs::read_to_string(&path) {
                return Self::parse_keybindings(&content);
            }
        }
        Vec::new()
    }

    /// Finds the VSCode keybindings.json path.
    fn find_keybindings_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|p| p.join("Code").join("User").join("keybindings.json"))
        }

        #[cfg(target_os = "macos")]
        {
            dirs::home_dir().map(|p| {
                p.join("Library")
                    .join("Application Support")
                    .join("Code")
                    .join("User")
                    .join("keybindings.json")
            })
        }

        #[cfg(target_os = "linux")]
        {
            dirs::config_dir().map(|p| p.join("Code").join("User").join("keybindings.json"))
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    /// Parses keybindings from JSON content.
    fn parse_keybindings(content: &str) -> Vec<VsCodeKeybinding> {
        let clean_content = strip_json_comments(content);
        let json: Value = match serde_json::from_str(&clean_content) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let mut keybindings = Vec::new();

        if let Value::Array(arr) = json {
            for item in arr {
                if let Value::Object(obj) = item {
                    let key = obj
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let command = obj
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let when = obj
                        .get("when")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    if !key.is_empty() && !command.is_empty() {
                        keybindings.push(VsCodeKeybinding { key, command, when });
                    }
                }
            }
        }

        keybindings
    }
}

/// Parses auto-closing mode from string.
fn parse_auto_closing_mode(s: &str) -> AutoClosingMode {
    match s {
        "always" => AutoClosingMode::Always,
        "languageDefined" => AutoClosingMode::LanguageDefined,
        "beforeWhitespace" => AutoClosingMode::BeforeWhitespace,
        "never" => AutoClosingMode::Never,
        _ => AutoClosingMode::LanguageDefined,
    }
}

/// Strips C-style comments from JSON (// and /* */).
fn strip_json_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                result.push(c);
            }
            continue;
        }

        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
            continue;
        }

        if in_string {
            result.push(c);
            if c == '"' {
                in_string = false;
            } else if c == '\\' {
                // Escape sequence - push next char too
                if let Some(next) = chars.next() {
                    result.push(next);
                }
            }
            continue;
        }

        if c == '/' {
            if chars.peek() == Some(&'/') {
                chars.next();
                in_line_comment = true;
                continue;
            } else if chars.peek() == Some(&'*') {
                chars.next();
                in_block_comment = true;
                continue;
            }
        }

        result.push(c);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_json_comments() {
        let input = r#"{
            // This is a comment
            "key": "value", /* inline comment */
            "key2": "value2"
        }"#;
        let result = strip_json_comments(input);
        assert!(!result.contains("//"));
        assert!(!result.contains("/*"));
        assert!(result.contains("\"key\""));
    }

    #[test]
    fn test_parse_settings() {
        let json = r#"{
            "editor.tabSize": 4,
            "editor.insertSpaces": true,
            "editor.wordWrap": "on"
        }"#;
        let settings = VsCodeSettings::parse(json);
        assert!(settings.is_some());
        let settings = settings.expect("settings parsed");
        assert_eq!(settings.tab_size, Some(4));
        assert_eq!(settings.insert_spaces, Some(true));
        assert_eq!(settings.word_wrap, Some(WordWrapMode::On));
    }

    #[test]
    fn test_parse_keybindings() {
        let json = r#"[
            {
                "key": "ctrl+shift+p",
                "command": "workbench.action.showCommands"
            },
            {
                "key": "ctrl+s",
                "command": "workbench.action.files.save",
                "when": "editorTextFocus"
            }
        ]"#;
        let keybindings = VsCodeSettings::parse_keybindings(json);
        assert_eq!(keybindings.len(), 2);
        assert_eq!(keybindings[0].key, "ctrl+shift+p");
        assert_eq!(keybindings[1].when, Some("editorTextFocus".to_string()));
    }
}
