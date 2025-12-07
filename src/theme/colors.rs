//! Theme color definitions and parsing.
//!
//! Provides color types and parsing for hex colors and named colors.

use ratatui::style::Color;

/// Parse a color string into a ratatui Color.
///
/// Supports:
/// - Hex colors: `#RRGGBB` or `#RGB`
/// - Named colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`
/// - Bright variants: `bright_black`, `bright_red`, etc.
/// - ANSI index: `color0` through `color255`
///
/// # Examples
/// ```
/// use ratterm::theme::colors::parse_color;
///
/// let color = parse_color("#ff0000").unwrap();
/// let color = parse_color("red").unwrap();
/// ```
pub fn parse_color(s: &str) -> Option<Color> {
    assert!(!s.is_empty(), "Color string must not be empty");

    let s = s.trim().to_lowercase();

    // Hex color
    if s.starts_with('#') {
        return parse_hex_color(&s);
    }

    // Named colors
    match s.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "lightred" | "light_red" | "bright_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" | "bright_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" | "bright_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" | "bright_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" | "bright_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" | "bright_cyan" => Some(Color::LightCyan),
        "reset" | "default" => Some(Color::Reset),
        _ => {
            // Try ANSI indexed color (color0-color255)
            if let Some(idx) = s.strip_prefix("color") {
                if let Ok(n) = idx.parse::<u8>() {
                    return Some(Color::Indexed(n));
                }
            }
            None
        }
    }
}

/// Parse a hex color string (#RRGGBB or #RGB).
fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;

    match hex.len() {
        3 => {
            // #RGB format - expand to #RRGGBB
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color::Rgb(r, g, b))
        }
        6 => {
            // #RRGGBB format
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Convert a ratatui Color to a hex string.
pub fn color_to_hex(color: Color) -> Option<String> {
    match color {
        Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
        Color::Black => Some("#000000".to_string()),
        Color::Red => Some("#cc0000".to_string()),
        Color::Green => Some("#00cc00".to_string()),
        Color::Yellow => Some("#cccc00".to_string()),
        Color::Blue => Some("#0000cc".to_string()),
        Color::Magenta => Some("#cc00cc".to_string()),
        Color::Cyan => Some("#00cccc".to_string()),
        Color::White => Some("#cccccc".to_string()),
        Color::Gray => Some("#808080".to_string()),
        Color::DarkGray => Some("#404040".to_string()),
        Color::LightRed => Some("#ff0000".to_string()),
        Color::LightGreen => Some("#00ff00".to_string()),
        Color::LightYellow => Some("#ffff00".to_string()),
        Color::LightBlue => Some("#0000ff".to_string()),
        Color::LightMagenta => Some("#ff00ff".to_string()),
        Color::LightCyan => Some("#00ffff".to_string()),
        _ => None,
    }
}

/// ANSI 16-color palette for terminal themes.
#[derive(Debug, Clone)]
pub struct AnsiPalette {
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
}

impl Default for AnsiPalette {
    fn default() -> Self {
        Self {
            black: Color::Black,
            red: Color::Red,
            green: Color::Green,
            yellow: Color::Yellow,
            blue: Color::Blue,
            magenta: Color::Magenta,
            cyan: Color::Cyan,
            white: Color::White,
            bright_black: Color::DarkGray,
            bright_red: Color::LightRed,
            bright_green: Color::LightGreen,
            bright_yellow: Color::LightYellow,
            bright_blue: Color::LightBlue,
            bright_magenta: Color::LightMagenta,
            bright_cyan: Color::LightCyan,
            bright_white: Color::White,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_6digit() {
        let color = parse_color("#ff0000").unwrap();
        assert_eq!(color, Color::Rgb(255, 0, 0));

        let color = parse_color("#00ff00").unwrap();
        assert_eq!(color, Color::Rgb(0, 255, 0));

        let color = parse_color("#0000ff").unwrap();
        assert_eq!(color, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_3digit() {
        let color = parse_color("#f00").unwrap();
        assert_eq!(color, Color::Rgb(255, 0, 0));

        let color = parse_color("#0f0").unwrap();
        assert_eq!(color, Color::Rgb(0, 255, 0));
    }

    #[test]
    fn test_parse_named_colors() {
        assert_eq!(parse_color("red").unwrap(), Color::Red);
        assert_eq!(parse_color("green").unwrap(), Color::Green);
        assert_eq!(parse_color("blue").unwrap(), Color::Blue);
        assert_eq!(parse_color("white").unwrap(), Color::White);
        assert_eq!(parse_color("black").unwrap(), Color::Black);
    }

    #[test]
    fn test_parse_bright_colors() {
        assert_eq!(parse_color("bright_red").unwrap(), Color::LightRed);
        assert_eq!(parse_color("light_green").unwrap(), Color::LightGreen);
    }

    #[test]
    fn test_parse_indexed_color() {
        assert_eq!(parse_color("color0").unwrap(), Color::Indexed(0));
        assert_eq!(parse_color("color255").unwrap(), Color::Indexed(255));
    }

    #[test]
    fn test_color_to_hex() {
        assert_eq!(color_to_hex(Color::Rgb(255, 0, 0)).unwrap(), "#ff0000");
        assert_eq!(color_to_hex(Color::Red).unwrap(), "#cc0000");
    }
}
