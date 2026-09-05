//! The Vim state machine and the Emacs bindings, driven end to end through the
//! editor the way the input layer drives them.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ratterm::editor::emacs::EmacsCommand;
use ratterm::editor::emacs_exec::EmacsEffect;
use ratterm::editor::emacs_keys::EmacsKey;
use ratterm::editor::vim::{VimEffect, VimKey, keys};
use ratterm::editor::{Editor, EditorMode, Position};

fn editor_with(text: &str) -> Editor {
    let mut editor = Editor::new(78, 20);
    editor.insert_str(text);
    editor.set_cursor_position(Position::new(0, 0));
    editor.set_mode(EditorMode::Normal);
    editor
}

/// Feeds a literal Vim key sequence.
fn vim(editor: &mut Editor, sequence: &str) {
    for key in keys(sequence) {
        editor.feed_vim_key(key);
    }
}

// --- Vim: motions and counts -------------------------------------------------

#[test]
fn a_count_multiplies_a_motion() {
    let mut editor = editor_with("one two three four\n");
    vim(&mut editor, "3w");
    assert_eq!(editor.cursor_position(), Position::new(0, 14));
}

#[test]
fn counts_before_and_after_an_operator_multiply() {
    let mut editor = editor_with("a b c d e f g\n");
    vim(&mut editor, "2d3w");
    assert_eq!(editor.buffer().text(), "g\n");
}

#[test]
fn a_bare_zero_is_the_line_start_motion_not_a_count() {
    let mut editor = editor_with("hello world\n");
    editor.set_cursor_position(Position::new(0, 6));
    vim(&mut editor, "0");
    assert_eq!(editor.cursor_position(), Position::new(0, 0));
}

#[test]
fn find_and_till_motions_work_with_operators() {
    let mut editor = editor_with("alpha,beta,gamma\n");
    vim(&mut editor, "df,");
    assert_eq!(editor.buffer().text(), "beta,gamma\n");

    let mut editor = editor_with("alpha,beta\n");
    vim(&mut editor, "dt,");
    assert_eq!(editor.buffer().text(), ",beta\n");
}

#[test]
fn semicolon_repeats_the_last_find() {
    let mut editor = editor_with("a.b.c.d\n");
    vim(&mut editor, "f.");
    assert_eq!(editor.cursor_position(), Position::new(0, 1));
    vim(&mut editor, ";");
    assert_eq!(editor.cursor_position(), Position::new(0, 3));
    vim(&mut editor, ",");
    assert_eq!(editor.cursor_position(), Position::new(0, 1));
}

#[test]
fn gg_and_shift_g_move_to_the_ends_of_the_file() {
    let mut editor = editor_with("one\ntwo\nthree\n");
    vim(&mut editor, "G");
    assert_eq!(editor.cursor_position().line, 2);
    vim(&mut editor, "gg");
    assert_eq!(editor.cursor_position().line, 0);
    vim(&mut editor, "2G");
    assert_eq!(editor.cursor_position().line, 1);
}

// --- Vim: operators and text objects ----------------------------------------

#[test]
fn dd_deletes_whole_lines_with_a_count() {
    let mut editor = editor_with("one\ntwo\nthree\nfour\n");
    vim(&mut editor, "2dd");
    assert_eq!(editor.buffer().text(), "three\nfour\n");
}

#[test]
fn ciw_changes_the_word_under_the_cursor() {
    let mut editor = editor_with("let value = 1;\n");
    editor.set_cursor_position(Position::new(0, 5));
    vim(&mut editor, "ciw");
    assert_eq!(editor.buffer().text(), "let  = 1;\n");
    assert_eq!(editor.mode(), EditorMode::Insert);
    editor.type_str("total");
    assert_eq!(editor.buffer().text(), "let total = 1;\n");
}

#[test]
fn di_quote_empties_a_string_literal() {
    let mut editor = editor_with("let s = \"hello\";\n");
    editor.set_cursor_position(Position::new(0, 11));
    vim(&mut editor, "di\"");
    assert_eq!(editor.buffer().text(), "let s = \"\";\n");
}

#[test]
fn da_paren_takes_the_brackets_too() {
    let mut editor = editor_with("f(a, b);\n");
    editor.set_cursor_position(Position::new(0, 3));
    vim(&mut editor, "da(");
    assert_eq!(editor.buffer().text(), "f;\n");
}

#[test]
fn an_indent_operator_shifts_the_lines_it_covers() {
    let mut editor = editor_with("a\nb\nc\n");
    vim(&mut editor, "2>>");
    assert_eq!(editor.buffer().text(), "    a\n    b\nc\n");
    vim(&mut editor, "2<<");
    assert_eq!(editor.buffer().text(), "a\nb\nc\n");
}

#[test]
fn the_case_operators_apply_to_a_motion() {
    let mut editor = editor_with("hello world\n");
    vim(&mut editor, "gUw");
    assert_eq!(editor.buffer().text(), "HELLO world\n");
    editor.set_cursor_position(Position::new(0, 0));
    vim(&mut editor, "guw");
    assert_eq!(editor.buffer().text(), "hello world\n");
}

// --- Vim: registers and dot-repeat ------------------------------------------

#[test]
fn a_named_register_round_trips_a_yank_and_a_put() {
    let mut editor = editor_with("first\nsecond\n");
    vim(&mut editor, "\"ayy");
    editor.set_cursor_position(Position::new(1, 0));
    vim(&mut editor, "\"ap");
    assert_eq!(editor.buffer().text(), "first\nsecond\nfirst\n");
}

#[test]
fn deletes_fill_the_numbered_registers_and_leave_register_zero_alone() {
    let mut editor = editor_with("keep\ndrop one\ndrop two\n");
    vim(&mut editor, "yy");
    editor.set_cursor_position(Position::new(1, 0));
    vim(&mut editor, "dd");
    editor.set_cursor_position(Position::new(1, 0));
    vim(&mut editor, "dd");

    assert_eq!(
        editor.registers().get(Some('0')).map(|c| c.text.clone()),
        Some("keep\n".to_string())
    );
    assert_eq!(
        editor.registers().get(Some('1')).map(|c| c.text.clone()),
        Some("drop two\n".to_string())
    );
    assert_eq!(
        editor.registers().get(Some('2')).map(|c| c.text.clone()),
        Some("drop one\n".to_string())
    );
}

#[test]
fn dot_repeats_the_last_change() {
    let mut editor = editor_with("aaa bbb ccc\n");
    vim(&mut editor, "dw");
    assert_eq!(editor.buffer().text(), "bbb ccc\n");
    vim(&mut editor, ".");
    assert_eq!(editor.buffer().text(), "ccc\n");
}

#[test]
fn a_motion_is_not_repeated_by_dot() {
    let mut editor = editor_with("aaa bbb ccc\n");
    vim(&mut editor, "w");
    let before = editor.cursor_position();
    vim(&mut editor, ".");
    assert_eq!(editor.cursor_position(), before);
    assert_eq!(editor.buffer().text(), "aaa bbb ccc\n");
}

// --- Vim: marks, visual mode, command mode ----------------------------------

#[test]
fn a_mark_can_be_set_and_jumped_back_to() {
    let mut editor = editor_with("one\ntwo\nthree\n");
    editor.set_cursor_position(Position::new(2, 2));
    vim(&mut editor, "ma");
    editor.set_cursor_position(Position::new(0, 0));
    vim(&mut editor, "`a");
    assert_eq!(editor.cursor_position(), Position::new(2, 2));
}

#[test]
fn visual_mode_selects_and_an_operator_applies_to_the_selection() {
    let mut editor = editor_with("hello world\n");
    vim(&mut editor, "v");
    assert_eq!(editor.mode(), EditorMode::Visual);
    vim(&mut editor, "eU");
    // `U` is not an operator on its own; escape and use a real one.
    editor.feed_vim_key(VimKey::Escape);
    editor.set_cursor_position(Position::new(0, 0));
    vim(&mut editor, "ved");
    assert_eq!(editor.buffer().text(), " world\n");
}

#[test]
fn command_mode_shows_the_line_and_runs_a_substitute() {
    let mut editor = editor_with("cat cat\ncat\n");
    vim(&mut editor, ":");
    assert_eq!(editor.mode(), EditorMode::Command);
    vim(&mut editor, "%s/cat/dog/g");
    assert_eq!(editor.vim_command_line(), "%s/cat/dog/g");
    editor.feed_vim_key(VimKey::Enter);

    assert_eq!(editor.buffer().text(), "dog dog\ndog\n");
    assert_eq!(editor.mode(), EditorMode::Normal);
}

#[test]
fn a_line_substitute_only_touches_the_cursor_line() {
    let mut editor = editor_with("cat cat\ncat\n");
    vim(&mut editor, ":s/cat/dog/g");
    editor.feed_vim_key(VimKey::Enter);
    assert_eq!(editor.buffer().text(), "dog dog\ncat\n");
}

#[test]
fn escape_abandons_a_command_line() {
    let mut editor = editor_with("abc\n");
    vim(&mut editor, ":s/a/b/");
    editor.feed_vim_key(VimKey::Escape);
    assert_eq!(editor.mode(), EditorMode::Normal);
    assert_eq!(editor.vim_command_line(), "");
    assert_eq!(editor.buffer().text(), "abc\n");
}

#[test]
fn write_and_quit_come_back_as_effects_for_the_application() {
    let mut editor = editor_with("abc\n");
    vim(&mut editor, ":w");
    let feed = editor.feed_vim_key(VimKey::Enter);
    assert_eq!(feed.effect, VimEffect::Save);

    vim(&mut editor, ":q!");
    let feed = editor.feed_vim_key(VimKey::Enter);
    assert_eq!(feed.effect, VimEffect::QuitWithoutSaving);
}

#[test]
fn an_incomplete_sequence_reports_itself_as_pending() {
    let mut editor = editor_with("abc\n");
    let feed = editor.feed_vim_key(VimKey::Char('d'));
    assert!(feed.pending);
    assert!(!feed.executed);
    let feed = editor.feed_vim_key(VimKey::Char('w'));
    assert!(feed.executed);
}

#[test]
fn a_key_that_spells_nothing_is_rejected_without_editing() {
    let mut editor = editor_with("abc\n");
    let feed = editor.feed_vim_key(VimKey::Char('Z'));
    assert!(!feed.executed);
    assert_eq!(editor.buffer().text(), "abc\n");
}

// --- Emacs -------------------------------------------------------------------

#[test]
fn the_kill_ring_carries_text_between_lines() {
    let mut editor = editor_with("alpha\nbeta\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.feed_emacs_key(EmacsKey::Ctrl('k'));
    assert_eq!(editor.buffer().text(), "\nbeta\n");

    editor.feed_emacs_key(EmacsKey::Ctrl('n'));
    editor.feed_emacs_key(EmacsKey::Ctrl('e'));
    editor.feed_emacs_key(EmacsKey::Ctrl('y'));
    assert_eq!(editor.buffer().text(), "\nbetaalpha\n");
}

#[test]
fn consecutive_kills_yank_back_as_one_block() {
    let mut editor = editor_with("one\ntwo\nthree\n");
    editor.set_cursor_position(Position::new(0, 0));
    for _ in 0..4 {
        editor.feed_emacs_key(EmacsKey::Ctrl('k'));
    }
    assert_eq!(editor.buffer().text(), "three\n");
    editor.feed_emacs_key(EmacsKey::Ctrl('e'));
    editor.feed_emacs_key(EmacsKey::Ctrl('y'));
    // Four kills took "one", its line break, "two", and its line break, so the
    // yank puts two whole lines back.
    assert_eq!(editor.buffer().text(), "threeone\ntwo\n\n");
    assert_eq!(editor.emacs().kill_ring.len(), 1);
}

#[test]
fn the_mark_and_ctrl_w_cut_a_region() {
    let mut editor = editor_with("hello world\n");
    editor.feed_emacs_key(EmacsKey::Ctrl(' '));
    editor.feed_emacs_key(EmacsKey::Alt('f'));
    editor.feed_emacs_key(EmacsKey::Ctrl('w'));
    assert_eq!(editor.buffer().text(), "world\n");
    assert_eq!(editor.kill_ring_head(), Some("hello "));
}

#[test]
fn alt_w_copies_without_cutting_and_ctrl_y_pastes() {
    let mut editor = editor_with("hello world\n");
    editor.feed_emacs_key(EmacsKey::Ctrl(' '));
    editor.feed_emacs_key(EmacsKey::Alt('f'));
    editor.feed_emacs_key(EmacsKey::Alt('w'));
    assert_eq!(editor.buffer().text(), "hello world\n");
    editor.feed_emacs_key(EmacsKey::Ctrl('e'));
    editor.feed_emacs_key(EmacsKey::Ctrl('y'));
    assert_eq!(editor.buffer().text(), "hello worldhello \n");
}

#[test]
fn alt_y_cycles_through_older_kills() {
    let mut editor = editor_with("first\nsecond\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.feed_emacs_key(EmacsKey::Ctrl('k'));
    editor.feed_emacs_key(EmacsKey::Ctrl('f'));
    editor.set_cursor_position(Position::new(1, 0));
    editor.feed_emacs_key(EmacsKey::Ctrl('k'));

    editor.feed_emacs_key(EmacsKey::Ctrl('y'));
    assert!(editor.buffer().text().contains("second"));
    editor.feed_emacs_key(EmacsKey::Alt('y'));
    assert!(editor.buffer().text().contains("first"));
}

#[test]
fn the_ctrl_x_prefix_reaches_save_and_exchange() {
    let mut editor = editor_with("hello\n");
    editor.feed_emacs_key(EmacsKey::Ctrl(' '));
    editor.feed_emacs_key(EmacsKey::Ctrl('e'));
    editor.feed_emacs_key(EmacsKey::Ctrl('x'));
    assert!(editor.emacs_prefix_pending());
    editor.feed_emacs_key(EmacsKey::Ctrl('x'));
    assert_eq!(editor.cursor_position(), Position::new(0, 0));

    editor.feed_emacs_key(EmacsKey::Ctrl('x'));
    assert_eq!(
        editor.feed_emacs_key(EmacsKey::Ctrl('s')),
        EmacsEffect::Save
    );
}

#[test]
fn emacs_movement_keys_navigate_without_editing() {
    let mut editor = editor_with("one\ntwo\nthree\n");
    editor.feed_emacs_key(EmacsKey::Ctrl('n'));
    assert_eq!(editor.cursor_position().line, 1);
    editor.feed_emacs_key(EmacsKey::Ctrl('e'));
    assert_eq!(editor.cursor_position(), Position::new(1, 3));
    editor.feed_emacs_key(EmacsKey::Ctrl('a'));
    assert_eq!(editor.cursor_position(), Position::new(1, 0));
    editor.feed_emacs_key(EmacsKey::Alt('>'));
    assert_eq!(editor.cursor_position().line, 3);
    editor.feed_emacs_key(EmacsKey::Alt('<'));
    assert_eq!(editor.cursor_position(), Position::new(0, 0));
    assert_eq!(editor.buffer().text(), "one\ntwo\nthree\n");
}

#[test]
fn m_x_command_names_resolve_to_the_same_commands_the_keys_run() {
    let mut editor = editor_with("hello world\n");
    let command = ratterm::editor::emacs::resolve("kill-line").expect("kill-line exists");
    assert_eq!(command, EmacsCommand::KillLine);
    editor.run_emacs(command);
    assert_eq!(editor.buffer().text(), "\n");
    assert!(ratterm::editor::emacs::complete("kill-").contains(&"kill-region"));
}

#[test]
fn ctrl_g_cancels_everything_in_flight() {
    let mut editor = editor_with("abc\nabc\n");
    editor.feed_emacs_key(EmacsKey::Ctrl(' '));
    editor.add_cursor_below();
    editor.open_search(false);
    editor.feed_emacs_key(EmacsKey::Ctrl('g'));
    assert!(!editor.search().is_active());
    assert!(editor.extra_cursors().is_empty());
    assert!(!editor.emacs().mark.is_active());
}
