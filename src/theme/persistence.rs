//! Theme persistence for saving/loading from .ratrc.
//!
//! Handles reading and writing theme configuration to the .ratrc file.

use std::fs;
use std::io;
use std::path::Path;

use ratatui::style::Color;

use super::{
    TabThemePattern, ThemeManager,
    colors::{color_to_hex, parse_color},
    component::Theme,
    preset::ThemePreset,
};

/// Theme settings that can be persisted to .ratrc.
#[derive(Debug, Clone, Default)]
pub struct ThemeSettings {
    /// Global theme preset name.
    pub theme: Option<String>,
    /// Terminal foreground color.
    pub terminal_foreground: Option<Color>,
    /// Terminal background color.
    pub terminal_background: Option<Color>,
    /// Terminal cursor color.
    pub terminal_cursor: Option<Color>,
    /// Terminal selection color.
    pub terminal_selection: Option<Color>,
    /// Terminal border color.
    pub terminal_border: Option<Color>,
    /// Terminal focused border color.
    pub terminal_border_focused: Option<Color>,
    /// Editor foreground color.
    pub editor_foreground: Option<Color>,
    /// Editor background color.
    pub editor_background: Option<Color>,
    /// Editor line numbers foreground.
    pub editor_line_numbers_fg: Option<Color>,
    /// Editor current line highlight.
    pub editor_current_line: Option<Color>,
    /// Editor cursor color.
    pub editor_cursor: Option<Color>,
    /// Status bar background color.
    pub statusbar_background: Option<Color>,
    /// Status bar foreground color.
    pub statusbar_foreground: Option<Color>,
    /// Tab active background.
    pub tab_active_bg: Option<Color>,
    /// Tab active foreground.
    pub tab_active_fg: Option<Color>,
    /// Tab inactive background.
    pub tab_inactive_bg: Option<Color>,
    /// Tab inactive foreground.
    pub tab_inactive_fg: Option<Color>,
    /// Tab theme pattern.
    pub tab_theme_pattern: Option<TabThemePattern>,
    /// Tab themes list (preset names).
    pub tab_themes: Vec<String>,
}

impl ThemeSettings {
    /// Parses theme settings from .ratrc content.
    #[must_use]
    pub fn parse(content: &str) -> Self {
        let mut settings = Self::default();
        let mut current_section: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Check for section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = Some(line[1..line.len() - 1].to_string());
                continue;
            }

            // Parse key = value
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.split('#').next().unwrap_or(value).trim();

                // Apply setting based on current section
                if current_section.is_none() {
                    settings.apply_global_setting(key, value);
                }
            }
        }

        settings
    }

    /// Applies a global (non-sectioned) setting.
    fn apply_global_setting(&mut self, key: &str, value: &str) {
        match key {
            "theme" => self.theme = Some(value.to_string()),
            "terminal.foreground" => self.terminal_foreground = parse_color(value),
            "terminal.background" => self.terminal_background = parse_color(value),
            "terminal.cursor" => self.terminal_cursor = parse_color(value),
            "terminal.selection" => self.terminal_selection = parse_color(value),
            "terminal.border" => self.terminal_border = parse_color(value),
            "terminal.border_focused" => self.terminal_border_focused = parse_color(value),
            "editor.foreground" => self.editor_foreground = parse_color(value),
            "editor.background" => self.editor_background = parse_color(value),
            "editor.line_numbers" => self.editor_line_numbers_fg = parse_color(value),
            "editor.current_line" => self.editor_current_line = parse_color(value),
            "editor.cursor" => self.editor_cursor = parse_color(value),
            "statusbar.background" => self.statusbar_background = parse_color(value),
            "statusbar.foreground" => self.statusbar_foreground = parse_color(value),
            "tab.active_bg" => self.tab_active_bg = parse_color(value),
            "tab.active_fg" => self.tab_active_fg = parse_color(value),
            "tab.inactive_bg" => self.tab_inactive_bg = parse_color(value),
            "tab.inactive_fg" => self.tab_inactive_fg = parse_color(value),
            "tab_theme_pattern" => self.tab_theme_pattern = TabThemePattern::from_name(value),
            "tab_themes" => {
                self.tab_themes = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    /// Applies these settings to a theme, returning a modified theme.
    #[must_use]
    pub fn apply_to_theme(&self, base: &Theme) -> Theme {
        let mut theme = base.clone();

        // Terminal settings
        if let Some(color) = self.terminal_foreground {
            theme.terminal.foreground = color;
        }
        if let Some(color) = self.terminal_background {
            theme.terminal.background = color;
        }
        if let Some(color) = self.terminal_cursor {
            theme.terminal.cursor = color;
        }
        if let Some(color) = self.terminal_selection {
            theme.terminal.selection = color;
        }
        if let Some(color) = self.terminal_border {
            theme.terminal.border = color;
        }
        if let Some(color) = self.terminal_border_focused {
            theme.terminal.border_focused = color;
        }

        // Editor settings
        if let Some(color) = self.editor_foreground {
            theme.editor.foreground = color;
        }
        if let Some(color) = self.editor_background {
            theme.editor.background = color;
        }
        if let Some(color) = self.editor_line_numbers_fg {
            theme.editor.line_numbers_fg = color;
        }
        if let Some(color) = self.editor_current_line {
            theme.editor.current_line = color;
        }
        if let Some(color) = self.editor_cursor {
            theme.editor.cursor = color;
        }

        // Status bar settings
        if let Some(color) = self.statusbar_background {
            theme.statusbar.background = color;
        }
        if let Some(color) = self.statusbar_foreground {
            theme.statusbar.foreground = color;
        }

        // Tab settings
        if let Some(color) = self.tab_active_bg {
            theme.tabs.active_bg = color;
        }
        if let Some(color) = self.tab_active_fg {
            theme.tabs.active_fg = color;
        }
        if let Some(color) = self.tab_inactive_bg {
            theme.tabs.inactive_bg = color;
        }
        if let Some(color) = self.tab_inactive_fg {
            theme.tabs.inactive_fg = color;
        }

        theme
    }

    /// Applies settings to a theme manager.
    pub fn apply_to_manager(&self, manager: &mut ThemeManager) {
        // Set base theme from preset
        if let Some(ref theme_name) = self.theme {
            if let Some(preset) = ThemePreset::from_name(theme_name) {
                manager.set_preset(preset);
            }
        }

        // Apply color overrides
        let current = manager.current().clone();
        let modified = self.apply_to_theme(&current);
        manager.set_theme(modified);

        // Set tab pattern
        if let Some(pattern) = self.tab_theme_pattern {
            manager.set_tab_pattern(pattern);
        }

        // Set tab themes
        if !self.tab_themes.is_empty() {
            let themes: Vec<Theme> = self
                .tab_themes
                .iter()
                .filter_map(|name| ThemePreset::from_name(name).map(|p| p.to_theme()))
                .collect();
            if !themes.is_empty() {
                manager.set_tab_themes(themes);
            }
        }
    }
}

/// Saves a single setting to the .ratrc file.
///
/// This function reads the file, updates or appends the setting, and writes back.
///
/// # Errors
/// Returns error if file cannot be read or written.
pub fn save_setting(path: &Path, key: &str, value: &str) -> io::Result<()> {
    assert!(!key.is_empty(), "Setting key cannot be empty");
    assert!(!value.is_empty(), "Setting value cannot be empty");

    let content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    let new_content = update_or_append_setting(&content, key, value);
    fs::write(path, new_content)?;

    Ok(())
}

/// Updates an existing setting or appends a new one.
fn update_or_append_setting(content: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut found = false;
    let setting_line = format!("{} = {}", key, value);

    // Find and update existing setting
    for line in &mut lines {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            if let Some((existing_key, _)) = trimmed.split_once('=') {
                if existing_key.trim() == key {
                    *line = setting_line.clone();
                    found = true;
                    break;
                }
            }
        }
    }

    // Append if not found
    if !found {
        // Find appropriate section or add at end
        let section = find_section_for_key(key);
        let insert_pos = find_insert_position(&lines, section);
        lines.insert(insert_pos, setting_line);
    }

    lines.join("\n")
}

/// Determines which section a key belongs to.
fn find_section_for_key(key: &str) -> Option<&'static str> {
    if key.starts_with("terminal.") {
        Some("# Terminal")
    } else if key.starts_with("editor.") {
        Some("# Editor")
    } else if key.starts_with("statusbar.") {
        Some("# Status Bar")
    } else if key.starts_with("tab.") || key.starts_with("tab_") {
        Some("# Tabs")
    } else if key == "theme" {
        Some("# Theme")
    } else {
        None
    }
}

/// Finds the position to insert a new setting.
fn find_insert_position(lines: &[String], section: Option<&str>) -> usize {
    if let Some(section_header) = section {
        // Look for section header
        for (i, line) in lines.iter().enumerate() {
            if line.contains(section_header) {
                // Find end of section (next section or end of file)
                for (offset, subsequent_line) in lines.iter().skip(i + 1).enumerate() {
                    let trimmed = subsequent_line.trim();
                    if trimmed.starts_with("# ") && trimmed.len() > 2 {
                        return i + 1 + offset;
                    }
                }
                return lines.len();
            }
        }
    }
    lines.len()
}

/// Saves a color setting to .ratrc.
///
/// # Errors
/// Returns error if file cannot be written.
pub fn save_color_setting(path: &Path, key: &str, color: Color) -> io::Result<()> {
    if let Some(hex) = color_to_hex(color) {
        save_setting(path, key, &hex)
    } else {
        // For indexed colors, use color<n> format
        match color {
            Color::Indexed(n) => save_setting(path, key, &format!("color{}", n)),
            _ => Ok(()), // Skip unsupported colors
        }
    }
}

/// Saves the theme preset to .ratrc.
///
/// # Errors
/// Returns error if file cannot be written.
pub fn save_theme_preset(path: &Path, preset: ThemePreset) -> io::Result<()> {
    save_setting(path, "theme", preset.name())
}

/// Saves tab theme pattern to .ratrc.
///
/// # Errors
/// Returns error if file cannot be written.
pub fn save_tab_pattern(path: &Path, pattern: TabThemePattern) -> io::Result<()> {
    save_setting(path, "tab_theme_pattern", pattern.name())
}

/// Saves tab themes list to .ratrc.
///
/// # Errors
/// Returns error if file cannot be written.
pub fn save_tab_themes(path: &Path, themes: &[ThemePreset]) -> io::Result<()> {
    let names: Vec<&str> = themes.iter().map(ThemePreset::name).collect();
    save_setting(path, "tab_themes", &names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_theme_settings() {
        let content = r#"
# Theme configuration
theme = dracula
terminal.foreground = #f8f8f2
terminal.background = #282a36
tab_theme_pattern = sequential
tab_themes = dark, dracula, nord
        "#;

        let settings = ThemeSettings::parse(content);
        assert_eq!(settings.theme, Some("dracula".to_string()));
        assert_eq!(
            settings.terminal_foreground,
            Some(Color::Rgb(248, 248, 242))
        );
        assert_eq!(settings.terminal_background, Some(Color::Rgb(40, 42, 54)));
        assert_eq!(
            settings.tab_theme_pattern,
            Some(TabThemePattern::Sequential)
        );
        assert_eq!(settings.tab_themes, vec!["dark", "dracula", "nord"]);
    }

    #[test]
    fn test_update_existing_setting() {
        let content = "theme = dark\nmode = vim\n";
        let result = update_or_append_setting(content, "theme", "dracula");
        assert!(result.contains("theme = dracula"));
        assert!(!result.contains("theme = dark"));
    }

    #[test]
    fn test_append_new_setting() {
        let content = "mode = vim\n";
        let result = update_or_append_setting(content, "theme", "dracula");
        assert!(result.contains("theme = dracula"));
        assert!(result.contains("mode = vim"));
    }

    #[test]
    fn test_apply_to_theme() {
        let settings = ThemeSettings {
            terminal_background: Some(Color::Rgb(10, 20, 30)),
            editor_foreground: Some(Color::Rgb(200, 200, 200)),
            ..Default::default()
        };

        let base = Theme::default();
        let modified = settings.apply_to_theme(&base);

        assert_eq!(modified.terminal.background, Color::Rgb(10, 20, 30));
        assert_eq!(modified.editor.foreground, Color::Rgb(200, 200, 200));
    }

    #[test]
    fn test_find_section_for_key() {
        assert_eq!(
            find_section_for_key("terminal.background"),
            Some("# Terminal")
        );
        assert_eq!(find_section_for_key("editor.foreground"), Some("# Editor"));
        assert_eq!(find_section_for_key("theme"), Some("# Theme"));
        assert_eq!(find_section_for_key("unknown"), None);
    }
}
