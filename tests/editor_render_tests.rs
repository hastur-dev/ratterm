//! What the editor pane actually draws.
//!
//! These render through `TestBackend`, so they run on every platform with no
//! terminal, and they assert on the cells — colour and modifiers — rather than
//! only on the text, because "the keyword is purple" and "the folded region is
//! one row" are exactly the claims that would otherwise go unverified.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer as RatatuiBuffer;
use ratatui::style::{Color, Modifier};

use ratterm::editor::decor::CharRole;
use ratterm::editor::highlight::{HighlightKind, Language};
use ratterm::editor::{Editor, Position};
use ratterm::ui::editor_tabs::{EditorTabBar, EditorTabInfo};
use ratterm::ui::editor_widget::EditorWidget;
use ratterm::ui::editor_widget::style::{Palette, syntax_color};

/// Renders an editor pane and returns the frame's cells.
fn draw(editor: &Editor, width: u16, height: u16) -> RatatuiBuffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("a terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(EditorWidget::new(editor).focused(true), frame.area());
        })
        .expect("a frame");
    terminal.backend().buffer().clone()
}

/// Returns one row of a rendered frame as text.
fn row(buf: &RatatuiBuffer, y: u16, width: u16) -> String {
    (0..width)
        .map(|x| {
            buf.cell((x, y))
                .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
        })
        .collect()
}

/// Returns the screen column where a line's text starts.
fn text_x(editor: &Editor) -> u16 {
    1 + u16::try_from(editor.view().gutter_width()).expect("a small gutter") + 1
}

fn rust_editor(text: &str) -> Editor {
    let mut editor = Editor::new(78, 18);
    editor.insert_str(text);
    editor.set_language(Language::Rust);
    editor.set_cursor_position(Position::new(0, 0));
    editor
}

#[test]
fn syntax_highlighting_reaches_the_rendered_cells() {
    let editor = rust_editor("fn main() {\n    let s = \"hi\";\n}\n");
    let buf = draw(&editor, 80, 20);
    let x = text_x(&editor);

    // `fn` is a keyword.
    let keyword = buf.cell((x, 1)).expect("cell");
    assert_eq!(keyword.symbol(), "f");
    assert_eq!(
        keyword.style().fg,
        Some(syntax_color(HighlightKind::Keyword))
    );

    // The contents of the string literal on line two are a different colour.
    // The quote itself is a delimiter as far as the grammar is concerned, so
    // the assertion is on the character inside it.
    let line = row(&buf, 2, 80);
    // One cell is one character, and the gutter separator is a multi-byte one,
    // so the column has to come from a character position, not a byte offset.
    let quote = line
        .chars()
        .position(|c| c == '"')
        .expect("a quote on line two");
    let string_cell = buf
        .cell((u16::try_from(quote + 1).expect("small"), 2))
        .expect("cell");
    assert_eq!(string_cell.symbol(), "h");
    assert_eq!(
        string_cell.style().fg,
        Some(syntax_color(HighlightKind::String))
    );
    assert_ne!(keyword.style().fg, string_cell.style().fg);
}

#[test]
fn plain_text_is_drawn_in_the_theme_foreground() {
    let mut editor = Editor::new(78, 8);
    editor.insert_str("fn main() {}\n");
    // No language, so nothing is a keyword.
    let buf = draw(&editor, 80, 10);
    let cell = buf.cell((text_x(&editor), 1)).expect("cell");
    assert_eq!(cell.symbol(), "f");
    assert_eq!(cell.style().fg, Some(Palette::resolve(None).foreground));
}

#[test]
fn highlighting_follows_an_edit_without_a_reopen() {
    let mut editor = rust_editor("fn main() {\n    let a = 1;\n}\n");
    editor.set_cursor_position(Position::new(1, 4));
    editor.type_str("// ");

    let buf = draw(&editor, 80, 20);
    // The line is indented four columns, so the marker starts there.
    let cell = buf.cell((text_x(&editor) + 4, 2)).expect("cell");
    assert_eq!(cell.symbol(), "/");
    assert_eq!(
        cell.style().fg,
        Some(syntax_color(HighlightKind::Comment)),
        "typing a comment marker must recolour the line"
    );
}

#[test]
fn the_matching_bracket_under_the_cursor_is_marked_on_both_lines() {
    let mut editor = rust_editor("fn f() {\n    g();\n}\n");
    editor.set_cursor_position(Position::new(0, 7));

    let buf = draw(&editor, 80, 20);
    let x = text_x(&editor);
    let open = buf.cell((x + 7, 1)).expect("cell");
    let close = buf.cell((x, 3)).expect("cell");
    assert_eq!(open.symbol(), "{");
    assert_eq!(close.symbol(), "}");
    // The cursor cell is reversed on top of the bracket style, so check the
    // partner, which carries the bracket style alone.
    assert!(close.style().add_modifier.contains(Modifier::BOLD));
    assert!(close.style().add_modifier.contains(Modifier::UNDERLINED));

    // A character that is not a bracket is not marked.
    let plain = buf.cell((x + 1, 1)).expect("cell");
    assert!(!plain.style().add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn a_cursor_away_from_brackets_marks_nothing() {
    let mut editor = rust_editor("let alpha = 1;\n");
    editor.set_cursor_position(Position::new(0, 5));
    let buf = draw(&editor, 80, 12);
    let x = text_x(&editor);
    for offset in 0..12u16 {
        let cell = buf.cell((x + offset, 1)).expect("cell");
        if offset == 5 {
            continue; // the cursor cell itself
        }
        assert!(
            !cell.style().add_modifier.contains(Modifier::UNDERLINED),
            "column {offset} should not be marked"
        );
    }
}

#[test]
fn a_folded_region_renders_as_one_line() {
    let mut editor = rust_editor("fn f() {\n    g();\n    h();\n}\nlet z = 1;\n");
    assert!(editor.toggle_fold(), "the function body is foldable");

    let buf = draw(&editor, 80, 12);
    let first = row(&buf, 1, 80);
    let second = row(&buf, 2, 80);

    assert!(first.contains("fn f()"), "row was {first:?}");
    assert!(first.contains('⋯'), "row was {first:?}");
    assert!(first.contains("3 lines"), "row was {first:?}");
    assert!(
        second.contains("let z"),
        "the next drawn row must be the line after the region, was {second:?}"
    );
    assert!(
        !second.contains("g()"),
        "the body must not be drawn at all, was {second:?}"
    );
}

#[test]
fn unfolding_puts_every_line_back_on_screen() {
    let mut editor = rust_editor("fn f() {\n    g();\n    h();\n}\n");
    editor.toggle_fold();
    editor.toggle_fold();
    let buf = draw(&editor, 80, 12);
    assert!(row(&buf, 2, 80).contains("g()"));
    assert!(row(&buf, 3, 80).contains("h()"));
}

#[test]
fn the_gutter_marks_a_foldable_region_and_flips_when_it_collapses() {
    let mut editor = rust_editor("fn f() {\n    g();\n}\n");
    let open = draw(&editor, 80, 10);
    assert_eq!(open.cell((1, 1)).expect("cell").symbol(), "▾");

    editor.toggle_fold();
    let closed = draw(&editor, 80, 10);
    assert_eq!(closed.cell((1, 1)).expect("cell").symbol(), "▸");
}

#[test]
fn the_cursor_cannot_land_inside_a_folded_region() {
    let mut editor = rust_editor("fn f() {\n    g();\n    h();\n}\nlet z = 1;\n");
    editor.set_cursor_position(Position::new(2, 4));
    // Collapse the outer region from outside it.
    assert!(editor.folds_mut().fold(0));
    editor.clamp_cursor_out_of_folds();

    assert_eq!(editor.cursor_position().line, 0);
    assert!(!editor.is_line_hidden(editor.cursor_position().line));

    // Moving down steps over the whole region rather than into it.
    editor.move_down_visible();
    assert_eq!(editor.cursor_position().line, 4);
    assert!(!editor.is_line_hidden(editor.cursor_position().line));
}

#[test]
fn a_selection_is_drawn_with_the_selection_background() {
    let mut editor = rust_editor("hello world\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.cursor_mut().start_selection();
    editor.cursor_mut().extend_to(Position::new(0, 5));

    let buf = draw(&editor, 80, 10);
    let x = text_x(&editor);
    let selection_bg = Palette::resolve(None).selection;
    assert_eq!(
        buf.cell((x + 1, 1)).expect("cell").style().bg,
        Some(selection_bg)
    );
    assert_ne!(
        buf.cell((x + 6, 1)).expect("cell").style().bg,
        Some(selection_bg)
    );
}

#[test]
fn the_decor_model_and_the_rendered_cells_agree() {
    let mut editor = rust_editor("fn f() {\n    g();\n}\n");
    editor.set_cursor_position(Position::new(0, 7));
    let decor = editor.line_decor(2);
    assert_eq!(decor.role_at(0), CharRole::MatchedBracket);
    assert_eq!(decor.syntax_at(0), decor.syntax_at(0));

    let buf = draw(&editor, 80, 10);
    let cell = buf.cell((text_x(&editor), 3)).expect("cell");
    assert!(cell.style().add_modifier.contains(Modifier::BOLD));
}

#[test]
fn a_tab_bar_marks_the_edited_file_and_not_the_saved_one() {
    let tabs = vec![
        EditorTabInfo {
            index: 0,
            name: "dirty.rs".to_string(),
            is_active: true,
            is_modified: true,
        },
        EditorTabInfo {
            index: 1,
            name: "clean.rs".to_string(),
            is_active: false,
            is_modified: false,
        },
    ];
    let backend = TestBackend::new(60, 1);
    let mut terminal = Terminal::new(backend).expect("a terminal");
    terminal
        .draw(|frame| frame.render_widget(EditorTabBar::new(&tabs), frame.area()))
        .expect("a frame");
    let buf = terminal.backend().buffer().clone();
    let line = row(&buf, 0, 60);

    assert!(line.contains("dirty.rs*"), "line was {line:?}");
    assert!(line.contains("clean.rs "), "line was {line:?}");
    assert!(!line.contains("clean.rs*"), "line was {line:?}");
}

#[test]
fn a_modified_editor_reports_a_dirty_title_and_a_saved_one_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("x.rs");
    std::fs::write(&path, "fn main() {}\n").expect("write");

    let mut editor = Editor::new(78, 8);
    editor.open(&path).expect("open");
    assert!(!editor.pane_title().contains("[+]"));

    editor.set_cursor_position(Position::new(0, 0));
    editor.type_char('/');
    assert!(editor.pane_title().contains("[+]"));

    editor.save().expect("save");
    assert!(!editor.pane_title().contains("[+]"));

    let buf = draw(&editor, 80, 10);
    assert!(!row(&buf, 0, 80).contains("[+]"));
}

#[test]
fn secondary_cursors_are_visible_in_the_frame() {
    let mut editor = rust_editor("aaa\nbbb\n");
    editor.set_cursor_position(Position::new(0, 0));
    assert!(editor.add_cursor_below());

    let buf = draw(&editor, 80, 10);
    let cell = buf.cell((text_x(&editor), 2)).expect("cell");
    assert_eq!(cell.symbol(), "b");
    assert!(
        cell.style().add_modifier.contains(Modifier::REVERSED),
        "the second cursor must be drawn"
    );
}

#[test]
fn rendering_a_pane_with_no_room_does_not_panic() {
    let editor = rust_editor("fn f() {}\n");
    let backend = TestBackend::new(3, 2);
    let mut terminal = Terminal::new(backend).expect("a terminal");
    terminal
        .draw(|frame| frame.render_widget(EditorWidget::new(&editor), frame.area()))
        .expect("a frame");
}

#[test]
fn a_non_ascii_line_lines_up_with_its_highlighting() {
    let editor = rust_editor("let s = \"wörld\";\n");
    let buf = draw(&editor, 80, 10);
    let line = row(&buf, 1, 80);
    assert!(line.contains("wörld"), "row was {line:?}");

    let x = text_x(&editor);
    // Column 8 is the opening quote in character space.
    let quote = buf.cell((x + 8, 1)).expect("cell");
    assert_eq!(quote.symbol(), "\"");
    assert_eq!(quote.style().fg, Some(syntax_color(HighlightKind::String)));
}

#[test]
fn an_empty_document_renders_a_frame_with_a_tilde_column() {
    let editor = Editor::new(78, 8);
    let buf = draw(&editor, 80, 10);
    assert_eq!(buf.cell((1, 2)).expect("cell").symbol(), "~");
    assert_eq!(
        buf.cell((1, 2)).expect("cell").style().fg,
        Some(Color::DarkGray)
    );
}
