//! Terminal text styling (colors and attributes).
//!
//! Provides color and attribute types for terminal cell styling.

/// Terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    /// Default terminal color.
    #[default]
    Default,
    /// Standard black (color 0).
    Black,
    /// Standard red (color 1).
    Red,
    /// Standard green (color 2).
    Green,
    /// Standard yellow (color 3).
    Yellow,
    /// Standard blue (color 4).
    Blue,
    /// Standard magenta (color 5).
    Magenta,
    /// Standard cyan (color 6).
    Cyan,
    /// Standard white (color 7).
    White,
    /// Bright black (color 8).
    BrightBlack,
    /// Bright red (color 9).
    BrightRed,
    /// Bright green (color 10).
    BrightGreen,
    /// Bright yellow (color 11).
    BrightYellow,
    /// Bright blue (color 12).
    BrightBlue,
    /// Bright magenta (color 13).
    BrightMagenta,
    /// Bright cyan (color 14).
    BrightCyan,
    /// Bright white (color 15).
    BrightWhite,
    /// 256-color palette index (0-255).
    Indexed(u8),
    /// True color RGB.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Creates a new indexed color, clamping to valid range.
    #[must_use]
    pub const fn indexed(index: u8) -> Self {
        Self::Indexed(index)
    }

    /// Creates a new RGB color.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb(r, g, b)
    }

    /// Converts standard color code (0-7) to Color.
    #[must_use]
    pub const fn from_standard(code: u8) -> Self {
        match code {
            0 => Self::Black,
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Blue,
            5 => Self::Magenta,
            6 => Self::Cyan,
            7 => Self::White,
            _ => Self::Default,
        }
    }

    /// Converts bright color code (8-15) to Color.
    #[must_use]
    pub const fn from_bright(code: u8) -> Self {
        match code {
            8 => Self::BrightBlack,
            9 => Self::BrightRed,
            10 => Self::BrightGreen,
            11 => Self::BrightYellow,
            12 => Self::BrightBlue,
            13 => Self::BrightMagenta,
            14 => Self::BrightCyan,
            15 => Self::BrightWhite,
            _ => Self::Default,
        }
    }

    /// Converts to ratatui Color for rendering.
    #[must_use]
    pub fn to_ratatui(self) -> ratatui::style::Color {
        match self {
            Self::Default => ratatui::style::Color::Reset,
            Self::Black => ratatui::style::Color::Black,
            Self::Red => ratatui::style::Color::Red,
            Self::Green => ratatui::style::Color::Green,
            Self::Yellow => ratatui::style::Color::Yellow,
            Self::Blue => ratatui::style::Color::Blue,
            Self::Magenta => ratatui::style::Color::Magenta,
            Self::Cyan => ratatui::style::Color::Cyan,
            Self::White => ratatui::style::Color::White,
            Self::BrightBlack => ratatui::style::Color::DarkGray,
            Self::BrightRed => ratatui::style::Color::LightRed,
            Self::BrightGreen => ratatui::style::Color::LightGreen,
            Self::BrightYellow => ratatui::style::Color::LightYellow,
            Self::BrightBlue => ratatui::style::Color::LightBlue,
            Self::BrightMagenta => ratatui::style::Color::LightMagenta,
            Self::BrightCyan => ratatui::style::Color::LightCyan,
            Self::BrightWhite => ratatui::style::Color::Gray,
            Self::Indexed(i) => ratatui::style::Color::Indexed(i),
            Self::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
        }
    }

    /// Converts to ratatui Color using a theme palette for ANSI colors.
    #[must_use]
    pub fn to_ratatui_with_palette(
        self,
        palette: &crate::theme::colors::AnsiPalette,
    ) -> ratatui::style::Color {
        match self {
            Self::Default => ratatui::style::Color::Reset,
            Self::Black => palette.black,
            Self::Red => palette.red,
            Self::Green => palette.green,
            Self::Yellow => palette.yellow,
            Self::Blue => palette.blue,
            Self::Magenta => palette.magenta,
            Self::Cyan => palette.cyan,
            Self::White => palette.white,
            Self::BrightBlack => palette.bright_black,
            Self::BrightRed => palette.bright_red,
            Self::BrightGreen => palette.bright_green,
            Self::BrightYellow => palette.bright_yellow,
            Self::BrightBlue => palette.bright_blue,
            Self::BrightMagenta => palette.bright_magenta,
            Self::BrightCyan => palette.bright_cyan,
            Self::BrightWhite => palette.bright_white,
            Self::Indexed(i) => ratatui::style::Color::Indexed(i),
            Self::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
        }
    }
}

/// Text attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Attr {
    /// Bold text.
    Bold,
    /// Dim/faint text.
    Dim,
    /// Italic text.
    Italic,
    /// Underlined text.
    Underline,
    /// Blinking text.
    Blink,
    /// Reverse video (swap fg/bg).
    Reverse,
    /// Hidden/invisible text.
    Hidden,
    /// Strikethrough text.
    Strikethrough,
}

/// Combined style for a terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Foreground color.
    fg: Option<Color>,
    /// Background color.
    bg: Option<Color>,
    /// Attribute flags.
    attrs: u8,
}

impl Style {
    /// Bit flags for attributes.
    const BOLD: u8 = 1 << 0;
    const DIM: u8 = 1 << 1;
    const ITALIC: u8 = 1 << 2;
    const UNDERLINE: u8 = 1 << 3;
    const BLINK: u8 = 1 << 4;
    const REVERSE: u8 = 1 << 5;
    const HIDDEN: u8 = 1 << 6;
    const STRIKETHROUGH: u8 = 1 << 7;

    /// Creates a new empty style.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            attrs: 0,
        }
    }

    /// Sets the foreground color.
    #[must_use]
    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Sets the background color.
    #[must_use]
    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Adds an attribute.
    #[must_use]
    pub const fn add_attr(mut self, attr: Attr) -> Self {
        self.attrs |= Self::attr_to_bit(attr);
        self
    }

    /// Removes an attribute.
    #[must_use]
    pub const fn remove_attr(mut self, attr: Attr) -> Self {
        self.attrs &= !Self::attr_to_bit(attr);
        self
    }

    /// Checks if an attribute is set.
    #[must_use]
    pub const fn has_attr(&self, attr: Attr) -> bool {
        (self.attrs & Self::attr_to_bit(attr)) != 0
    }

    /// Gets the foreground color.
    #[must_use]
    pub const fn fg_color(&self) -> Option<Color> {
        self.fg
    }

    /// Gets the background color.
    #[must_use]
    pub const fn bg_color(&self) -> Option<Color> {
        self.bg
    }

    /// Resets all attributes.
    #[must_use]
    pub const fn reset_attrs(mut self) -> Self {
        self.attrs = 0;
        self
    }

    /// Resets all style (colors and attributes).
    #[must_use]
    pub const fn reset(self) -> Self {
        Self::new()
    }

    /// Converts attribute to bit flag.
    const fn attr_to_bit(attr: Attr) -> u8 {
        match attr {
            Attr::Bold => Self::BOLD,
            Attr::Dim => Self::DIM,
            Attr::Italic => Self::ITALIC,
            Attr::Underline => Self::UNDERLINE,
            Attr::Blink => Self::BLINK,
            Attr::Reverse => Self::REVERSE,
            Attr::Hidden => Self::HIDDEN,
            Attr::Strikethrough => Self::STRIKETHROUGH,
        }
    }

    /// Converts to ratatui Style for rendering.
    ///
    /// Note: Always sets explicit fg/bg colors to ensure consistent rendering
    /// across all platforms, especially Windows where Color::Reset can behave
    /// inconsistently.
    #[must_use]
    pub fn to_ratatui(self) -> ratatui::style::Style {
        let mut style = ratatui::style::Style::default();

        // Always set explicit colors - use Reset for None to ensure consistent behavior
        // On Windows, not setting colors can cause ghosting and color artifacts
        let fg_color = self
            .fg
            .map(|c| c.to_ratatui())
            .unwrap_or(ratatui::style::Color::Reset);
        let bg_color = self
            .bg
            .map(|c| c.to_ratatui())
            .unwrap_or(ratatui::style::Color::Reset);
        style = style.fg(fg_color).bg(bg_color);

        self.apply_modifiers(style)
    }

    /// Converts to ratatui Style using a theme palette for ANSI colors.
    /// Also takes default fg/bg colors to use when the cell has no explicit color.
    #[must_use]
    pub fn to_ratatui_with_palette(
        self,
        palette: &crate::theme::colors::AnsiPalette,
    ) -> ratatui::style::Style {
        self.to_ratatui_with_palette_and_defaults(palette, None, None)
    }

    /// Converts to ratatui Style using theme palette and default colors.
    /// When fg/bg is Default/None, uses the provided default colors instead of Reset.
    #[must_use]
    pub fn to_ratatui_with_palette_and_defaults(
        self,
        palette: &crate::theme::colors::AnsiPalette,
        default_fg: Option<ratatui::style::Color>,
        default_bg: Option<ratatui::style::Color>,
    ) -> ratatui::style::Style {
        let mut style = ratatui::style::Style::default();

        let fg_color = match self.fg {
            Some(Color::Default) | None => default_fg.unwrap_or(ratatui::style::Color::Reset),
            Some(c) => c.to_ratatui_with_palette(palette),
        };
        let bg_color = match self.bg {
            Some(Color::Default) | None => default_bg.unwrap_or(ratatui::style::Color::Reset),
            Some(c) => c.to_ratatui_with_palette(palette),
        };
        style = style.fg(fg_color).bg(bg_color);

        self.apply_modifiers(style)
    }

    /// Applies text modifiers to a ratatui style.
    fn apply_modifiers(self, mut style: ratatui::style::Style) -> ratatui::style::Style {
        if self.has_attr(Attr::Bold) {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        if self.has_attr(Attr::Dim) {
            style = style.add_modifier(ratatui::style::Modifier::DIM);
        }
        if self.has_attr(Attr::Italic) {
            style = style.add_modifier(ratatui::style::Modifier::ITALIC);
        }
        if self.has_attr(Attr::Underline) {
            style = style.add_modifier(ratatui::style::Modifier::UNDERLINED);
        }
        if self.has_attr(Attr::Blink) {
            style = style.add_modifier(ratatui::style::Modifier::SLOW_BLINK);
        }
        if self.has_attr(Attr::Reverse) {
            style = style.add_modifier(ratatui::style::Modifier::REVERSED);
        }
        if self.has_attr(Attr::Hidden) {
            style = style.add_modifier(ratatui::style::Modifier::HIDDEN);
        }
        if self.has_attr(Attr::Strikethrough) {
            style = style.add_modifier(ratatui::style::Modifier::CROSSED_OUT);
        }

        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_default() {
        assert_eq!(Color::default(), Color::Default);
    }

    #[test]
    fn test_style_new() {
        let style = Style::new();
        assert_eq!(style.fg_color(), None);
        assert_eq!(style.bg_color(), None);
        assert!(!style.has_attr(Attr::Bold));
    }

    #[test]
    fn test_style_with_colors() {
        let style = Style::new().fg(Color::Red).bg(Color::Blue);
        assert_eq!(style.fg_color(), Some(Color::Red));
        assert_eq!(style.bg_color(), Some(Color::Blue));
    }

    #[test]
    fn test_style_with_attrs() {
        let style = Style::new().add_attr(Attr::Bold).add_attr(Attr::Underline);
        assert!(style.has_attr(Attr::Bold));
        assert!(style.has_attr(Attr::Underline));
        assert!(!style.has_attr(Attr::Italic));
    }

    #[test]
    fn test_style_remove_attr() {
        let style = Style::new()
            .add_attr(Attr::Bold)
            .add_attr(Attr::Italic)
            .remove_attr(Attr::Bold);
        assert!(!style.has_attr(Attr::Bold));
        assert!(style.has_attr(Attr::Italic));
    }
}
