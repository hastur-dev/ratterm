//! The line-number gutter, the git marks, and the fold arrows.

use std::collections::HashMap;

use ratatui::{
    buffer::Buffer as RatatuiBuffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::editor::Editor;
use crate::git::gutter::GutterMark;

use super::style::Palette;

/// Marker drawn beside a line that starts a collapsed region.
pub const FOLDED_MARK: char = '▸';
/// Marker drawn beside a line that starts an expanded region.
pub const FOLDABLE_MARK: char = '▾';

/// Returns the gutter marker for a line, if it starts a foldable region.
#[must_use]
pub fn fold_mark(editor: &Editor, line: usize) -> Option<char> {
    if !editor.folds().ranges().iter().any(|r| r.start_line == line) {
        return None;
    }
    Some(if editor.folds().is_collapsed(line) {
        FOLDED_MARK
    } else {
        FOLDABLE_MARK
    })
}

/// Renders the gutter and the separator column.
pub fn render(
    editor: &Editor,
    palette: &Palette,
    git: Option<&HashMap<usize, GutterMark>>,
    breakpoints: Option<&[usize]>,
    area: Rect,
    buf: &mut RatatuiBuffer,
) {
    let gutter_width = editor.view().gutter_width();
    let gutter_cols = u16::try_from(gutter_width).unwrap_or(0);
    let number_style = Style::default()
        .fg(palette.line_numbers_fg)
        .bg(palette.line_numbers_bg);
    let current_style = Style::default()
        .fg(palette.cursor)
        .bg(palette.line_numbers_bg);
    let separator_style = Style::default().fg(Color::DarkGray).bg(palette.background);

    let lines = editor.screen_lines();
    let cursor_line = editor.cursor_position().line;

    for row in 0..area.height {
        let y = area.y + row;
        for col in 0..gutter_cols {
            if let Some(cell) = buf.cell_mut((area.x + col, y)) {
                cell.set_char(' ');
                cell.set_style(number_style);
            }
        }

        let line = lines.get(row as usize).copied();

        // The separator column doubles as the git gutter.
        let mark = line.and_then(|l| git.and_then(|marks| marks.get(&l)));
        if let Some(cell) = buf.cell_mut((area.x + gutter_cols, y)) {
            match mark {
                Some(GutterMark::Added) => {
                    cell.set_char('▎');
                    cell.set_style(Style::default().fg(Color::Green).bg(palette.background));
                }
                Some(GutterMark::Modified) => {
                    cell.set_char('▎');
                    cell.set_style(Style::default().fg(Color::Yellow).bg(palette.background));
                }
                Some(GutterMark::Deleted) => {
                    cell.set_char('▁');
                    cell.set_style(Style::default().fg(Color::Red).bg(palette.background));
                }
                None => {
                    cell.set_char('│');
                    cell.set_style(separator_style);
                }
            }
        }

        let Some(line) = line else {
            // Past the end of the document.
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('~');
                cell.set_style(number_style);
            }
            continue;
        };

        let style = if line == cursor_line {
            current_style
        } else {
            number_style
        };
        let text = format!(
            "{:>width$} ",
            line + 1,
            width = gutter_width.saturating_sub(1)
        );
        for (i, c) in text.chars().enumerate() {
            if i >= gutter_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((area.x + u16::try_from(i).unwrap_or(0), y)) {
                cell.set_char(c);
                cell.set_style(style);
            }
        }

        // The first column carries a breakpoint dot, then a fold arrow.
        let has_breakpoint = breakpoints.is_some_and(|lines| lines.contains(&line));
        if has_breakpoint {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_char('\u{25CF}');
                cell.set_style(Style::default().fg(Color::Red).bg(palette.line_numbers_bg));
            }
        } else if let Some(arrow) = fold_mark(editor, line)
            && let Some(cell) = buf.cell_mut((area.x, y))
        {
            cell.set_char(arrow);
            cell.set_style(
                Style::default()
                    .fg(Color::Rgb(150, 150, 150))
                    .bg(palette.line_numbers_bg),
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::editor::Position;
    use crate::editor::highlight::Language;

    fn rust_editor(text: &str) -> Editor {
        let mut editor = Editor::new(60, 8);
        editor.insert_str(text);
        editor.set_language(Language::Rust);
        editor.set_cursor_position(Position::new(0, 0));
        editor
    }

    fn column(buf: &RatatuiBuffer, x: u16, rows: u16) -> String {
        (0..rows)
            .map(|y| {
                buf.cell((x, y))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect()
    }

    #[test]
    fn a_foldable_region_gets_an_arrow_that_flips_when_collapsed() {
        let mut editor = rust_editor("fn f() {\n    g();\n}\n");
        assert_eq!(fold_mark(&editor, 0), Some(FOLDABLE_MARK));
        assert_eq!(fold_mark(&editor, 1), None);
        editor.toggle_fold();
        assert_eq!(fold_mark(&editor, 0), Some(FOLDED_MARK));
    }

    #[test]
    fn the_gutter_numbers_the_lines_that_are_actually_drawn() {
        let mut editor = rust_editor("fn f() {\n    g();\n    h();\n}\nlet z = 1;\n");
        editor.toggle_fold();
        let palette = Palette::resolve(None);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 60, 8));
        render(
            &editor,
            &palette,
            None,
            None,
            Rect::new(0, 0, 60, 8),
            &mut buf,
        );

        // Row 0 is line 1, row 1 is line 5 because 2 to 4 are folded away.
        let numbers: Vec<String> = (0..2)
            .map(|y| {
                (0..editor.view().gutter_width() as u16)
                    .map(|x| {
                        buf.cell((x, y))
                            .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
                    })
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect();
        // The fold was just collapsed, so line 1 carries the collapsed arrow —
        // the same one `a_foldable_region_gets_an_arrow_that_flips_when_collapsed`
        // asserts after the same call.
        assert_eq!(numbers[0], format!("{FOLDED_MARK}1").trim().to_string());
        assert!(numbers[1].contains('5'), "row 1 was {:?}", numbers[1]);
    }

    #[test]
    fn lines_past_the_end_of_the_document_show_a_tilde() {
        let editor = rust_editor("one\n");
        let palette = Palette::resolve(None);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 40, 6));
        render(
            &editor,
            &palette,
            None,
            None,
            Rect::new(0, 0, 40, 6),
            &mut buf,
        );
        let first_col = column(&buf, 0, 6);
        assert!(first_col.contains('~'), "column was {first_col:?}");
    }

    #[test]
    fn a_breakpoint_wins_the_first_column_over_a_fold_arrow() {
        let editor = rust_editor("fn f() {\n    g();\n}\n");
        let palette = Palette::resolve(None);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 40, 6));
        render(
            &editor,
            &palette,
            None,
            Some(&[0]),
            Rect::new(0, 0, 40, 6),
            &mut buf,
        );
        assert_eq!(buf.cell((0, 0)).expect("cell").symbol(), "\u{25CF}");
    }

    #[test]
    fn git_marks_take_over_the_separator_column() {
        let editor = rust_editor("a\nb\n");
        let palette = Palette::resolve(None);
        let mut marks = HashMap::new();
        marks.insert(0usize, GutterMark::Added);
        marks.insert(1usize, GutterMark::Modified);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 40, 4));
        render(
            &editor,
            &palette,
            Some(&marks),
            None,
            Rect::new(0, 0, 40, 4),
            &mut buf,
        );
        let sep = editor.view().gutter_width() as u16;
        assert_eq!(buf.cell((sep, 0)).expect("cell").symbol(), "▎");
        assert_eq!(
            buf.cell((sep, 0)).expect("cell").style().fg,
            Some(Color::Green)
        );
        assert_eq!(
            buf.cell((sep, 1)).expect("cell").style().fg,
            Some(Color::Yellow)
        );
    }

    #[test]
    fn the_cursor_line_number_is_coloured_differently() {
        let mut editor = rust_editor("a\nb\nc\n");
        editor.set_cursor_position(Position::new(1, 0));
        let palette = Palette::resolve(None);
        let mut buf = RatatuiBuffer::empty(Rect::new(0, 0, 40, 4));
        render(
            &editor,
            &palette,
            None,
            None,
            Rect::new(0, 0, 40, 4),
            &mut buf,
        );
        let width = editor.view().gutter_width() as u16;
        assert_eq!(
            buf.cell((width - 2, 1)).expect("cell").style().fg,
            Some(palette.cursor)
        );
        assert_eq!(
            buf.cell((width - 2, 0)).expect("cell").style().fg,
            Some(palette.line_numbers_fg)
        );
    }
}
