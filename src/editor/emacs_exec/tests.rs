//! Unit tests for the parent module.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;

fn editor_with(text: &str) -> Editor {
    let mut editor = Editor::new(80, 24);
    editor.insert_str(text);
    editor.set_cursor_position(Position::new(0, 0));
    editor
}

#[test]
fn kill_line_then_yank_moves_text() {
    let mut editor = editor_with("hello world\nsecond\n");
    editor.set_cursor_position(Position::new(0, 5));
    editor.run_emacs(EmacsCommand::KillLine);
    assert_eq!(editor.buffer().text(), "hello\nsecond\n");
    assert_eq!(editor.kill_ring_head(), Some(" world"));

    editor.set_cursor_position(Position::new(1, 6));
    editor.run_emacs(EmacsCommand::Yank);
    assert_eq!(editor.buffer().text(), "hello\nsecond world\n");
}

#[test]
fn consecutive_kills_accumulate_into_one_entry() {
    let mut editor = editor_with("one\ntwo\nthree\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.run_emacs(EmacsCommand::KillLine);
    editor.run_emacs(EmacsCommand::KillLine);
    editor.run_emacs(EmacsCommand::KillLine);
    assert_eq!(editor.kill_ring_head(), Some("one\ntwo"));
    assert_eq!(editor.emacs().kill_ring.len(), 1);
}

#[test]
fn a_move_between_kills_starts_a_new_entry() {
    let mut editor = editor_with("one\ntwo\n");
    editor.run_emacs(EmacsCommand::KillLine);
    editor.run_emacs(EmacsCommand::ForwardChar);
    editor.run_emacs(EmacsCommand::KillLine);
    assert_eq!(editor.emacs().kill_ring.len(), 2);
}

#[test]
fn yank_pop_cycles_to_the_older_entry() {
    let mut editor = editor_with("aaa\nbbb\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.run_emacs(EmacsCommand::KillLine);
    editor.run_emacs(EmacsCommand::ForwardChar);
    editor.set_cursor_position(Position::new(1, 0));
    editor.run_emacs(EmacsCommand::KillLine);

    editor.run_emacs(EmacsCommand::Yank);
    assert!(editor.buffer().text().contains("bbb"));
    editor.run_emacs(EmacsCommand::YankPop);
    assert!(editor.buffer().text().contains("aaa"));
}

#[test]
fn the_mark_makes_movement_select() {
    let mut editor = editor_with("hello world\n");
    editor.run_emacs(EmacsCommand::SetMarkCommand);
    editor.run_emacs(EmacsCommand::ForwardWord);
    assert_eq!(editor.selected_text().as_deref(), Some("hello "));
}

#[test]
fn kill_region_removes_the_marked_text() {
    let mut editor = editor_with("hello world\n");
    editor.run_emacs(EmacsCommand::SetMarkCommand);
    editor.run_emacs(EmacsCommand::ForwardWord);
    editor.run_emacs(EmacsCommand::KillRegion);
    assert_eq!(editor.buffer().text(), "world\n");
    assert_eq!(editor.kill_ring_head(), Some("hello "));
}

#[test]
fn kill_ring_save_copies_without_deleting() {
    let mut editor = editor_with("hello world\n");
    editor.run_emacs(EmacsCommand::SetMarkCommand);
    editor.run_emacs(EmacsCommand::ForwardWord);
    editor.run_emacs(EmacsCommand::KillRingSave);
    assert_eq!(editor.buffer().text(), "hello world\n");
    assert_eq!(editor.kill_ring_head(), Some("hello "));
}

#[test]
fn killing_with_no_region_does_nothing() {
    let mut editor = editor_with("hello\n");
    editor.run_emacs(EmacsCommand::KillRegion);
    assert_eq!(editor.buffer().text(), "hello\n");
    assert_eq!(editor.kill_ring_head(), None);
}

#[test]
fn exchange_point_and_mark_swaps_the_ends() {
    let mut editor = editor_with("hello world\n");
    editor.run_emacs(EmacsCommand::SetMarkCommand);
    editor.run_emacs(EmacsCommand::ForwardWord);
    let point = editor.cursor_position();
    editor.run_emacs(EmacsCommand::ExchangePointAndMark);
    assert_eq!(editor.cursor_position(), Position::new(0, 0));
    assert_eq!(editor.emacs().mark.mark(), Some(point));
}

#[test]
fn kill_word_and_backward_kill_word_grow_one_entry() {
    let mut editor = editor_with("alpha beta gamma\n");
    editor.set_cursor_position(Position::new(0, 0));
    editor.run_emacs(EmacsCommand::KillWord);
    assert_eq!(editor.buffer().text(), "beta gamma\n");
    assert_eq!(editor.kill_ring_head(), Some("alpha "));

    editor.set_cursor_position(Position::new(0, 5));
    editor.run_emacs(EmacsCommand::BackwardKillWord);
    assert_eq!(editor.buffer().text(), "gamma\n");
    assert_eq!(editor.kill_ring_head(), Some("beta alpha "));
}

#[test]
fn transpose_swaps_the_characters_around_the_cursor() {
    let mut editor = editor_with("abcd\n");
    editor.set_cursor_position(Position::new(0, 2));
    editor.run_emacs(EmacsCommand::TransposeChars);
    assert_eq!(editor.buffer().text(), "acbd\n");
    assert_eq!(editor.cursor_position(), Position::new(0, 3));
}

#[test]
fn transpose_on_a_one_character_line_does_nothing() {
    let mut editor = editor_with("a\n");
    editor.run_emacs(EmacsCommand::TransposeChars);
    assert_eq!(editor.buffer().text(), "a\n");
}

#[test]
fn open_line_leaves_the_cursor_where_it_was() {
    let mut editor = editor_with("ab\n");
    editor.set_cursor_position(Position::new(0, 1));
    editor.run_emacs(EmacsCommand::OpenLine);
    assert_eq!(editor.buffer().text(), "a\nb\n");
    assert_eq!(editor.cursor_position(), Position::new(0, 1));
}

#[test]
fn keyboard_quit_clears_everything_transient() {
    let mut editor = editor_with("abc\n");
    editor.run_emacs(EmacsCommand::SetMarkCommand);
    editor.add_cursor_below();
    editor.open_search(false);
    editor.run_emacs(EmacsCommand::KeyboardQuit);
    assert!(!editor.emacs().mark.is_active());
    assert!(editor.extra_cursors().is_empty());
    assert!(!editor.search().is_active());
}

#[test]
fn what_cursor_position_reports_one_based_coordinates() {
    let mut editor = editor_with("ab\ncd\n");
    editor.set_cursor_position(Position::new(1, 1));
    editor.run_emacs(EmacsCommand::WhatCursorPosition);
    assert_eq!(editor.status(), "Line 2 Column 2");
}

#[test]
fn the_commands_the_application_has_to_finish_report_an_effect() {
    let mut editor = editor_with("x\n");
    assert_eq!(
        editor.run_emacs(EmacsCommand::SaveBuffer),
        EmacsEffect::Save
    );
    assert_eq!(
        editor.run_emacs(EmacsCommand::FindFile),
        EmacsEffect::FindFile
    );
    assert_eq!(
        editor.run_emacs(EmacsCommand::SaveBuffersKillTerminal),
        EmacsEffect::Quit
    );
    assert_eq!(
        editor.run_emacs(EmacsCommand::IsearchForward),
        EmacsEffect::Search { forward: true }
    );
    assert_eq!(
        editor.run_emacs(EmacsCommand::IsearchBackward),
        EmacsEffect::Search { forward: false }
    );
    assert_eq!(
        editor.run_emacs(EmacsCommand::QueryReplace),
        EmacsEffect::QueryReplace
    );
}

#[test]
fn a_read_only_document_refuses_emacs_edits() {
    let mut editor = editor_with("hello world\n");
    editor.set_read_only(true);
    editor.run_emacs(EmacsCommand::KillLine);
    editor.run_emacs(EmacsCommand::TransposeChars);
    editor.run_emacs(EmacsCommand::DeleteChar);
    assert_eq!(editor.buffer().text(), "hello world\n");
}

#[test]
fn resetting_transient_state_keeps_the_kill_ring() {
    let mut state = EmacsState::new();
    state.kill_ring.kill("kept");
    state.mark.set_mark(Position::new(1, 1));
    state.prefix_pending = true;
    state.reset_transient();
    assert_eq!(state.kill_ring.yank(), Some("kept"));
    assert!(!state.prefix_pending);
    assert_eq!(state.mark.mark(), None);
}
