//! Search and replace, driven through the keys the find bar accepts and then
//! read back out of a rendered frame.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ratterm::editor::decor::CharRole;
use ratterm::editor::search_ops::{SearchKey, SearchOutcome};
use ratterm::editor::{Editor, Position};
use ratterm::ui::editor_widget::EditorWidget;

fn editor_with(text: &str) -> Editor {
    let mut editor = Editor::new(78, 18);
    editor.insert_str(text);
    editor.set_cursor_position(Position::new(0, 0));
    editor
}

fn type_into_bar(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        editor.feed_search_key(SearchKey::Char(c));
    }
}

/// Renders the pane and returns the row the find bar occupies.
///
/// The bar sits inside the pane's border, so it is the row above the bottom
/// edge rather than the last one.
fn bar_row(editor: &Editor, width: u16, height: u16) -> String {
    let rows = frame(editor, width, height);
    rows[rows.len() - 2].clone()
}

/// Renders the pane and returns its rows as text.
fn frame(editor: &Editor, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("a terminal");
    terminal
        .draw(|f| f.render_widget(EditorWidget::new(editor).focused(true), f.area()))
        .expect("a frame");
    let buf = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| {
                    buf.cell((x, y))
                        .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
                })
                .collect()
        })
        .collect()
}

#[test]
fn the_find_bar_shows_a_live_match_count() {
    let mut editor = editor_with("cat dog cat\ncat\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "cat");

    let bar = bar_row(&editor, 80, 12);
    assert!(bar.contains("Find: cat"), "bar was {bar:?}");
    assert!(bar.contains("1/3"), "bar was {bar:?}");
}

#[test]
fn the_count_changes_as_the_query_narrows() {
    let mut editor = editor_with("cat car cab\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "ca");
    assert_eq!(editor.search().match_count(), 3);
    type_into_bar(&mut editor, "t");
    assert_eq!(editor.search().match_count(), 1);
    editor.feed_search_key(SearchKey::Backspace);
    assert_eq!(editor.search().match_count(), 3);
}

#[test]
fn searching_forward_wraps_correctly_at_the_end() {
    let mut editor = editor_with("hit\nmiss\nhit\nmiss\nhit\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "hit");

    assert_eq!(editor.cursor_position(), Position::new(0, 0));
    editor.feed_search_key(SearchKey::Enter);
    assert_eq!(editor.cursor_position(), Position::new(2, 0));
    editor.feed_search_key(SearchKey::Enter);
    assert_eq!(editor.cursor_position(), Position::new(4, 0));
    assert!(!editor.search().wrapped());

    editor.feed_search_key(SearchKey::Enter);
    assert_eq!(
        editor.cursor_position(),
        Position::new(0, 0),
        "the search must come back round to the first hit"
    );
    assert!(editor.search().wrapped());
    assert_eq!(editor.search().count_label(), "1/3");
}

#[test]
fn searching_backward_wraps_correctly_at_the_start() {
    let mut editor = editor_with("hit\nmiss\nhit\nmiss\nhit\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "hit");

    editor.feed_search_key(SearchKey::ShiftEnter);
    assert_eq!(
        editor.cursor_position(),
        Position::new(4, 0),
        "moving back from the first hit must land on the last"
    );
    assert!(editor.search().wrapped());
    editor.feed_search_key(SearchKey::ShiftEnter);
    assert_eq!(editor.cursor_position(), Position::new(2, 0));
    assert!(!editor.search().wrapped());
}

#[test]
fn a_single_match_wraps_onto_itself() {
    let mut editor = editor_with("only one hit\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "hit");
    assert_eq!(editor.search().match_count(), 1);
    editor.feed_search_key(SearchKey::Enter);
    assert_eq!(editor.search().count_label(), "1/1");
    assert!(editor.search().wrapped());
}

#[test]
fn no_match_leaves_the_cursor_alone_and_says_zero() {
    let mut editor = editor_with("nothing here\n");
    editor.set_cursor_position(Position::new(0, 4));
    editor.open_search(false);
    type_into_bar(&mut editor, "zzz");
    editor.feed_search_key(SearchKey::Enter);
    assert_eq!(editor.search().count_label(), "0/0");

    assert!(bar_row(&editor, 80, 10).contains("0/0"));
}

#[test]
fn replace_one_changes_only_the_current_match() {
    let mut editor = editor_with("cat cat cat\n");
    editor.open_search(true);
    type_into_bar(&mut editor, "cat");
    editor.feed_search_key(SearchKey::Tab);
    type_into_bar(&mut editor, "dog");

    editor.feed_search_key(SearchKey::Replace);
    assert_eq!(editor.buffer().text(), "dog cat cat\n");
    assert_eq!(editor.search().match_count(), 2);

    editor.feed_search_key(SearchKey::Replace);
    assert_eq!(editor.buffer().text(), "dog dog cat\n");
}

#[test]
fn replace_all_changes_every_match_in_one_step() {
    let mut editor = editor_with("cat\ncat cat\n");
    editor.open_search(true);
    type_into_bar(&mut editor, "cat");
    editor.feed_search_key(SearchKey::Tab);
    type_into_bar(&mut editor, "bird");

    editor.feed_search_key(SearchKey::ReplaceAll);
    assert_eq!(editor.buffer().text(), "bird\nbird bird\n");
    assert_eq!(editor.status(), "Replaced 3");

    editor.undo();
    assert_eq!(editor.buffer().text(), "cat\ncat cat\n");
}

#[test]
fn replacing_with_a_longer_string_keeps_the_remaining_matches_correct() {
    let mut editor = editor_with("a a a\n");
    editor.open_search(true);
    type_into_bar(&mut editor, "a");
    editor.feed_search_key(SearchKey::Tab);
    type_into_bar(&mut editor, "long");
    editor.feed_search_key(SearchKey::ReplaceAll);
    assert_eq!(editor.buffer().text(), "long long long\n");
}

#[test]
fn case_sensitivity_can_be_toggled_from_the_bar() {
    let mut editor = editor_with("Cat cat CAT\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "cat");
    assert_eq!(editor.search().match_count(), 3);

    editor.feed_search_key(SearchKey::ToggleCase);
    assert_eq!(editor.search().match_count(), 1);
    assert!(bar_row(&editor, 80, 10).contains("Aa"));
}

#[test]
fn matches_are_highlighted_and_the_current_one_stands_out() {
    let mut editor = editor_with("cat and cat\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "cat");

    let decor = editor.line_decor(0);
    assert_eq!(decor.role_at(0), CharRole::CurrentMatch);
    assert_eq!(decor.role_at(2), CharRole::CurrentMatch);
    assert_eq!(decor.role_at(3), CharRole::Plain);
    assert_eq!(decor.role_at(8), CharRole::SearchMatch);
}

#[test]
fn escape_closes_the_bar_and_stops_consuming_keys() {
    let mut editor = editor_with("cat\n");
    editor.open_search(false);
    assert_eq!(
        editor.feed_search_key(SearchKey::Escape),
        SearchOutcome::Closed
    );
    assert!(!bar_row(&editor, 80, 10).contains("Find:"));
    assert_eq!(
        editor.feed_search_key(SearchKey::Char('x')),
        SearchOutcome::Ignored
    );
    assert_eq!(editor.buffer().text(), "cat\n");
}

#[test]
fn a_read_only_document_can_be_searched_but_not_replaced() {
    let mut editor = editor_with("cat cat\n");
    editor.set_read_only(true);
    editor.open_search(true);
    type_into_bar(&mut editor, "cat");
    assert_eq!(editor.search().match_count(), 2);
    editor.feed_search_key(SearchKey::ReplaceAll);
    assert_eq!(editor.buffer().text(), "cat cat\n");
}

#[test]
fn a_search_across_a_folded_region_moves_the_cursor_to_a_visible_line() {
    let mut editor = Editor::new(78, 18);
    editor.insert_str("fn f() {\n    target();\n}\ntarget\n");
    editor.set_language(ratterm::editor::highlight::Language::Rust);
    editor.set_cursor_position(Position::new(0, 0));
    assert!(editor.toggle_fold());

    editor.open_search(false);
    type_into_bar(&mut editor, "target");
    assert!(
        !editor.is_line_hidden(editor.cursor_position().line),
        "the cursor must not be parked inside a collapsed region"
    );
}

#[test]
fn editing_after_a_search_recounts_before_the_next_move() {
    let mut editor = editor_with("cat cat\n");
    editor.open_search(false);
    type_into_bar(&mut editor, "cat");
    assert_eq!(editor.search().match_count(), 2);

    editor.close_search();
    editor.set_cursor_position(Position::new(0, 7));
    editor.insert_str(" cat");

    editor.open_search(false);
    assert_eq!(editor.search().match_count(), 3);
}
