//! Regression tests for per-tab editor state.
//!
//! Before this, every tab shared one `Editor` and switching tabs called
//! `Editor::open`, which replaced the buffer with a fresh read from disk. Any
//! unsaved edit and the entire undo history were discarded silently, and the
//! `io::Error` from the reopen was thrown away with `let _ =`.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use ratterm::app::App;
use ratterm::editor::EditorMode;
use tempfile::TempDir;

/// Builds an app with two files open and returns their paths.
///
/// Tab 0 is `a.txt`, tab 1 is `b.txt`, and tab 1 is active on return.
fn app_with_two_files() -> (App, TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, "alpha\n").expect("write a");
    std::fs::write(&b, "bravo\n").expect("write b");

    let mut app = App::isolated(80, 24).expect("app");
    app.open_file(a.clone()).expect("open a");
    app.open_file(b.clone()).expect("open b");

    (app, dir, a, b)
}

#[test]
fn switching_tabs_preserves_unsaved_edits() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    // Edit tab 1 without saving.
    app.editor_mut().insert_str("UNSAVED");
    assert!(app.editor().buffer().text().contains("UNSAVED"));

    app.prev_file();
    assert_eq!(app.active_tab_index(), 0);
    assert!(app.editor().buffer().text().contains("alpha"));

    app.next_file();
    assert_eq!(app.active_tab_index(), 1);
    assert!(
        app.editor().buffer().text().contains("UNSAVED"),
        "unsaved text must survive a round trip through another tab"
    );
    assert!(app.editor().is_modified());
}

#[test]
fn switching_tabs_preserves_undo_history() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    app.editor_mut().insert_str("one");
    app.editor_mut().insert_str("two");

    app.prev_file();
    app.next_file();

    app.editor_mut().undo();
    let text = app.editor().buffer().text();
    assert!(
        text.contains("one") && !text.contains("two"),
        "undo after a tab switch should remove only the last edit, got {text:?}"
    );
}

#[test]
fn switching_tabs_preserves_cursor_position() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    app.editor_mut().insert_str("xyz");
    let before = app.editor().cursor_position();

    app.prev_file();
    app.next_file();

    assert_eq!(app.editor().cursor_position(), before);
}

#[test]
fn each_tab_reports_its_own_dirty_flag() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    assert!(!app.tab_is_modified(0));
    assert!(!app.tab_is_modified(1));
    assert!(!app.any_tab_modified());

    app.editor_mut().insert_str("dirty");
    assert!(app.tab_is_modified(1), "the active tab is dirty");
    assert!(!app.tab_is_modified(0), "the parked tab is clean");

    app.prev_file();
    assert!(
        app.tab_is_modified(1),
        "the parked tab keeps its dirty flag"
    );
    assert!(!app.tab_is_modified(0));
    assert!(app.any_tab_modified());
}

#[test]
fn tab_info_exposes_per_tab_dirty_state() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    app.editor_mut().insert_str("dirty");
    app.prev_file();

    let info = app.editor_tab_info();
    assert_eq!(info.len(), 2);
    assert!(info[0].is_active);
    assert!(!info[0].is_modified);
    assert!(!info[1].is_active);
    assert!(
        info[1].is_modified,
        "the tab bar must mark the background tab as modified"
    );
}

#[test]
fn tab_is_modified_is_false_for_an_unknown_index() {
    let (app, _dir, _a, _b) = app_with_two_files();
    assert!(!app.tab_is_modified(99));
}

#[test]
fn reopening_an_open_file_activates_its_tab_without_reloading() {
    let (mut app, _dir, a, _b) = app_with_two_files();

    app.prev_file();
    app.editor_mut().insert_str("PENDING");

    app.next_file();
    app.open_file(a).expect("reopen a");

    assert_eq!(app.active_tab_index(), 0);
    assert!(
        app.editor().buffer().text().contains("PENDING"),
        "reopening an already-open file must not discard its edits"
    );
}

#[test]
fn activate_tab_rejects_an_out_of_range_index() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    assert!(!app.activate_tab(7));
    assert_eq!(app.active_tab_index(), 1, "the active tab is unchanged");
}

#[test]
fn activate_tab_on_the_current_tab_is_a_no_op() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    app.editor_mut().insert_str("HERE");
    assert!(app.activate_tab(1));
    assert!(app.editor().buffer().text().contains("HERE"));
}

#[test]
fn a_failed_open_leaves_the_current_tab_untouched() {
    let (mut app, dir, _a, _b) = app_with_two_files();
    app.editor_mut().insert_str("KEEP");

    let missing = dir.path().join("nope.txt");
    assert!(app.open_file(missing).is_err());

    assert_eq!(app.active_tab_index(), 1);
    assert_eq!(app.tab_count(), 2);
    assert!(app.editor().buffer().text().contains("KEEP"));
}

#[test]
fn new_tab_parks_the_previous_document() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    app.editor_mut().insert_str("BEFORE");

    app.new_editor_tab();
    assert_eq!(app.tab_count(), 3);
    assert!(app.editor().buffer().is_empty());

    app.prev_file();
    assert!(app.editor().buffer().text().contains("BEFORE"));
}

#[test]
fn closing_a_clean_tab_activates_a_neighbour_with_its_own_state() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    // Park an edit on tab 0 and save it so the tab is closable.
    app.prev_file();
    app.editor_mut().insert_str("saved-edit");
    app.save_current_file();
    app.next_file();

    app.close_editor_tab();

    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.active_tab_index(), 0);
    assert!(
        app.editor().buffer().text().contains("saved-edit"),
        "the surviving tab keeps its own document"
    );
}

#[test]
fn closing_a_dirty_tab_is_refused() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    app.editor_mut().insert_str("dirty");

    app.close_editor_tab();

    assert_eq!(app.tab_count(), 2, "a dirty tab is not closed silently");
}

#[test]
fn closing_the_last_tab_leaves_an_empty_editor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("only.txt");
    std::fs::write(&a, "solo").expect("write");

    let mut app = App::isolated(80, 24).expect("app");
    app.open_file(a).expect("open");
    app.close_editor_tab();

    assert_eq!(app.tab_count(), 0);
    assert!(app.editor().buffer().is_empty());
}

#[test]
fn a_large_file_opens_read_only_and_rejects_typing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let big = dir.path().join("big.txt");
    std::fs::write(&big, "x".repeat(4096)).expect("write");

    let mut app = App::isolated(80, 24).expect("app");
    app.editor_mut().set_size_limits(1024, 1024 * 1024);
    app.open_file(big).expect("open");

    assert!(app.editor().is_read_only());
    app.editor_mut().insert_str("nope");
    assert_eq!(app.editor().buffer().len_chars(), 4096);
}

#[test]
fn read_only_state_travels_with_the_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let small = dir.path().join("small.txt");
    let big = dir.path().join("big.txt");
    std::fs::write(&small, "tiny").expect("write small");
    std::fs::write(&big, "x".repeat(4096)).expect("write big");

    let mut app = App::isolated(80, 24).expect("app");
    app.editor_mut().set_size_limits(1024, 1024 * 1024);
    app.open_file(big).expect("open big");
    app.open_file(small).expect("open small");

    assert!(!app.editor().is_read_only(), "the small file is editable");
    app.prev_file();
    assert!(app.editor().is_read_only(), "the big file is still frozen");
}

#[test]
fn editing_mode_is_per_tab() {
    let (mut app, _dir, _a, _b) = app_with_two_files();

    app.editor_mut().set_mode(EditorMode::Insert);
    app.prev_file();
    assert_eq!(app.editor().mode(), EditorMode::Normal);
    app.next_file();
    assert_eq!(app.editor().mode(), EditorMode::Insert);
}

#[test]
fn cycling_through_every_tab_returns_to_the_start() {
    let (mut app, _dir, _a, _b) = app_with_two_files();
    let start = app.active_tab_index();
    app.next_file();
    app.next_file();
    assert_eq!(app.active_tab_index(), start);
    app.prev_file();
    app.prev_file();
    assert_eq!(app.active_tab_index(), start);
}

#[test]
fn tab_navigation_on_an_empty_workspace_is_harmless() {
    let mut app = App::isolated(80, 24).expect("app");
    app.next_file();
    app.prev_file();
    assert_eq!(app.tab_count(), 0);
    assert_eq!(app.active_tab_index(), 0);
}
