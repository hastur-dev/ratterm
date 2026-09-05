//! The colours the editor pane paints with.
//!
//! [`EditorTheme`] carries the pane's frame colours; the per-token colours are
//! here because the theme schema has no syntax section yet. Keeping them in one
//! table means a theme can grow one later without the renderer changing.

use ratatui::style::{Color, Modifier, Style};

use crate::editor::decor::CharRole;
use crate::editor::highlight::HighlightKind;
use crate::theme::EditorTheme;

/// Background used when no theme is supplied.
pub const FALLBACK_BG: Color = Color::Rgb(30, 30, 30);
/// Foreground used when no theme is supplied.
pub const FALLBACK_FG: Color = Color::White;

/// Returns the colour a highlight kind is drawn in.
#[must_use]
pub const fn syntax_color(kind: HighlightKind) -> Color {
    match kind {
        HighlightKind::Keyword => Color::Rgb(197, 134, 192),
        HighlightKind::Function => Color::Rgb(220, 220, 170),
        HighlightKind::Type => Color::Rgb(78, 201, 176),
        HighlightKind::String => Color::Rgb(206, 145, 120),
        HighlightKind::Number => Color::Rgb(181, 206, 168),
        HighlightKind::Comment => Color::Rgb(106, 153, 85),
        HighlightKind::Operator | HighlightKind::Punctuation => Color::Rgb(212, 212, 212),
        HighlightKind::Variable => Color::Rgb(156, 220, 254),
        HighlightKind::Constant => Color::Rgb(79, 193, 255),
        HighlightKind::Attribute => Color::Rgb(215, 186, 125),
    }
}

/// The resolved palette for one render pass.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Text colour for characters with no syntax kind.
    pub foreground: Color,
    /// Pane background.
    pub background: Color,
    /// Selection background.
    pub selection: Color,
    /// Line-number colour.
    pub line_numbers_fg: Color,
    /// Line-number background.
    pub line_numbers_bg: Color,
    /// Cursor colour, used for the current line number.
    pub cursor: Color,
    /// Border colour when the pane is not focused.
    pub border: Color,
    /// Border colour when it is.
    pub border_focused: Color,
}

impl Palette {
    /// Resolves a theme, falling back to the same colours the pane used before
    /// themes existed so an unthemed frame does not flicker.
    #[must_use]
    pub fn resolve(theme: Option<&EditorTheme>) -> Self {
        match theme {
            Some(t) => Self {
                foreground: t.foreground,
                background: t.background,
                selection: t.selection,
                line_numbers_fg: t.line_numbers_fg,
                line_numbers_bg: t.line_numbers_bg,
                cursor: t.cursor,
                border: t.border,
                border_focused: t.border_focused,
            },
            None => Self {
                foreground: FALLBACK_FG,
                background: FALLBACK_BG,
                selection: Color::Blue,
                line_numbers_fg: Color::DarkGray,
                line_numbers_bg: FALLBACK_BG,
                cursor: Color::Yellow,
                border: Color::DarkGray,
                border_focused: Color::Rgb(204, 60, 60),
            },
        }
    }

    /// Returns the style for one character.
    ///
    /// The syntax kind sets the foreground; the role layers a background or a
    /// modifier over it, so a keyword inside the selection stays a keyword.
    #[must_use]
    pub fn cell_style(&self, kind: Option<HighlightKind>, role: CharRole) -> Style {
        let fg = kind.map_or(self.foreground, syntax_color);
        let base = Style::default().fg(fg).bg(self.background);
        match role {
            CharRole::Plain => base,
            CharRole::Selected => base.bg(self.selection),
            CharRole::MatchedBracket => base
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
            CharRole::SearchMatch => base.bg(Color::Rgb(88, 74, 24)),
            CharRole::CurrentMatch => Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(219, 177, 42))
                .add_modifier(Modifier::BOLD),
            CharRole::ExtraCursor => base.add_modifier(Modifier::REVERSED),
        }
    }

    /// Returns the style a folded line's marker is drawn in.
    #[must_use]
    pub fn fold_marker_style(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(140, 140, 140))
            .bg(self.background)
            .add_modifier(Modifier::ITALIC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_highlight_kind_has_a_distinct_enough_colour() {
        let kinds = [
            HighlightKind::Keyword,
            HighlightKind::Function,
            HighlightKind::Type,
            HighlightKind::String,
            HighlightKind::Number,
            HighlightKind::Comment,
            HighlightKind::Variable,
            HighlightKind::Constant,
            HighlightKind::Attribute,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in kinds.iter().skip(i + 1) {
                assert_ne!(syntax_color(*a), syntax_color(*b), "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn an_unthemed_palette_matches_the_historic_fallbacks() {
        let palette = Palette::resolve(None);
        assert_eq!(palette.background, FALLBACK_BG);
        assert_eq!(palette.foreground, FALLBACK_FG);
    }

    #[test]
    fn a_theme_is_carried_through() {
        let theme = EditorTheme::default();
        let palette = Palette::resolve(Some(&theme));
        assert_eq!(palette.foreground, theme.foreground);
        assert_eq!(palette.selection, theme.selection);
    }

    #[test]
    fn a_syntax_kind_sets_the_foreground_and_a_role_the_background() {
        let palette = Palette::resolve(None);
        let plain = palette.cell_style(Some(HighlightKind::Keyword), CharRole::Plain);
        assert_eq!(plain.fg, Some(syntax_color(HighlightKind::Keyword)));

        let selected = palette.cell_style(Some(HighlightKind::Keyword), CharRole::Selected);
        assert_eq!(selected.fg, Some(syntax_color(HighlightKind::Keyword)));
        assert_eq!(selected.bg, Some(palette.selection));
    }

    #[test]
    fn a_matched_bracket_is_bold_and_underlined() {
        let palette = Palette::resolve(None);
        let style = palette.cell_style(None, CharRole::MatchedBracket);
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn the_current_search_hit_stands_out_from_the_others() {
        let palette = Palette::resolve(None);
        let current = palette.cell_style(None, CharRole::CurrentMatch);
        let other = palette.cell_style(None, CharRole::SearchMatch);
        assert_ne!(current.bg, other.bg);
    }

    #[test]
    fn a_secondary_cursor_is_drawn_reversed() {
        let palette = Palette::resolve(None);
        let style = palette.cell_style(None, CharRole::ExtraCursor);
        assert!(style.add_modifier.contains(Modifier::REVERSED));
    }
}
