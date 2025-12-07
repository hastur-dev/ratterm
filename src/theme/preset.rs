//! Built-in theme presets.
//!
//! Provides pre-configured themes like dark, light, dracula, etc.

use ratatui::style::Color;

use super::component::{
    EditorTheme, FileBrowserTheme, PopupTheme, StatusBarTheme, TabTheme, TerminalTheme, Theme,
};

/// Available built-in theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreset {
    /// Dark theme (default).
    Dark,
    /// Light theme.
    Light,
    /// Dracula theme.
    Dracula,
    /// Gruvbox Dark theme.
    Gruvbox,
    /// Nord theme.
    Nord,
}

impl ThemePreset {
    /// Returns all available presets.
    #[must_use]
    pub fn all() -> &'static [ThemePreset] {
        &[
            ThemePreset::Dark,
            ThemePreset::Light,
            ThemePreset::Dracula,
            ThemePreset::Gruvbox,
            ThemePreset::Nord,
        ]
    }

    /// Returns the preset name as a string.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            ThemePreset::Dark => "dark",
            ThemePreset::Light => "light",
            ThemePreset::Dracula => "dracula",
            ThemePreset::Gruvbox => "gruvbox",
            ThemePreset::Nord => "nord",
        }
    }

    /// Creates a Theme from this preset.
    #[must_use]
    pub fn to_theme(&self) -> Theme {
        match self {
            ThemePreset::Dark => create_dark_theme(),
            ThemePreset::Light => create_light_theme(),
            ThemePreset::Dracula => create_dracula_theme(),
            ThemePreset::Gruvbox => create_gruvbox_theme(),
            ThemePreset::Nord => create_nord_theme(),
        }
    }

    /// Try to parse a preset from a string.
    #[must_use]
    pub fn from_name(name: &str) -> Option<ThemePreset> {
        match name.to_lowercase().as_str() {
            "dark" => Some(ThemePreset::Dark),
            "light" => Some(ThemePreset::Light),
            "dracula" => Some(ThemePreset::Dracula),
            "gruvbox" => Some(ThemePreset::Gruvbox),
            "nord" => Some(ThemePreset::Nord),
            _ => None,
        }
    }
}

/// Creates the default dark theme.
fn create_dark_theme() -> Theme {
    Theme::default()
}

/// Creates a light theme.
fn create_light_theme() -> Theme {
    Theme {
        name: "light".to_string(),
        terminal: TerminalTheme {
            foreground: Color::Rgb(30, 30, 30),
            background: Color::Rgb(255, 255, 255),
            cursor: Color::Rgb(30, 30, 30),
            selection: Color::Rgb(173, 214, 255),
            border: Color::Rgb(200, 200, 200),
            border_focused: Color::Rgb(0, 122, 204),
            ..Default::default()
        },
        editor: EditorTheme {
            foreground: Color::Rgb(30, 30, 30),
            background: Color::Rgb(255, 255, 255),
            line_numbers_fg: Color::Rgb(120, 120, 120),
            line_numbers_bg: Color::Rgb(245, 245, 245),
            current_line: Color::Rgb(240, 240, 240),
            selection: Color::Rgb(173, 214, 255),
            cursor: Color::Rgb(30, 30, 30),
            border: Color::Rgb(200, 200, 200),
            border_focused: Color::Rgb(0, 122, 204),
        },
        statusbar: StatusBarTheme {
            background: Color::Rgb(0, 122, 204),
            foreground: Color::White,
            mode_normal: Color::Rgb(0, 122, 204),
            mode_insert: Color::Rgb(22, 163, 74),
            mode_visual: Color::Rgb(147, 51, 234),
            mode_command: Color::Rgb(217, 119, 6),
        },
        tabs: TabTheme {
            active_bg: Color::Rgb(255, 255, 255),
            active_fg: Color::Rgb(30, 30, 30),
            inactive_bg: Color::Rgb(236, 236, 236),
            inactive_fg: Color::Rgb(100, 100, 100),
        },
        popup: PopupTheme {
            background: Color::Rgb(255, 255, 255),
            foreground: Color::Rgb(30, 30, 30),
            border: Color::Rgb(200, 200, 200),
            selected_bg: Color::Rgb(0, 122, 204),
            selected_fg: Color::White,
            input_bg: Color::Rgb(245, 245, 245),
        },
        file_browser: FileBrowserTheme {
            background: Color::Rgb(250, 250, 250),
            foreground: Color::Rgb(30, 30, 30),
            directory: Color::Rgb(0, 122, 204),
            file: Color::Rgb(30, 30, 30),
            selected_bg: Color::Rgb(0, 122, 204),
            selected_fg: Color::White,
            border: Color::Rgb(200, 200, 200),
        },
    }
}

/// Creates a Dracula theme.
fn create_dracula_theme() -> Theme {
    // Dracula colors
    let bg = Color::Rgb(40, 42, 54);
    let fg = Color::Rgb(248, 248, 242);
    let selection = Color::Rgb(68, 71, 90);
    let comment = Color::Rgb(98, 114, 164);
    let cyan = Color::Rgb(139, 233, 253);
    let green = Color::Rgb(80, 250, 123);
    let orange = Color::Rgb(255, 184, 108);
    let pink = Color::Rgb(255, 121, 198);
    let purple = Color::Rgb(189, 147, 249);

    Theme {
        name: "dracula".to_string(),
        terminal: TerminalTheme {
            foreground: fg,
            background: bg,
            cursor: fg,
            selection,
            border: comment,
            border_focused: purple,
            ..Default::default()
        },
        editor: EditorTheme {
            foreground: fg,
            background: bg,
            line_numbers_fg: comment,
            line_numbers_bg: bg,
            current_line: Color::Rgb(50, 52, 64),
            selection,
            cursor: fg,
            border: comment,
            border_focused: purple,
        },
        statusbar: StatusBarTheme {
            background: purple,
            foreground: bg,
            mode_normal: purple,
            mode_insert: green,
            mode_visual: pink,
            mode_command: orange,
        },
        tabs: TabTheme {
            active_bg: bg,
            active_fg: fg,
            inactive_bg: Color::Rgb(50, 52, 64),
            inactive_fg: comment,
        },
        popup: PopupTheme {
            background: bg,
            foreground: fg,
            border: comment,
            selected_bg: selection,
            selected_fg: fg,
            input_bg: Color::Rgb(50, 52, 64),
        },
        file_browser: FileBrowserTheme {
            background: bg,
            foreground: fg,
            directory: cyan,
            file: fg,
            selected_bg: selection,
            selected_fg: fg,
            border: comment,
        },
    }
}

/// Creates a Gruvbox Dark theme.
fn create_gruvbox_theme() -> Theme {
    // Gruvbox colors
    let bg = Color::Rgb(40, 40, 40);
    let fg = Color::Rgb(235, 219, 178);
    let gray = Color::Rgb(146, 131, 116);
    let _red = Color::Rgb(251, 73, 52);
    let green = Color::Rgb(184, 187, 38);
    let yellow = Color::Rgb(250, 189, 47);
    let blue = Color::Rgb(131, 165, 152);
    let purple = Color::Rgb(211, 134, 155);
    let aqua = Color::Rgb(142, 192, 124);
    let orange = Color::Rgb(254, 128, 25);

    Theme {
        name: "gruvbox".to_string(),
        terminal: TerminalTheme {
            foreground: fg,
            background: bg,
            cursor: fg,
            selection: Color::Rgb(60, 60, 60),
            border: gray,
            border_focused: yellow,
            ..Default::default()
        },
        editor: EditorTheme {
            foreground: fg,
            background: bg,
            line_numbers_fg: gray,
            line_numbers_bg: bg,
            current_line: Color::Rgb(50, 48, 47),
            selection: Color::Rgb(60, 60, 60),
            cursor: fg,
            border: gray,
            border_focused: yellow,
        },
        statusbar: StatusBarTheme {
            background: Color::Rgb(50, 48, 47),
            foreground: fg,
            mode_normal: blue,
            mode_insert: green,
            mode_visual: purple,
            mode_command: orange,
        },
        tabs: TabTheme {
            active_bg: bg,
            active_fg: fg,
            inactive_bg: Color::Rgb(50, 48, 47),
            inactive_fg: gray,
        },
        popup: PopupTheme {
            background: Color::Rgb(50, 48, 47),
            foreground: fg,
            border: gray,
            selected_bg: Color::Rgb(60, 60, 60),
            selected_fg: fg,
            input_bg: bg,
        },
        file_browser: FileBrowserTheme {
            background: bg,
            foreground: fg,
            directory: aqua,
            file: fg,
            selected_bg: Color::Rgb(60, 60, 60),
            selected_fg: fg,
            border: gray,
        },
    }
}

/// Creates a Nord theme.
fn create_nord_theme() -> Theme {
    // Nord colors
    let polar_night_0 = Color::Rgb(46, 52, 64);
    let polar_night_1 = Color::Rgb(59, 66, 82);
    let polar_night_2 = Color::Rgb(67, 76, 94);
    let polar_night_3 = Color::Rgb(76, 86, 106);
    let snow_storm_0 = Color::Rgb(216, 222, 233);
    let snow_storm_1 = Color::Rgb(229, 233, 240);
    let frost_0 = Color::Rgb(143, 188, 187);
    let frost_1 = Color::Rgb(136, 192, 208);
    let frost_3 = Color::Rgb(129, 161, 193);
    let _aurora_red = Color::Rgb(191, 97, 106);
    let aurora_orange = Color::Rgb(208, 135, 112);
    let aurora_green = Color::Rgb(163, 190, 140);
    let aurora_purple = Color::Rgb(180, 142, 173);

    Theme {
        name: "nord".to_string(),
        terminal: TerminalTheme {
            foreground: snow_storm_0,
            background: polar_night_0,
            cursor: snow_storm_0,
            selection: polar_night_2,
            border: polar_night_3,
            border_focused: frost_1,
            ..Default::default()
        },
        editor: EditorTheme {
            foreground: snow_storm_0,
            background: polar_night_0,
            line_numbers_fg: polar_night_3,
            line_numbers_bg: polar_night_0,
            current_line: polar_night_1,
            selection: polar_night_2,
            cursor: snow_storm_0,
            border: polar_night_3,
            border_focused: frost_1,
        },
        statusbar: StatusBarTheme {
            background: polar_night_1,
            foreground: snow_storm_1,
            mode_normal: frost_3,
            mode_insert: aurora_green,
            mode_visual: aurora_purple,
            mode_command: aurora_orange,
        },
        tabs: TabTheme {
            active_bg: polar_night_0,
            active_fg: snow_storm_0,
            inactive_bg: polar_night_1,
            inactive_fg: polar_night_3,
        },
        popup: PopupTheme {
            background: polar_night_1,
            foreground: snow_storm_0,
            border: polar_night_3,
            selected_bg: polar_night_2,
            selected_fg: snow_storm_1,
            input_bg: polar_night_0,
        },
        file_browser: FileBrowserTheme {
            background: polar_night_0,
            foreground: snow_storm_0,
            directory: frost_0,
            file: snow_storm_0,
            selected_bg: polar_night_2,
            selected_fg: snow_storm_1,
            border: polar_night_3,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_from_name() {
        assert_eq!(ThemePreset::from_name("dark"), Some(ThemePreset::Dark));
        assert_eq!(ThemePreset::from_name("LIGHT"), Some(ThemePreset::Light));
        assert_eq!(
            ThemePreset::from_name("Dracula"),
            Some(ThemePreset::Dracula)
        );
        assert_eq!(ThemePreset::from_name("invalid"), None);
    }

    #[test]
    fn test_preset_to_theme() {
        let theme = ThemePreset::Dark.to_theme();
        assert_eq!(theme.name(), "dark");

        let theme = ThemePreset::Dracula.to_theme();
        assert_eq!(theme.name(), "dracula");
    }

    #[test]
    fn test_all_presets() {
        let presets = ThemePreset::all();
        assert_eq!(presets.len(), 5);
    }
}
