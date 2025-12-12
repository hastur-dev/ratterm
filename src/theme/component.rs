//! Component-specific theme settings.
//!
//! Defines theme structures for each UI component.

use ratatui::style::Color;

use super::colors::AnsiPalette;

/// Terminal pane theme.
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    /// Foreground (text) color.
    pub foreground: Color,
    /// Background color.
    pub background: Color,
    /// Cursor color.
    pub cursor: Color,
    /// Selection highlight color.
    pub selection: Color,
    /// Border color when unfocused.
    pub border: Color,
    /// Border color when focused.
    pub border_focused: Color,
    /// ANSI color palette.
    pub palette: AnsiPalette,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            foreground: Color::White,
            background: Color::Rgb(30, 30, 30),
            cursor: Color::White,
            selection: Color::Rgb(38, 79, 120),
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            palette: AnsiPalette::default(),
        }
    }
}

/// Editor pane theme.
#[derive(Debug, Clone)]
pub struct EditorTheme {
    /// Foreground (text) color.
    pub foreground: Color,
    /// Background color.
    pub background: Color,
    /// Line numbers foreground color.
    pub line_numbers_fg: Color,
    /// Line numbers background color.
    pub line_numbers_bg: Color,
    /// Current line highlight color.
    pub current_line: Color,
    /// Selection highlight color.
    pub selection: Color,
    /// Cursor color.
    pub cursor: Color,
    /// Border color when unfocused.
    pub border: Color,
    /// Border color when focused.
    pub border_focused: Color,
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self {
            foreground: Color::Rgb(212, 212, 212),
            background: Color::Rgb(30, 30, 30),
            line_numbers_fg: Color::Rgb(133, 133, 133),
            line_numbers_bg: Color::Rgb(30, 30, 30),
            current_line: Color::Rgb(42, 42, 42),
            selection: Color::Rgb(38, 79, 120),
            cursor: Color::White,
            border: Color::DarkGray,
            border_focused: Color::Rgb(86, 156, 214),
        }
    }
}

/// Status bar theme.
#[derive(Debug, Clone)]
pub struct StatusBarTheme {
    /// Background color.
    pub background: Color,
    /// Foreground (text) color.
    pub foreground: Color,
    /// Normal mode indicator color.
    pub mode_normal: Color,
    /// Insert mode indicator color.
    pub mode_insert: Color,
    /// Visual mode indicator color.
    pub mode_visual: Color,
    /// Command mode indicator color.
    pub mode_command: Color,
}

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(0, 122, 204),
            foreground: Color::White,
            mode_normal: Color::Rgb(0, 122, 204),
            mode_insert: Color::Rgb(78, 201, 176),
            mode_visual: Color::Rgb(197, 134, 192),
            mode_command: Color::Rgb(206, 145, 120),
        }
    }
}

/// Tab bar theme.
#[derive(Debug, Clone)]
pub struct TabTheme {
    /// Active tab background.
    pub active_bg: Color,
    /// Active tab foreground.
    pub active_fg: Color,
    /// Inactive tab background.
    pub inactive_bg: Color,
    /// Inactive tab foreground.
    pub inactive_fg: Color,
}

impl Default for TabTheme {
    fn default() -> Self {
        Self {
            active_bg: Color::Rgb(30, 30, 30),
            active_fg: Color::White,
            inactive_bg: Color::Rgb(45, 45, 45),
            inactive_fg: Color::Rgb(128, 128, 128),
        }
    }
}

/// Popup/dialog theme.
#[derive(Debug, Clone)]
pub struct PopupTheme {
    /// Background color.
    pub background: Color,
    /// Foreground (text) color.
    pub foreground: Color,
    /// Border color.
    pub border: Color,
    /// Selected item background.
    pub selected_bg: Color,
    /// Selected item foreground.
    pub selected_fg: Color,
    /// Input field background.
    pub input_bg: Color,
}

impl Default for PopupTheme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(37, 37, 38),
            foreground: Color::Rgb(204, 204, 204),
            border: Color::Rgb(60, 60, 60),
            selected_bg: Color::Rgb(9, 71, 113),
            selected_fg: Color::White,
            input_bg: Color::Rgb(60, 60, 60),
        }
    }
}

/// File browser theme.
#[derive(Debug, Clone)]
pub struct FileBrowserTheme {
    /// Background color.
    pub background: Color,
    /// Foreground (text) color.
    pub foreground: Color,
    /// Directory color.
    pub directory: Color,
    /// File color.
    pub file: Color,
    /// Selected item background.
    pub selected_bg: Color,
    /// Selected item foreground.
    pub selected_fg: Color,
    /// Border color.
    pub border: Color,
}

impl Default for FileBrowserTheme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(37, 37, 38),
            foreground: Color::Rgb(204, 204, 204),
            directory: Color::Rgb(86, 156, 214),
            file: Color::Rgb(204, 204, 204),
            selected_bg: Color::Rgb(9, 71, 113),
            selected_fg: Color::White,
            border: Color::Rgb(60, 60, 60),
        }
    }
}

/// Complete theme configuration.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name.
    pub name: String,
    /// Terminal pane theme.
    pub terminal: TerminalTheme,
    /// Editor pane theme.
    pub editor: EditorTheme,
    /// Status bar theme.
    pub statusbar: StatusBarTheme,
    /// Tab bar theme.
    pub tabs: TabTheme,
    /// Popup/dialog theme.
    pub popup: PopupTheme,
    /// File browser theme.
    pub file_browser: FileBrowserTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "dark".to_string(),
            terminal: TerminalTheme::default(),
            editor: EditorTheme::default(),
            statusbar: StatusBarTheme::default(),
            tabs: TabTheme::default(),
            popup: PopupTheme::default(),
            file_browser: FileBrowserTheme::default(),
        }
    }
}

impl Theme {
    /// Creates a new theme with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Returns the theme name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
