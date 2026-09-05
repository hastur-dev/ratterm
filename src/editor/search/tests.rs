//! Unit tests for the parent module.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;

fn state_over(text: &str, query: &str) -> (SearchState, Buffer) {
    let buffer = Buffer::from_str(text);
    let mut state = SearchState::new();
    state.open(false);
    state.set_query(query);
    state.refresh(&buffer);
    (state, buffer)
}

#[test]
fn a_fresh_state_is_closed_and_empty() {
    let state = SearchState::new();
    assert!(!state.is_active());
    assert_eq!(state.match_count(), 0);
    assert_eq!(state.current_match(), None);
    assert_eq!(state.count_label(), "");
}

#[test]
fn refreshing_finds_every_match_and_selects_the_first() {
    let (state, _) = state_over("ab ab ab", "ab");
    assert_eq!(state.match_count(), 3);
    assert_eq!(state.current_match(), Some(Position::new(0, 0)));
    assert_eq!(state.count_label(), "1/3");
    assert!(!state.is_stale());
}

#[test]
fn search_is_case_insensitive_until_asked_otherwise() {
    let buffer = Buffer::from_str("Ab ab AB");
    let mut state = SearchState::new();
    state.set_query("ab");
    state.refresh(&buffer);
    assert_eq!(state.match_count(), 3);

    state.set_case_sensitive(true);
    state.refresh(&buffer);
    assert_eq!(state.match_count(), 1);
    assert_eq!(state.current_match(), Some(Position::new(0, 3)));
}

#[test]
fn next_wraps_at_the_end_and_says_so() {
    let (mut state, _) = state_over("a a a", "a");
    assert_eq!(state.next_match(), Some(Position::new(0, 2)));
    assert!(!state.wrapped());
    assert_eq!(state.next_match(), Some(Position::new(0, 4)));
    assert!(!state.wrapped());
    assert_eq!(state.next_match(), Some(Position::new(0, 0)));
    assert!(state.wrapped(), "moving past the last match wraps");
    assert_eq!(state.count_label(), "1/3");
}

#[test]
fn prev_wraps_at_the_start_and_says_so() {
    let (mut state, _) = state_over("a a a", "a");
    assert_eq!(state.current_ordinal(), Some(1));
    assert_eq!(state.prev_match(), Some(Position::new(0, 4)));
    assert!(state.wrapped(), "moving before the first match wraps");
    assert_eq!(state.prev_match(), Some(Position::new(0, 2)));
    assert!(!state.wrapped());
}

#[test]
fn navigation_on_an_empty_match_list_is_a_no_op() {
    let (mut state, _) = state_over("hello", "zzz");
    assert_eq!(state.match_count(), 0);
    assert_eq!(state.next_match(), None);
    assert_eq!(state.prev_match(), None);
    assert_eq!(state.count_label(), "0/0");
}

#[test]
fn select_from_starts_at_the_cursor_rather_than_the_top() {
    let (mut state, _) = state_over("a\na\na\n", "a");
    state.select_from(Position::new(1, 0));
    assert_eq!(state.current_match(), Some(Position::new(1, 0)));
    // Past the last match it comes back to the first.
    state.select_from(Position::new(9, 0));
    assert_eq!(state.current_match(), Some(Position::new(0, 0)));
}

#[test]
fn refreshing_keeps_the_current_match_when_it_survives() {
    let buffer = Buffer::from_str("one two one");
    let mut state = SearchState::new();
    state.set_query("one");
    state.refresh(&buffer);
    state.next_match();
    assert_eq!(state.current_match(), Some(Position::new(0, 8)));
    state.refresh(&buffer);
    assert_eq!(
        state.current_match(),
        Some(Position::new(0, 8)),
        "a recount must not throw the user back to the first match"
    );
}

#[test]
fn the_direction_flag_chooses_which_way_enter_moves() {
    let (mut state, _) = state_over("a a a", "a");
    state.set_direction(SearchDirection::Backward);
    assert_eq!(state.direction(), SearchDirection::Backward);
    assert_eq!(state.advance(), Some(Position::new(0, 4)));
    state.set_direction(SearchDirection::Forward);
    assert_eq!(state.advance(), Some(Position::new(0, 0)));
}

#[test]
fn typing_goes_to_the_focused_field() {
    let mut state = SearchState::new();
    state.open(true);
    state.push_char('a');
    state.toggle_field();
    state.push_char('b');
    assert_eq!(state.query(), "a");
    assert_eq!(state.replacement(), "b");
    assert_eq!(state.field(), SearchField::Replacement);
    state.pop_char();
    assert_eq!(state.replacement(), "");
    state.toggle_field();
    state.pop_char();
    assert_eq!(state.query(), "");
}

#[test]
fn the_field_cannot_be_switched_when_replace_is_hidden() {
    let mut state = SearchState::new();
    state.open(false);
    state.toggle_field();
    assert_eq!(state.field(), SearchField::Query);
    assert_eq!(SearchField::Query.toggled(), SearchField::Replacement);
}

#[test]
fn matches_on_a_line_report_their_columns_and_which_is_current() {
    let (state, _) = state_over("ab\nxaby\nab", "ab");
    assert_eq!(state.matches_on_line(0), vec![(0, 2, true)]);
    assert_eq!(state.matches_on_line(1), vec![(1, 3, false)]);
    assert!(state.matches_on_line(9).is_empty());
}

#[test]
fn editing_the_buffer_marks_the_matches_stale() {
    let (mut state, _) = state_over("a a", "a");
    assert!(!state.is_stale());
    state.mark_stale();
    assert!(state.is_stale());
}

#[test]
fn forgetting_the_current_match_keeps_the_index_in_range() {
    let (mut state, _) = state_over("a a a", "a");
    state.next_match();
    state.next_match();
    assert_eq!(state.current_ordinal(), Some(3));
    state.forget_current();
    assert_eq!(state.match_count(), 2);
    assert_eq!(state.current_ordinal(), Some(2));
    state.forget_current();
    state.forget_current();
    assert_eq!(state.match_count(), 0);
    assert_eq!(state.current_match(), None);
    // And once more on an empty list.
    state.forget_current();
    assert_eq!(state.current_match(), None);
}

#[test]
fn closing_keeps_the_query_and_clearing_does_not() {
    let (mut state, _) = state_over("a", "a");
    state.close();
    assert!(!state.is_active());
    assert_eq!(state.query(), "a");
    state.clear();
    assert_eq!(state.query(), "");
}

#[test]
fn an_empty_query_matches_nothing() {
    let (state, _) = state_over("abc", "");
    assert_eq!(state.match_count(), 0);
    assert_eq!(state.query_len(), 0);
}
