//! The find bar drawn along the bottom of the editor pane.
//!
//! The text is assembled by [`bar_text`], which is a pure function of the
//! search state, so what the bar says can be asserted without rendering.

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use unicode_width::UnicodeWidthChar;

use crate::editor::search::{SearchField, SearchState};

use super::style::Palette;

/// Returns the line the find bar shows.
///
/// The focused field is marked with a caret so it is obvious which one typing
/// goes into, and the count is always present once a query exists.
#[must_use]
pub fn bar_text(search: &SearchState) -> String {
    let mut parts = Vec::new();

    let query_caret = if search.field() == SearchField::Query {
        "\u{2502}"
    } else {
        ""
    };
    parts.push(format!("Find: {}{}", search.query(), query_caret));

    if search.is_replacing() {
        let replace_caret = if search.field() == SearchField::Replacement {
            "\u{2502}"
        } else {
            ""
        };
        parts.push(format!(
            "Replace: {}{}",
            search.replacement(),
            replace_caret
        ));
    }

    let count = search.count_label();
    if !count.is_empty() {
        parts.push(count);
    }
    if search.wrapped() {
        parts.push("wrapped".to_string());
    }
    if search.is_case_sensitive() {
        parts.push("Aa".to_string());
    }

    parts.join("  ")
}

/// Returns the hint shown on the right of the bar.
#[must_use]
pub fn hint_text(search: &SearchState) -> &'static str {
    if search.is_replacing() {
        "Enter next  Shift+Enter prev  Ctrl+Enter replace  Alt+Enter all  Tab field  Esc close"
    } else {
        "Enter next  Shift+Enter prev  Alt+C case  Esc close"
    }
}

/// Renders the bar into the last row of `area`.
pub fn render(search: &SearchState, palette: &Palette, area: Rect, buf: &mut RatatuiBuffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y + area.height - 1;
    let background = Style::default()
        .fg(Color::Rgb(230, 230, 230))
        .bg(Color::Rgb(58, 58, 58));

    for x in area.x..area.x + area.width {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(' ');
            cell.set_style(background);
        }
    }

    let text = bar_text(search);
    let mut x = area.x;
    for c in text.chars() {
        if x >= area.x + area.width {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(c);
            cell.set_style(background.add_modifier(Modifier::BOLD));
        }
        x += u16::try_from(c.width().unwrap_or(1)).unwrap_or(1);
    }

    // The hint fills the right-hand side when there is room for it.
    let hint = hint_text(search);
    let hint_len = u16::try_from(hint.chars().count()).unwrap_or(u16::MAX);
    if area.width > hint_len + (x - area.x) + 2 {
        let start = area.x + area.width - hint_len;
        let style = Style::default()
            .fg(Color::Rgb(150, 150, 150))
            .bg(palette.background);
        for (hx, c) in (start..area.x + area.width).zip(hint.chars()) {
            if let Some(cell) = buf.cell_mut((hx, y)) {
                cell.set_char(c);
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::buffer::{Buffer, Position};

    fn searched(text: &str, query: &str, replacing: bool) -> SearchState {
        let buffer = Buffer::from_str(text);
        let mut search = SearchState::new();
        search.open(replacing);
        search.set_query(query);
        search.refresh(&buffer);
        search.select_from(Position::new(0, 0));
        search
    }

    #[test]
    fn the_bar_shows_the_query_and_the_count() {
        let search = searched("cat cat dog", "cat", false);
        let text = bar_text(&search);
        assert!(text.contains("Find: cat"));
        assert!(text.contains("1/2"), "text was {text:?}");
        assert!(!text.contains("Replace"));
    }

    #[test]
    fn zero_matches_are_shown_as_zero_of_zero() {
        let search = searched("hello", "zzz", false);
        assert!(bar_text(&search).contains("0/0"));
    }

    #[test]
    fn the_replace_field_appears_only_in_replace_mode() {
        let mut search = searched("cat", "cat", true);
        search.set_replacement("dog");
        let text = bar_text(&search);
        assert!(text.contains("Replace: dog"));
    }

    #[test]
    fn the_focused_field_is_marked() {
        let mut search = searched("cat", "cat", true);
        assert!(bar_text(&search).contains("Find: cat\u{2502}"));
        search.toggle_field();
        let text = bar_text(&search);
        assert!(text.contains("Replace: \u{2502}"));
        assert!(!text.contains("cat\u{2502}"));
    }

    #[test]
    fn wrapping_and_case_sensitivity_are_announced() {
        let mut search = searched("a a", "a", false);
        search.next_match();
        search.next_match();
        assert!(search.wrapped());
        assert!(bar_text(&search).contains("wrapped"));

        search.set_case_sensitive(true);
        assert!(bar_text(&search).contains("Aa"));
    }

    #[test]
    fn an_empty_query_shows_no_count() {
        let search = searched("abc", "", false);
        let text = bar_text(&search);
        assert!(text.starts_with("Find: "));
        assert!(!text.contains('/'));
    }

    #[test]
    fn the_hint_lists_the_replace_keys_only_in_replace_mode() {
        let plain = searched("a", "a", false);
        let replacing = searched("a", "a", true);
        assert!(!hint_text(&plain).contains("replace"));
        assert!(hint_text(&replacing).contains("replace"));
    }

    #[test]
    fn rendering_writes_the_bar_into_the_last_row() {
        let search = searched("cat cat", "cat", false);
        let palette = Palette::resolve(None);
        let area = Rect::new(0, 0, 60, 5);
        let mut buf = RatatuiBuffer::empty(area);
        render(&search, &palette, area, &mut buf);

        let row: String = (0..60)
            .map(|x| {
                buf.cell((x, 4))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect();
        assert!(row.contains("Find: cat"), "row was {row:?}");
        assert!(row.contains("1/2"), "row was {row:?}");
    }

    #[test]
    fn rendering_into_no_space_is_a_no_op() {
        let search = searched("a", "a", false);
        let palette = Palette::resolve(None);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 10, 1));
        render(&search, &palette, Rect::new(0, 0, 0, 0), &mut buf);
        assert_eq!(buf.cell((0, 0)).expect("cell").symbol(), " ");
    }
}
