//! Custom theme loading from TOML files.
//!
//! Allows users to create and load custom themes from `~/.ratterm/themes/`.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::colors::parse_color;
use super::component::Theme;
use super::preset::ThemePreset;

/// Custom theme definition from TOML.
#[derive(Debug, Deserialize)]
pub struct CustomThemeFile {
    /// Theme metadata.
    pub theme: ThemeMetadata,
    /// Color overrides.
    #[serde(default)]
    pub colors: HashMap<String, String>,
    /// Named color palette for reuse.
    #[serde(default)]
    pub palette: HashMap<String, String>,
}

/// Theme metadata.
#[derive(Debug, Deserialize)]
pub struct ThemeMetadata {
    /// Theme display name.
    pub name: String,
    /// Theme description.
    #[serde(default)]
    pub description: String,
    /// Base theme to inherit from.
    #[serde(default)]
    pub base: Option<String>,
}

/// Errors that can occur when loading custom themes.
#[derive(Debug)]
pub enum CustomThemeError {
    /// Failed to read the theme file.
    Io(io::Error),
    /// Failed to parse the TOML.
    Parse(toml::de::Error),
    /// Invalid color value.
    InvalidColor(String),
    /// Base theme not found.
    BaseNotFound(String),
}

impl std::fmt::Display for CustomThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CustomThemeError::Io(e) => write!(f, "IO error: {}", e),
            CustomThemeError::Parse(e) => write!(f, "Parse error: {}", e),
            CustomThemeError::InvalidColor(c) => write!(f, "Invalid color: {}", c),
            CustomThemeError::BaseNotFound(b) => write!(f, "Base theme not found: {}", b),
        }
    }
}

impl std::error::Error for CustomThemeError {}

impl From<io::Error> for CustomThemeError {
    fn from(e: io::Error) -> Self {
        CustomThemeError::Io(e)
    }
}

impl From<toml::de::Error> for CustomThemeError {
    fn from(e: toml::de::Error) -> Self {
        CustomThemeError::Parse(e)
    }
}

/// Returns the path to the custom themes directory.
#[must_use]
pub fn themes_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ratterm").join("themes"))
}

/// Ensures the themes directory exists.
pub fn ensure_themes_dir() -> io::Result<PathBuf> {
    let dir = themes_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine home directory",
        )
    })?;

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    Ok(dir)
}

/// Lists all custom theme files in the themes directory.
#[must_use]
pub fn list_custom_themes() -> Vec<PathBuf> {
    let Some(dir) = themes_dir() else {
        return Vec::new();
    };

    if !dir.exists() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect()
}

/// Loads a custom theme from a TOML file.
pub fn load_custom_theme(path: &Path) -> Result<Theme, CustomThemeError> {
    let content = fs::read_to_string(path)?;
    let custom: CustomThemeFile = toml::from_str(&content)?;

    // Start with base theme or default
    let mut theme = if let Some(ref base_name) = custom.theme.base {
        ThemePreset::from_name(base_name)
            .map(|p| p.to_theme())
            .ok_or_else(|| CustomThemeError::BaseNotFound(base_name.clone()))?
    } else {
        Theme::default()
    };

    // Set the theme name
    theme.name = custom.theme.name.clone();

    // Build palette lookup (palette colors can reference other palette colors)
    let palette = resolve_palette(&custom.palette)?;

    // Apply color overrides
    for (key, value) in &custom.colors {
        // If value starts with $, look up in palette without the $ prefix
        let color_str = if value.starts_with('$') {
            let palette_key = value.trim_start_matches('$');
            palette.get(palette_key).unwrap_or(value)
        } else {
            palette.get(value).unwrap_or(value)
        };
        let Some(color) = parse_color(color_str) else {
            return Err(CustomThemeError::InvalidColor(value.clone()));
        };

        apply_color_to_theme(&mut theme, key, color);
    }

    Ok(theme)
}

/// Resolves palette references (colors that reference other palette entries).
fn resolve_palette(
    palette: &HashMap<String, String>,
) -> Result<HashMap<String, String>, CustomThemeError> {
    let mut resolved = HashMap::new();

    // Simple single-level resolution
    for (key, value) in palette {
        let resolved_value = if value.starts_with('$') {
            let ref_name = value.trim_start_matches('$');
            palette
                .get(ref_name)
                .cloned()
                .unwrap_or_else(|| value.clone())
        } else {
            value.clone()
        };
        resolved.insert(key.clone(), resolved_value);
    }

    Ok(resolved)
}

/// Applies a color to the appropriate theme field.
fn apply_color_to_theme(theme: &mut Theme, key: &str, color: ratatui::style::Color) {
    match key {
        // Terminal
        "terminal.foreground" => theme.terminal.foreground = color,
        "terminal.background" => theme.terminal.background = color,
        "terminal.cursor" => theme.terminal.cursor = color,
        "terminal.selection" => theme.terminal.selection = color,
        "terminal.border" => theme.terminal.border = color,
        "terminal.border_focused" => theme.terminal.border_focused = color,

        // Editor
        "editor.foreground" => theme.editor.foreground = color,
        "editor.background" => theme.editor.background = color,
        "editor.cursor" => theme.editor.cursor = color,
        "editor.selection" => theme.editor.selection = color,
        "editor.line_numbers_fg" => theme.editor.line_numbers_fg = color,
        "editor.line_numbers_bg" => theme.editor.line_numbers_bg = color,
        "editor.current_line" => theme.editor.current_line = color,
        "editor.border" => theme.editor.border = color,
        "editor.border_focused" => theme.editor.border_focused = color,

        // Status bar
        "statusbar.foreground" => theme.statusbar.foreground = color,
        "statusbar.background" => theme.statusbar.background = color,
        "statusbar.mode_normal" => theme.statusbar.mode_normal = color,
        "statusbar.mode_insert" => theme.statusbar.mode_insert = color,
        "statusbar.mode_visual" => theme.statusbar.mode_visual = color,
        "statusbar.mode_command" => theme.statusbar.mode_command = color,

        // Tabs
        "tabs.active_bg" => theme.tabs.active_bg = color,
        "tabs.active_fg" => theme.tabs.active_fg = color,
        "tabs.inactive_bg" => theme.tabs.inactive_bg = color,
        "tabs.inactive_fg" => theme.tabs.inactive_fg = color,

        // Popup
        "popup.foreground" => theme.popup.foreground = color,
        "popup.background" => theme.popup.background = color,
        "popup.border" => theme.popup.border = color,
        "popup.selected_bg" => theme.popup.selected_bg = color,
        "popup.selected_fg" => theme.popup.selected_fg = color,
        "popup.input_bg" => theme.popup.input_bg = color,

        // File browser
        "filebrowser.foreground" => theme.file_browser.foreground = color,
        "filebrowser.background" => theme.file_browser.background = color,
        "filebrowser.directory" => theme.file_browser.directory = color,
        "filebrowser.file" => theme.file_browser.file = color,
        "filebrowser.selected_bg" => theme.file_browser.selected_bg = color,
        "filebrowser.selected_fg" => theme.file_browser.selected_fg = color,
        "filebrowser.border" => theme.file_browser.border = color,

        // Unknown keys are ignored
        _ => {}
    }
}

/// Information about a custom theme file.
#[derive(Debug, Clone)]
pub struct CustomThemeInfo {
    /// Path to the theme file.
    pub path: PathBuf,
    /// Theme name from metadata.
    pub name: String,
    /// Theme description.
    pub description: String,
}

/// Loads metadata for all custom themes without fully parsing them.
#[must_use]
pub fn list_custom_theme_info() -> Vec<CustomThemeInfo> {
    list_custom_themes()
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let custom: CustomThemeFile = toml::from_str(&content).ok()?;
            Some(CustomThemeInfo {
                path,
                name: custom.theme.name,
                description: custom.theme.description,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_theme(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(format!("{}.toml", name));
        let mut file = fs::File::create(&path).expect("create test file");
        file.write_all(content.as_bytes()).expect("write test file");
        path
    }

    #[test]
    fn test_load_basic_custom_theme() {
        let dir = TempDir::new().expect("create temp dir");
        let content = r##"
[theme]
name = "My Custom Theme"
description = "A test theme"

[colors]
"terminal.background" = "#1a1a2e"
"terminal.foreground" = "#eaeaea"
"editor.background" = "#16213e"
"##;

        let path = create_test_theme(dir.path(), "test", content);
        let theme = load_custom_theme(&path).expect("load theme");

        assert_eq!(theme.name(), "My Custom Theme");
        assert_eq!(
            theme.terminal.background,
            ratatui::style::Color::Rgb(26, 26, 46)
        );
        assert_eq!(
            theme.editor.background,
            ratatui::style::Color::Rgb(22, 33, 62)
        );
    }

    #[test]
    fn test_load_theme_with_base() {
        let dir = TempDir::new().expect("create temp dir");
        let content = r##"
[theme]
name = "Dracula Modified"
base = "dracula"

[colors]
"terminal.background" = "#1e1e2e"
"##;

        let path = create_test_theme(dir.path(), "dracula-mod", content);
        let theme = load_custom_theme(&path).expect("load theme");

        assert_eq!(theme.name(), "Dracula Modified");
        // Background should be overridden
        assert_eq!(
            theme.terminal.background,
            ratatui::style::Color::Rgb(30, 30, 46)
        );
        // Other colors should come from dracula base
    }

    #[test]
    fn test_load_theme_with_palette() {
        let dir = TempDir::new().expect("create temp dir");
        let content = r##"
[theme]
name = "Palette Test"

[palette]
bg = "#282a36"
fg = "#f8f8f2"

[colors]
"terminal.background" = "$bg"
"terminal.foreground" = "$fg"
"##;

        let path = create_test_theme(dir.path(), "palette", content);
        let theme = load_custom_theme(&path).expect("load theme");

        assert_eq!(
            theme.terminal.background,
            ratatui::style::Color::Rgb(40, 42, 54)
        );
        assert_eq!(
            theme.terminal.foreground,
            ratatui::style::Color::Rgb(248, 248, 242)
        );
    }
}
