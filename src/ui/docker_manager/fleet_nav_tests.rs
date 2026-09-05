//! Tests for `fleet_nav.rs`.
//!
//! Included with `#[path]`, so they are a child module of it and can still
//! reach its private items while living in their own file.

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn movement_keys_move_the_cursor_and_ask_for_nothing() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Down), 3),
        FleetAction::None
    );
    assert_eq!(state.selected(), 1);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('j')), 3),
        FleetAction::None
    );
    assert_eq!(state.selected(), 2);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('k')), 3),
        FleetAction::None
    );
    assert_eq!(state.selected(), 1);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::End), 3),
        FleetAction::None
    );
    assert_eq!(state.selected(), 2);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Home), 3),
        FleetAction::None
    );
    assert_eq!(state.selected(), 0);
}

#[test]
fn the_view_can_be_closed_two_ways() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Esc), 0),
        FleetAction::Close
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('q')), 0),
        FleetAction::Close
    );
}

#[test]
fn commands_that_need_a_daemon_come_back_to_the_caller() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('r')), 1),
        FleetAction::Refresh
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('s')), 1),
        FleetAction::CycleSort
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Enter), 1),
        FleetAction::Activate
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('S')), 1),
        FleetAction::Start
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('X')), 1),
        FleetAction::Stop
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('R')), 1),
        FleetAction::Restart
    );
}

#[test]
fn ctrl_r_is_not_a_refresh() {
    // Ctrl+R belongs to the surrounding application; the fleet view must
    // not swallow it.
    let mut state = FleetViewState::new();
    let event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert_eq!(handle_key(&mut state, event, 1), FleetAction::None);
}

#[test]
fn the_events_pane_is_toggled_locally() {
    let mut state = FleetViewState::new();
    assert!(state.events_shown());
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('e')), 1),
        FleetAction::None
    );
    assert!(!state.events_shown());
}

#[test]
fn the_filter_line_takes_every_printable_key_while_it_is_open() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('/')), 5),
        FleetAction::None
    );
    assert!(state.editing_filter());

    // `r` would be a refresh outside the filter line.
    for c in "rock".chars() {
        assert_eq!(
            handle_key(&mut state, key(KeyCode::Char(c)), 5),
            FleetAction::FilterChanged
        );
    }
    assert_eq!(state.filter(), "rock");
    assert_eq!(state.selected(), 0, "a narrowed list starts at the top");

    assert_eq!(
        handle_key(&mut state, key(KeyCode::Backspace), 5),
        FleetAction::FilterChanged
    );
    assert_eq!(state.filter(), "roc");

    assert_eq!(
        handle_key(&mut state, key(KeyCode::Enter), 5),
        FleetAction::None
    );
    assert!(
        !state.editing_filter(),
        "enter keeps the filter and closes the line"
    );
    assert_eq!(state.filter(), "roc");
}

#[test]
fn backspace_on_an_empty_filter_changes_nothing() {
    let mut state = FleetViewState::new();
    handle_key(&mut state, key(KeyCode::Char('/')), 5);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Backspace), 5),
        FleetAction::None
    );
    assert!(state.filter().is_empty());
}

#[test]
fn escape_closes_the_filter_before_it_closes_the_view() {
    let mut state = FleetViewState::new();
    handle_key(&mut state, key(KeyCode::Char('/')), 5);
    handle_key(&mut state, key(KeyCode::Char('a')), 5);

    assert_eq!(
        handle_key(&mut state, key(KeyCode::Esc), 5),
        FleetAction::FilterChanged,
        "the first escape clears the filter"
    );
    assert!(!state.editing_filter());
    assert!(state.filter().is_empty());

    assert_eq!(
        handle_key(&mut state, key(KeyCode::Esc), 5),
        FleetAction::Close,
        "the second escape leaves the view"
    );
}

#[test]
fn escaping_an_empty_filter_line_just_closes_it() {
    let mut state = FleetViewState::new();
    handle_key(&mut state, key(KeyCode::Char('/')), 5);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Esc), 5),
        FleetAction::None
    );
    assert!(!state.editing_filter());
}

#[test]
fn clearing_an_already_empty_filter_asks_for_no_work() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('c')), 5),
        FleetAction::None
    );

    handle_key(&mut state, key(KeyCode::Char('/')), 5);
    handle_key(&mut state, key(KeyCode::Char('x')), 5);
    handle_key(&mut state, key(KeyCode::Enter), 5);
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Char('c')), 5),
        FleetAction::FilterChanged
    );
    assert!(state.filter().is_empty());
}

#[test]
fn an_unbound_key_does_nothing() {
    let mut state = FleetViewState::new();
    assert_eq!(
        handle_key(&mut state, key(KeyCode::Tab), 5),
        FleetAction::None
    );
    assert_eq!(
        handle_key(&mut state, key(KeyCode::F(4)), 5),
        FleetAction::None
    );
}

#[test]
fn a_new_state_starts_at_the_top_with_events_shown() {
    let state = FleetViewState::new();
    assert_eq!(state.selected(), 0);
    assert!(state.events_shown());
}

#[test]
fn the_events_pane_toggles() {
    let mut state = FleetViewState::new();
    state.toggle_events();
    assert!(!state.events_shown());
    state.toggle_events();
    assert!(state.events_shown());
}

#[test]
fn selection_moves_and_wraps_in_both_directions() {
    let mut state = FleetViewState::new();
    state.select_next(3);
    assert_eq!(state.selected(), 1);
    state.select_next(3);
    state.select_next(3);
    assert_eq!(state.selected(), 0, "past the end wraps to the start");
    state.select_prev(3);
    assert_eq!(state.selected(), 2, "before the start wraps to the end");
}

#[test]
fn selection_in_an_empty_list_stays_at_zero() {
    let mut state = FleetViewState::new();
    state.select_next(0);
    assert_eq!(state.selected(), 0);
    state.select_prev(0);
    assert_eq!(state.selected(), 0);
    state.select_last(0);
    assert_eq!(state.selected(), 0);
}

#[test]
fn first_and_last_jump_to_the_ends() {
    let mut state = FleetViewState::new();
    state.select_last(5);
    assert_eq!(state.selected(), 4);
    state.select_first();
    assert_eq!(state.selected(), 0);
}

#[test]
fn a_shrinking_list_pulls_the_selection_back_inside() {
    let mut state = FleetViewState::new();
    state.select_last(10);
    state.clamp(3);
    assert_eq!(state.selected(), 2);
    state.clamp(0);
    assert_eq!(state.selected(), 0);
    state.clamp(9);
    assert_eq!(state.selected(), 0, "clamping never moves down");
}

#[test]
fn a_window_over_an_empty_list_is_empty() {
    assert_eq!(visible_window(0, 0, 10), (0, 0));
    assert_eq!(visible_window(5, 0, 0), (0, 0));
}

#[test]
fn a_list_shorter_than_the_window_is_shown_whole() {
    assert_eq!(visible_window(3, 2, 10), (0, 3));
}

#[test]
fn the_window_scrolls_only_as_far_as_it_must() {
    assert_eq!(visible_window(100, 0, 10), (0, 10));
    assert_eq!(visible_window(100, 9, 10), (0, 10));
    assert_eq!(visible_window(100, 10, 10), (1, 11));
    assert_eq!(visible_window(100, 99, 10), (90, 100));
}

#[test]
fn a_selection_past_the_end_still_produces_a_valid_window() {
    let (start, end) = visible_window(5, 99, 3);
    assert!(start < end);
    assert!(end <= 5);
    assert_eq!((start, end), (2, 5));
}

#[test]
fn text_is_fitted_to_the_available_width() {
    assert_eq!(fit("short", 10), "short");
    assert_eq!(fit("exactfit!!", 10), "exactfit!!");
    assert_eq!(fit("much too long here", 10), "much to...");
    assert_eq!(fit("abc", 0), "");
    assert_eq!(fit("abcdef", 2), "ab");
}

#[test]
fn fitting_does_not_split_a_multibyte_character() {
    // Slicing by bytes here would panic.
    assert_eq!(fit("ééééééé", 5), "éé...");
}
