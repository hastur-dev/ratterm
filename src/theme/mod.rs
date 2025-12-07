//! Theme system for Ratterm.
//!
//! Provides color customization for all UI components with built-in presets
//! and user customization via `.ratrc` configuration.

pub mod colors;
pub mod component;
pub mod custom;
pub mod persistence;
pub mod preset;

pub use colors::{AnsiPalette, color_to_hex, parse_color};
pub use component::{
    EditorTheme, FileBrowserTheme, PopupTheme, StatusBarTheme, TabTheme, TerminalTheme, Theme,
};
pub use custom::{
    CustomThemeError, CustomThemeInfo, ensure_themes_dir, list_custom_theme_info,
    list_custom_themes, load_custom_theme, themes_dir,
};
pub use persistence::{
    ThemeSettings, save_color_setting, save_setting, save_tab_pattern, save_tab_themes,
    save_theme_preset,
};
pub use preset::ThemePreset;

use std::collections::HashMap;

/// Theme manager that handles theme selection and customization.
#[derive(Debug, Clone)]
pub struct ThemeManager {
    /// Current active theme.
    current: Theme,
    /// Current preset (if using a preset).
    current_preset: Option<ThemePreset>,
    /// Per-shell theme overrides.
    shell_themes: HashMap<String, Theme>,
    /// Tab theme cycling pattern.
    tab_pattern: TabThemePattern,
    /// Themes for tab cycling.
    tab_themes: Vec<Theme>,
    /// Current tab theme index for cycling.
    tab_cycle_index: usize,
}

/// Pattern for assigning themes to new tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabThemePattern {
    /// All tabs use the same theme.
    #[default]
    Same,
    /// Tabs cycle through a list of themes.
    Sequential,
    /// Tabs get random themes from the list.
    Random,
}

impl TabThemePattern {
    /// Parse pattern from string.
    #[must_use]
    pub fn from_name(name: &str) -> Option<TabThemePattern> {
        match name.to_lowercase().as_str() {
            "same" | "none" => Some(TabThemePattern::Same),
            "sequential" | "cycle" => Some(TabThemePattern::Sequential),
            "random" => Some(TabThemePattern::Random),
            _ => None,
        }
    }

    /// Get pattern name as string.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            TabThemePattern::Same => "same",
            TabThemePattern::Sequential => "sequential",
            TabThemePattern::Random => "random",
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            current: Theme::default(),
            current_preset: Some(ThemePreset::Dark),
            shell_themes: HashMap::new(),
            tab_pattern: TabThemePattern::Same,
            tab_themes: vec![Theme::default()],
            tab_cycle_index: 0,
        }
    }
}

impl ThemeManager {
    /// Creates a new theme manager with the default theme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a theme manager with a specific preset.
    #[must_use]
    pub fn with_preset(preset: ThemePreset) -> Self {
        Self {
            current: preset.to_theme(),
            current_preset: Some(preset),
            ..Default::default()
        }
    }

    /// Returns the current theme.
    #[must_use]
    pub fn current(&self) -> &Theme {
        &self.current
    }

    /// Returns the current preset, if using one.
    #[must_use]
    pub fn current_preset(&self) -> Option<ThemePreset> {
        self.current_preset
    }

    /// Sets the theme from a preset.
    pub fn set_preset(&mut self, preset: ThemePreset) {
        self.current = preset.to_theme();
        self.current_preset = Some(preset);
    }

    /// Sets a custom theme (clears preset).
    pub fn set_theme(&mut self, theme: Theme) {
        self.current = theme;
        self.current_preset = None;
    }

    /// Gets the theme for a specific shell type.
    #[must_use]
    pub fn theme_for_shell(&self, shell: &str) -> &Theme {
        self.shell_themes.get(shell).unwrap_or(&self.current)
    }

    /// Sets a theme override for a specific shell.
    pub fn set_shell_theme(&mut self, shell: &str, theme: Theme) {
        self.shell_themes.insert(shell.to_lowercase(), theme);
    }

    /// Removes the theme override for a specific shell.
    pub fn clear_shell_theme(&mut self, shell: &str) {
        self.shell_themes.remove(&shell.to_lowercase());
    }

    /// Gets the next theme for a new tab based on the pattern.
    #[must_use]
    pub fn next_tab_theme(&mut self) -> Theme {
        assert!(
            !self.tab_themes.is_empty(),
            "Tab themes list cannot be empty"
        );

        match self.tab_pattern {
            TabThemePattern::Same => self.current.clone(),
            TabThemePattern::Sequential => {
                let theme = self.tab_themes[self.tab_cycle_index].clone();
                self.tab_cycle_index = (self.tab_cycle_index + 1) % self.tab_themes.len();
                theme
            }
            TabThemePattern::Random => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as usize)
                    .unwrap_or(0);
                let index = seed % self.tab_themes.len();
                self.tab_themes[index].clone()
            }
        }
    }

    /// Sets the tab theme pattern.
    pub fn set_tab_pattern(&mut self, pattern: TabThemePattern) {
        self.tab_pattern = pattern;
    }

    /// Sets the themes for tab cycling.
    pub fn set_tab_themes(&mut self, themes: Vec<Theme>) {
        assert!(!themes.is_empty(), "Tab themes list cannot be empty");
        self.tab_themes = themes;
        self.tab_cycle_index = 0;
    }

    /// Adds a theme to the tab cycling list.
    pub fn add_tab_theme(&mut self, theme: Theme) {
        self.tab_themes.push(theme);
    }

    /// Returns all shell theme overrides.
    #[must_use]
    pub fn shell_themes(&self) -> &HashMap<String, Theme> {
        &self.shell_themes
    }

    /// Returns the tab theme pattern.
    #[must_use]
    pub fn tab_pattern(&self) -> TabThemePattern {
        self.tab_pattern
    }

    /// Returns the tab themes list.
    #[must_use]
    pub fn tab_themes(&self) -> &[Theme] {
        &self.tab_themes
    }

    /// Loads and sets a custom theme from a file path.
    pub fn load_custom_theme(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), custom::CustomThemeError> {
        let theme = custom::load_custom_theme(path)?;
        self.current = theme;
        self.current_preset = None; // Custom themes clear the preset
        Ok(())
    }

    /// Lists available custom themes.
    #[must_use]
    pub fn list_custom_themes(&self) -> Vec<custom::CustomThemeInfo> {
        custom::list_custom_theme_info()
    }

    /// Returns all available themes (presets + custom).
    #[must_use]
    pub fn all_available_themes(&self) -> Vec<String> {
        let mut themes: Vec<String> = ThemePreset::all()
            .iter()
            .map(|p| p.name().to_string())
            .collect();

        for info in custom::list_custom_theme_info() {
            themes.push(info.name);
        }

        themes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_manager_default() {
        let manager = ThemeManager::new();
        assert_eq!(manager.current().name(), "dark");
        assert_eq!(manager.current_preset(), Some(ThemePreset::Dark));
    }

    #[test]
    fn test_theme_manager_preset() {
        let manager = ThemeManager::with_preset(ThemePreset::Dracula);
        assert_eq!(manager.current().name(), "dracula");
        assert_eq!(manager.current_preset(), Some(ThemePreset::Dracula));
    }

    #[test]
    fn test_set_preset() {
        let mut manager = ThemeManager::new();
        manager.set_preset(ThemePreset::Nord);
        assert_eq!(manager.current().name(), "nord");
    }

    #[test]
    fn test_shell_themes() {
        let mut manager = ThemeManager::new();
        let pwsh_theme = ThemePreset::Dracula.to_theme();
        manager.set_shell_theme("powershell", pwsh_theme);

        assert_eq!(manager.theme_for_shell("powershell").name(), "dracula");
        assert_eq!(manager.theme_for_shell("bash").name(), "dark");
    }

    #[test]
    fn test_tab_pattern_sequential() {
        let mut manager = ThemeManager::new();
        manager.set_tab_pattern(TabThemePattern::Sequential);
        manager.set_tab_themes(vec![
            ThemePreset::Dark.to_theme(),
            ThemePreset::Dracula.to_theme(),
            ThemePreset::Nord.to_theme(),
        ]);

        assert_eq!(manager.next_tab_theme().name(), "dark");
        assert_eq!(manager.next_tab_theme().name(), "dracula");
        assert_eq!(manager.next_tab_theme().name(), "nord");
        assert_eq!(manager.next_tab_theme().name(), "dark"); // cycles
    }

    #[test]
    fn test_tab_pattern_from_name() {
        assert_eq!(
            TabThemePattern::from_name("same"),
            Some(TabThemePattern::Same)
        );
        assert_eq!(
            TabThemePattern::from_name("sequential"),
            Some(TabThemePattern::Sequential)
        );
        assert_eq!(
            TabThemePattern::from_name("cycle"),
            Some(TabThemePattern::Sequential)
        );
        assert_eq!(
            TabThemePattern::from_name("random"),
            Some(TabThemePattern::Random)
        );
        assert_eq!(TabThemePattern::from_name("invalid"), None);
    }
}
