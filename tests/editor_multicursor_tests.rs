//! Multiple cursors, auto-indentation and auto-pairing, exercised the way a
//! user reaches them.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ratterm::editor::brackets::AutoPair;
use ratterm::editor::decor::CharRole;
use ratterm::editor::highlight::Language;
use ratterm::editor::indent::IndentStyle;
use ratterm::editor::{Editor, Position};

fn rust_editor(text: &str) -> Editor {
    let mut editor = Editor::new(78, 18);
    editor.insert_str(text);
    editor.set_language(Language::Rust);
    editor.set_cursor_position(Position::new(0, 0));
    editor
}

// --- multiple cursors --------------------------------------------------------

#[test]
fn a_cursor_can_be_added_below_and_typing_reaches_both() {
    let mut editor = rust_editor("aa\nbb\ncc\n");
    assert!(editor.add_cursor_below());
    editor.type_char('X');
    assert_eq!(editor.buffer().text(), "Xaa\nXbb\ncc\n");
}

#[test]
fn cursors_can_be_added_above_and_below_at_the_same_time() {
    let mut editor = rust_editor("aa\nbb\ncc\n");
    editor.set_cursor_position(Position::new(1, 0));
    assert!(editor.add_cursor_above());
    assert!(editor.add_cursor_below());
    assert_eq!(editor.all_cursors().len(), 3);
    editor.type_str("// ");
    assert_eq!(editor.buffer().text(), "// aa\n// bb\n// cc\n");
}

#[test]
fn a_cursor_lands_on_the_next_occurrence_of_the_word() {
    let mut editor = rust_editor("let total = 1;\nlet total2 = total;\n");
    editor.set_cursor_position(Position::new(0, 4));
    assert!(editor.add_cursor_at_next_occurrence());
    assert_eq!(
        editor.extra_cursors().positions(),
        &[Position::new(1, 4)],
        "the second `total` is on line two"
    );
}

#[test]
fn every_occurrence_can_be_taken_at_once() {
    let mut editor = rust_editor("foo\nfoo\nfoo\n");
    editor.set_cursor_position(Position::new(0, 0));
    assert_eq!(editor.add_cursors_at_all_occurrences(), 2);
    editor.type_str("my_");
    assert_eq!(editor.buffer().text(), "my_foo\nmy_foo\nmy_foo\n");
}

#[test]
fn escape_drops_back_to_a_single_cursor() {
    let mut editor = rust_editor("aa\nbb\n");
    editor.add_cursor_below();
    editor.clear_extra_cursors();
    assert!(editor.extra_cursors().is_empty());
    editor.type_char('X');
    assert_eq!(editor.buffer().text(), "Xaa\nbb\n");
}

#[test]
fn backspace_reaches_every_cursor() {
    let mut editor = rust_editor("abc\nabc\nabc\n");
    editor.set_cursor_position(Position::new(0, 3));
    editor.add_cursor_below();
    editor.add_cursor_below();
    editor.backspace();
    assert_eq!(editor.buffer().text(), "ab\nab\nab\n");
}

#[test]
fn a_multi_cursor_edit_undoes_in_one_step() {
    let mut editor = rust_editor("aa\nbb\ncc\n");
    editor.add_cursor_below();
    editor.add_cursor_below();
    editor.type_char('X');
    assert_eq!(editor.buffer().text(), "Xaa\nXbb\nXcc\n");
    editor.undo();
    assert_eq!(editor.buffer().text(), "aa\nbb\ncc\n");
}

#[test]
fn secondary_cursors_show_up_in_the_render_model() {
    let mut editor = rust_editor("aaa\nbbb\n");
    editor.set_cursor_position(Position::new(0, 1));
    editor.add_cursor_below();
    assert_eq!(editor.line_decor(1).role_at(1), CharRole::ExtraCursor);
    assert_eq!(editor.line_decor(0).role_at(1), CharRole::Plain);
}

#[test]
fn cursors_cannot_be_added_past_the_ends_of_the_document() {
    let mut editor = rust_editor("only\n");
    editor.set_cursor_position(Position::new(0, 0));
    assert!(!editor.add_cursor_above());
}

// --- auto-indent -------------------------------------------------------------

#[test]
fn enter_after_an_opening_brace_indents_the_new_line() {
    let mut editor = rust_editor("fn f() {");
    editor.set_cursor_position(Position::new(0, 8));
    editor.insert_newline();
    assert_eq!(editor.buffer().text(), "fn f() {\n    ");
}

#[test]
fn enter_between_a_pair_of_braces_opens_a_body() {
    let mut editor = rust_editor("fn f() {}");
    editor.set_cursor_position(Position::new(0, 8));
    editor.insert_newline_smart();
    assert_eq!(editor.buffer().text(), "fn f() {\n    \n}");
    assert_eq!(editor.cursor_position(), Position::new(1, 4));
}

#[test]
fn a_closing_brace_snaps_onto_the_line_that_opened_the_block() {
    let mut editor = rust_editor("fn f() {\n    g();\n            ");
    editor.set_cursor_position(Position::new(2, 12));
    editor.type_char('}');
    assert_eq!(editor.buffer().text(), "fn f() {\n    g();\n}");
}

#[test]
fn the_indent_style_is_detected_from_the_file() {
    let mut editor = Editor::new(78, 18);
    editor.insert_str("function f() {\n  a();\n  if (b) {\n    c();\n  }\n}\n");
    assert_eq!(editor.redetect_indent(), IndentStyle::Spaces(2));
    editor.set_cursor_position(Position::new(0, 14));
    editor.insert_newline();
    assert_eq!(editor.buffer().line_text(1), "  ");
}

#[test]
fn a_tab_indented_file_keeps_using_tabs() {
    let mut editor = Editor::new(78, 18);
    editor.insert_str("fn f() {\n\tg();\n\tif a {\n\t\th();\n\t}\n}\n");
    assert_eq!(editor.redetect_indent(), IndentStyle::Tabs);
    assert_eq!(editor.indent_unit(), "\t");
}

#[test]
fn python_indents_after_a_colon() {
    let mut editor = Editor::new(78, 18);
    editor.insert_str("def f():");
    editor.set_language(Language::Python);
    editor.set_cursor_position(Position::new(0, 8));
    editor.insert_newline();
    assert_eq!(editor.buffer().text(), "def f():\n    ");
}

// --- brackets ----------------------------------------------------------------

#[test]
fn typing_an_opening_bracket_inserts_the_pair() {
    let mut editor = rust_editor("let a = ");
    editor.set_cursor_position(Position::new(0, 8));
    editor.type_char('(');
    assert_eq!(editor.buffer().text(), "let a = ()");
    assert_eq!(editor.cursor_position(), Position::new(0, 9));
}

#[test]
fn typing_the_closer_steps_over_rather_than_doubling_it() {
    let mut editor = rust_editor("let a = ");
    editor.set_cursor_position(Position::new(0, 8));
    editor.type_char('(');
    editor.type_char('1');
    editor.type_char(')');
    assert_eq!(editor.buffer().text(), "let a = (1)");
    assert_eq!(editor.cursor_position(), Position::new(0, 11));
}

#[test]
fn quotes_pair_and_close() {
    let mut editor = rust_editor("let s = ");
    editor.set_cursor_position(Position::new(0, 8));
    editor.type_str("\"hi\"");
    assert_eq!(editor.buffer().text(), "let s = \"hi\"");
}

#[test]
fn auto_pairing_is_suppressed_directly_before_a_word() {
    let mut editor = rust_editor("value");
    editor.set_cursor_position(Position::new(0, 0));
    editor.type_char('(');
    assert_eq!(editor.buffer().text(), "(value");
}

#[test]
fn auto_pairing_can_be_switched_off_entirely() {
    let mut editor = rust_editor("");
    editor.set_auto_pair(AutoPair {
        enabled: false,
        ..AutoPair::default()
    });
    editor.type_str("([\"");
    assert_eq!(editor.buffer().text(), "([\"");
}

#[test]
fn the_matching_bracket_is_reported_from_either_side() {
    let mut editor = rust_editor("fn f() {\n    g();\n}\n");
    editor.set_cursor_position(Position::new(0, 7));
    assert_eq!(
        editor.matching_bracket_pair(),
        Some((Position::new(0, 7), Position::new(2, 0)))
    );
    editor.set_cursor_position(Position::new(2, 0));
    assert_eq!(
        editor.matching_bracket_pair(),
        Some((Position::new(0, 7), Position::new(2, 0)))
    );
    // And from just past the closing brace, where insert mode leaves the cursor.
    editor.set_cursor_position(Position::new(2, 1));
    assert_eq!(
        editor.matching_bracket_pair(),
        Some((Position::new(0, 7), Position::new(2, 0)))
    );
}

#[test]
fn a_bracket_inside_a_string_is_not_matched() {
    let mut editor = rust_editor("let s = \"(\";\n");
    editor.set_cursor_position(Position::new(0, 9));
    assert_eq!(editor.matching_bracket_pair(), None);
}
