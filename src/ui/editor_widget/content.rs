//! Painting the text area.
//!
//! Every decision about what a character *is* was already made by
//! [`Editor::screen_decor`](crate::editor::Editor::screen_decor); this file
//! only turns that into cells, handling tabs, wide characters, and horizontal
//! scrolling.

use ratatui::{buffer::Buffer as RatatuiBuffer, layout::Rect, style::Style};
use unicode_width::UnicodeWidthChar;

use crate::editor::Editor;
use crate::editor::decor::LineDecor;

use super::style::Palette;

/// Columns one tab expands to.
pub const TAB_WIDTH: usize = 4;

/// Writes `text` starting at `x`, stopping at `limit`.
fn put_str(buf: &mut RatatuiBuffer, x: u16, y: u16, limit: u16, text: &str, style: Style) {
    let mut at = x;
    for c in text.chars() {
        if at >= limit {
            break;
        }
        if let Some(cell) = buf.cell_mut((at, y)) {
            cell.set_char(c);
            cell.set_style(style);
        }
        at += u16::try_from(c.width().unwrap_or(1)).unwrap_or(1);
    }
}

/// Renders the visible text of the buffer.
pub fn render(editor: &Editor, palette: &Palette, area: Rect, buf: &mut RatatuiBuffer) {
    let view = editor.view();
    let gutter_width = view.gutter_width();
    let text_x = area.x + u16::try_from(gutter_width).unwrap_or(0) + 1;
    let text_width = area
        .width
        .saturating_sub(u16::try_from(gutter_width).unwrap_or(0) + 1);
    if text_width == 0 {
        return;
    }
    let limit = text_x + text_width;
    let scroll_left = view.scroll_left();

    for (row, decor) in editor.screen_decor().into_iter().enumerate() {
        if row >= area.height as usize {
            break;
        }
        let y = area.y + u16::try_from(row).unwrap_or(u16::MAX);
        let end_col = render_line(editor, palette, &decor, text_x, y, limit, scroll_left, buf);

        if decor.is_folded() {
            put_str(
                buf,
                end_col,
                y,
                limit,
                &decor.fold_marker(),
                palette.fold_marker_style(),
            );
        }
    }
}

/// Renders one line, returning the screen column just past its last character.
#[allow(clippy::too_many_arguments)]
fn render_line(
    editor: &Editor,
    palette: &Palette,
    decor: &LineDecor,
    text_x: u16,
    y: u16,
    limit: u16,
    scroll_left: usize,
    buf: &mut RatatuiBuffer,
) -> u16 {
    let text = editor.buffer().line_text(decor.line);
    let mut visual_col = 0usize;
    let mut last_x = text_x;

    for (col_idx, c) in text.chars().enumerate() {
        let char_width = if c == '\t' {
            TAB_WIDTH - (visual_col % TAB_WIDTH)
        } else {
            c.width().unwrap_or(1)
        };

        if visual_col + char_width <= scroll_left {
            visual_col += char_width;
            continue;
        }

        let screen_col = visual_col.saturating_sub(scroll_left);
        if screen_col >= (limit - text_x) as usize {
            break;
        }

        let style = palette.cell_style(decor.syntax_at(col_idx), decor.role_at(col_idx));
        let x = text_x + u16::try_from(screen_col).unwrap_or(u16::MAX);

        // A tab paints as spaces; a wide character pads the cells it covers.
        let glyph = if c == '\t' { ' ' } else { c };
        if x < limit
            && let Some(cell) = buf.cell_mut((x, y))
        {
            cell.set_char(glyph);
            cell.set_style(style);
        }
        for i in 1..char_width {
            let pad = x + u16::try_from(i).unwrap_or(0);
            if pad < limit
                && let Some(cell) = buf.cell_mut((pad, y))
            {
                cell.set_char(' ');
                cell.set_style(style);
            }
        }

        visual_col += char_width;
        last_x = (x + u16::try_from(char_width).unwrap_or(1)).min(limit);
    }

    last_x
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn blank(width: u16, height: u16) -> RatatuiBuffer {
        RatatuiBuffer::empty(Rect::new(0, 0, width, height))
    }

    #[test]
    fn put_str_stops_at_the_limit() {
        let mut buf = blank(10, 1);
        put_str(&mut buf, 0, 0, 4, "abcdef", Style::default());
        let row: String = (0..10)
            .map(|x| {
                buf.cell((x, 0))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect();
        assert_eq!(row.trim_end(), "abcd");
    }

    #[test]
    fn a_tab_paints_as_spaces_to_the_next_stop() {
        let mut editor = Editor::new(40, 5);
        editor.insert_str("\tx\n");
        let palette = Palette::resolve(None);
        let mut buf = blank(40, 5);
        render(&editor, &palette, Rect::new(0, 0, 40, 5), &mut buf);

        let gutter = editor.view().gutter_width() as u16 + 1;
        for offset in 0..TAB_WIDTH as u16 {
            let cell = buf.cell((gutter + offset, 0)).expect("cell");
            assert_eq!(cell.symbol(), " ");
        }
        assert_eq!(
            buf.cell((gutter + TAB_WIDTH as u16, 0))
                .expect("cell")
                .symbol(),
            "x"
        );
    }

    #[test]
    fn horizontal_scrolling_drops_the_characters_to_the_left() {
        let mut editor = Editor::new(20, 3);
        editor.insert_str("abcdefghijklmnop\n");
        editor.view_mut().scroll_right_by(4);
        let palette = Palette::resolve(None);
        let mut buf = blank(20, 3);
        render(&editor, &palette, Rect::new(0, 0, 20, 3), &mut buf);

        let gutter = editor.view().gutter_width() as u16 + 1;
        assert_eq!(buf.cell((gutter, 0)).expect("cell").symbol(), "e");
    }

    #[test]
    fn a_folded_line_shows_a_marker_after_its_text() {
        let mut editor = Editor::new(60, 8);
        editor.insert_str("fn f() {\n    g();\n    h();\n}\n");
        editor.set_language(crate::editor::highlight::Language::Rust);
        editor.set_cursor_position(crate::editor::Position::new(0, 0));
        assert!(editor.toggle_fold());

        let palette = Palette::resolve(None);
        let mut buf = blank(60, 8);
        render(&editor, &palette, Rect::new(0, 0, 60, 8), &mut buf);

        let row: String = (0..60)
            .map(|x| {
                buf.cell((x, 0))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect();
        assert!(row.contains('⋯'), "expected a fold marker in {row:?}");
        assert!(row.contains("3 lines"), "row was {row:?}");
    }

    #[test]
    fn a_keyword_is_painted_in_its_own_colour() {
        let mut editor = Editor::new(40, 4);
        editor.insert_str("fn main() {}\n");
        editor.set_language(crate::editor::highlight::Language::Rust);
        let palette = Palette::resolve(None);
        let mut buf = blank(40, 4);
        render(&editor, &palette, Rect::new(0, 0, 40, 4), &mut buf);

        let gutter = editor.view().gutter_width() as u16 + 1;
        let cell = buf.cell((gutter, 0)).expect("cell");
        assert_eq!(cell.symbol(), "f");
        assert_eq!(
            cell.style().fg,
            Some(super::super::style::syntax_color(
                crate::editor::highlight::HighlightKind::Keyword
            ))
        );
    }

    #[test]
    fn a_zero_width_text_area_paints_nothing() {
        // Compared against an untouched buffer rather than against a specific
        // default style: what "paints nothing" means is that the buffer did
        // not change, and an empty ratatui buffer's cells are not styleless.
        let editor = Editor::new(40, 4);
        let palette = Palette::resolve(None);
        let untouched = blank(40, 4);
        let mut buf = blank(40, 4);

        // One column of area: the gutter alone leaves no room for text.
        render(&editor, &palette, Rect::new(0, 0, 1, 4), &mut buf);

        assert_eq!(buf, untouched, "a zero-width text area painted something");
    }
}
