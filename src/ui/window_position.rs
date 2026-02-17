//! Window positioning system for popups and overlays.
//!
//! Supports two positioning modes:
//! - **Grid positions:** A 3x3 named grid (e.g., `top-left`, `middle-center`)
//! - **Pixel offsets:** Absolute distance from top-left in cells (e.g., `"150 x 80"`)
//!
//! Configured in `.ratrc`:
//! ```text
//! # Grid positions: top-left, top-center, top-right,
//! #                 middle-left, middle-center, middle-right,
//! #                 bottom-left, bottom-center, bottom-right
//! hotkey_overlay_position = middle-center
//! ssh_manager_position = middle-center
//!
//! # Pixel offsets (cell coordinates from top-left corner):
//! # hotkey_overlay_position = 150 x 80
//! ```

use ratatui::layout::Rect;

/// Margin from screen edges for grid positions (in cells).
const GRID_MARGIN: u16 = 2;

/// Named position in a 3x3 grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridPosition {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    #[default]
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Window position specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowPosition {
    /// Named grid position.
    Grid(GridPosition),
    /// Absolute cell offset from top-left (x, y).
    Offset(u16, u16),
}

impl Default for WindowPosition {
    fn default() -> Self {
        Self::Grid(GridPosition::MiddleCenter)
    }
}

impl WindowPosition {
    /// Parses a position string.
    ///
    /// Accepts:
    /// - Grid names: `"top-left"`, `"middle_center"`, `"BOTTOM RIGHT"` (case-insensitive)
    /// - Pixel offsets: `"150 x 80"`, `"150x80"`, `"0 x 0"`
    ///
    /// # Errors
    /// Returns an error message if the string cannot be parsed.
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("Empty position string".to_string());
        }

        // Check for offset format: contains 'x' with digits on both sides
        if trimmed.contains('x') || trimmed.contains('X') {
            return Self::parse_offset(trimmed);
        }

        // Try grid name
        Self::parse_grid(trimmed)
    }

    /// Parses an offset string like "150 x 80" or "150x80".
    fn parse_offset(s: &str) -> Result<Self, String> {
        // Split on 'x' or 'X'
        let parts: Vec<&str> = s.splitn(2, ['x', 'X']).collect();
        if parts.len() != 2 {
            return Err(format!("Invalid offset format '{s}', expected 'X x Y'"));
        }

        let x: u16 = parts[0]
            .trim()
            .parse()
            .map_err(|_| format!("Invalid X coordinate in '{s}'"))?;
        let y: u16 = parts[1]
            .trim()
            .parse()
            .map_err(|_| format!("Invalid Y coordinate in '{s}'"))?;

        Ok(Self::Offset(x, y))
    }

    /// Parses a grid position name (case-insensitive, accepts `-`, `_`, or space).
    fn parse_grid(s: &str) -> Result<Self, String> {
        // Normalize: lowercase, replace separators with '-'
        let normalized = s.to_lowercase().replace(['_', ' '], "-").trim().to_string();

        match normalized.as_str() {
            "top-left" => Ok(Self::Grid(GridPosition::TopLeft)),
            "top-center" => Ok(Self::Grid(GridPosition::TopCenter)),
            "top-right" => Ok(Self::Grid(GridPosition::TopRight)),
            "middle-left" => Ok(Self::Grid(GridPosition::MiddleLeft)),
            "middle-center" => Ok(Self::Grid(GridPosition::MiddleCenter)),
            "middle-right" => Ok(Self::Grid(GridPosition::MiddleRight)),
            "bottom-left" => Ok(Self::Grid(GridPosition::BottomLeft)),
            "bottom-center" => Ok(Self::Grid(GridPosition::BottomCenter)),
            "bottom-right" => Ok(Self::Grid(GridPosition::BottomRight)),
            _ => Err(format!(
                "Unknown position '{s}'. Expected grid name (e.g., top-left, middle-center) \
                 or offset (e.g., 150 x 80)"
            )),
        }
    }

    /// Resolves this position to a concrete `Rect` given popup and screen dimensions.
    ///
    /// For grid positions, applies a margin from screen edges.
    /// For offsets, clamps to keep the popup visible on screen.
    #[must_use]
    pub fn resolve(
        &self,
        popup_width: u16,
        popup_height: u16,
        screen_width: u16,
        screen_height: u16,
    ) -> Rect {
        // Ensure popup fits in screen
        let pw = popup_width.min(screen_width);
        let ph = popup_height.min(screen_height);

        let (x, y) = match self {
            Self::Grid(grid) => Self::resolve_grid(*grid, pw, ph, screen_width, screen_height),
            Self::Offset(ox, oy) => {
                Self::resolve_offset(*ox, *oy, pw, ph, screen_width, screen_height)
            }
        };

        Rect::new(x, y, pw, ph)
    }

    /// Resolves a grid position to (x, y) coordinates.
    fn resolve_grid(grid: GridPosition, pw: u16, ph: u16, sw: u16, sh: u16) -> (u16, u16) {
        let margin = GRID_MARGIN;

        let x = match grid {
            GridPosition::TopLeft | GridPosition::MiddleLeft | GridPosition::BottomLeft => margin,
            GridPosition::TopCenter | GridPosition::MiddleCenter | GridPosition::BottomCenter => {
                sw.saturating_sub(pw) / 2
            }
            GridPosition::TopRight | GridPosition::MiddleRight | GridPosition::BottomRight => {
                sw.saturating_sub(pw).saturating_sub(margin)
            }
        };

        let y = match grid {
            GridPosition::TopLeft | GridPosition::TopCenter | GridPosition::TopRight => margin,
            GridPosition::MiddleLeft | GridPosition::MiddleCenter | GridPosition::MiddleRight => {
                sh.saturating_sub(ph) / 2
            }
            GridPosition::BottomLeft | GridPosition::BottomCenter | GridPosition::BottomRight => {
                sh.saturating_sub(ph).saturating_sub(margin)
            }
        };

        (x, y)
    }

    /// Resolves an offset position, clamped to screen bounds.
    fn resolve_offset(ox: u16, oy: u16, pw: u16, ph: u16, sw: u16, sh: u16) -> (u16, u16) {
        let x = ox.min(sw.saturating_sub(pw));
        let y = oy.min(sh.saturating_sub(ph));
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Parsing tests
    // ========================================================================

    #[test]
    fn test_parse_grid_top_left() {
        assert_eq!(
            WindowPosition::parse("top-left"),
            Ok(WindowPosition::Grid(GridPosition::TopLeft))
        );
    }

    #[test]
    fn test_parse_grid_middle_center() {
        assert_eq!(
            WindowPosition::parse("middle-center"),
            Ok(WindowPosition::Grid(GridPosition::MiddleCenter))
        );
    }

    #[test]
    fn test_parse_grid_underscore() {
        assert_eq!(
            WindowPosition::parse("bottom_right"),
            Ok(WindowPosition::Grid(GridPosition::BottomRight))
        );
    }

    #[test]
    fn test_parse_grid_space() {
        assert_eq!(
            WindowPosition::parse("top center"),
            Ok(WindowPosition::Grid(GridPosition::TopCenter))
        );
    }

    #[test]
    fn test_parse_grid_case_insensitive() {
        assert_eq!(
            WindowPosition::parse("MIDDLE-LEFT"),
            Ok(WindowPosition::Grid(GridPosition::MiddleLeft))
        );
    }

    #[test]
    fn test_parse_offset() {
        assert_eq!(
            WindowPosition::parse("150 x 80"),
            Ok(WindowPosition::Offset(150, 80))
        );
    }

    #[test]
    fn test_parse_offset_no_space() {
        assert_eq!(
            WindowPosition::parse("150x80"),
            Ok(WindowPosition::Offset(150, 80))
        );
    }

    #[test]
    fn test_parse_offset_zero() {
        assert_eq!(
            WindowPosition::parse("0 x 0"),
            Ok(WindowPosition::Offset(0, 0))
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert!(WindowPosition::parse("nowhere").is_err());
    }

    #[test]
    fn test_parse_empty() {
        assert!(WindowPosition::parse("").is_err());
    }

    // ========================================================================
    // Resolution tests
    // ========================================================================

    #[test]
    fn test_resolve_middle_center() {
        let pos = WindowPosition::Grid(GridPosition::MiddleCenter);
        let rect = pos.resolve(50, 20, 100, 40);
        assert_eq!(rect, Rect::new(25, 10, 50, 20));
    }

    #[test]
    fn test_resolve_top_left() {
        let pos = WindowPosition::Grid(GridPosition::TopLeft);
        let rect = pos.resolve(50, 20, 100, 40);
        assert_eq!(rect, Rect::new(2, 2, 50, 20));
    }

    #[test]
    fn test_resolve_bottom_right() {
        let pos = WindowPosition::Grid(GridPosition::BottomRight);
        let rect = pos.resolve(50, 20, 100, 40);
        // x = 100 - 50 - 2 = 48, y = 40 - 20 - 2 = 18
        assert_eq!(rect, Rect::new(48, 18, 50, 20));
    }

    #[test]
    fn test_resolve_offset_within_bounds() {
        let pos = WindowPosition::Offset(10, 5);
        let rect = pos.resolve(50, 20, 100, 40);
        assert_eq!(rect, Rect::new(10, 5, 50, 20));
    }

    #[test]
    fn test_resolve_offset_clamped() {
        let pos = WindowPosition::Offset(999, 999);
        let rect = pos.resolve(50, 20, 100, 40);
        // x = min(999, 100-50) = 50, y = min(999, 40-20) = 20
        assert_eq!(rect, Rect::new(50, 20, 50, 20));
    }

    #[test]
    fn test_resolve_popup_larger_than_screen() {
        let pos = WindowPosition::Grid(GridPosition::MiddleCenter);
        let rect = pos.resolve(200, 100, 100, 40);
        // popup clamped to screen: 100x40, centered at (0, 0)
        assert_eq!(rect, Rect::new(0, 0, 100, 40));
    }

    #[test]
    fn test_default_is_middle_center() {
        assert_eq!(
            WindowPosition::default(),
            WindowPosition::Grid(GridPosition::MiddleCenter)
        );
    }

    // ========================================================================
    // All grid positions
    // ========================================================================

    #[test]
    fn test_all_grid_positions_parse() {
        let positions = [
            "top-left",
            "top-center",
            "top-right",
            "middle-left",
            "middle-center",
            "middle-right",
            "bottom-left",
            "bottom-center",
            "bottom-right",
        ];
        for name in positions {
            assert!(
                WindowPosition::parse(name).is_ok(),
                "Failed to parse '{name}'"
            );
        }
    }
}
