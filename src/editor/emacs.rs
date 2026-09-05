//! Emacs editing model: the kill ring, the mark, and the `M-x` command table.
//!
//! None of this touches a buffer. The input layer maps keys to an
//! [`EmacsCommand`] and applies the effect; this module owns the state that has
//! to survive between keystrokes and the naming that `M-x` completion needs.

use std::collections::VecDeque;

use super::buffer::Position;

pub use super::emacs_commands::{EmacsCommand, command_names, complete, resolve};

/// Entries a kill ring holds before the oldest is dropped.
pub const DEFAULT_KILL_RING_SIZE: usize = 60;

/// Emacs kill ring: a bounded, cycling history of killed text.
///
/// Consecutive kills join into one entry, which is what makes repeated `C-k`
/// accumulate a whole region instead of leaving one line per press. Any command
/// that is not a kill calls [`KillRing::end_kill_sequence`] to break the run.
#[derive(Debug, Clone)]
pub struct KillRing {
    entries: VecDeque<String>,
    index: usize,
    capacity: usize,
    in_kill_sequence: bool,
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new()
    }
}

impl KillRing {
    /// Creates a ring holding [`DEFAULT_KILL_RING_SIZE`] entries.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_KILL_RING_SIZE)
    }

    /// Creates a ring holding at most `capacity` entries.
    ///
    /// A capacity of zero is raised to one; a ring that cannot hold anything
    /// would silently discard every kill.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            index: 0,
            capacity: capacity.max(1),
            in_kill_sequence: false,
        }
    }

    /// Returns how many entries the ring holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when nothing has been killed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the entry limit.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Drops every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index = 0;
        self.in_kill_sequence = false;
    }

    /// Records `text` as a new entry at the front of the ring.
    ///
    /// Empty text is ignored, matching Emacs: killing nothing does not push a
    /// blank entry over the previous one.
    pub fn kill(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        self.entries.push_front(text);
        while self.entries.len() > self.capacity {
            self.entries.pop_back();
        }
        self.index = 0;
        self.in_kill_sequence = true;
    }

    /// Appends `text` to the newest entry when the previous command was a kill,
    /// and otherwise starts a new entry.
    pub fn kill_append(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        if self.in_kill_sequence
            && let Some(front) = self.entries.front_mut()
        {
            front.push_str(&text);
            self.index = 0;
            return;
        }
        self.kill(text);
    }

    /// Prepends `text` to the newest entry when the previous command was a kill.
    ///
    /// Backward kills grow the entry leftwards so the text stays in reading
    /// order.
    pub fn kill_prepend(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        if self.in_kill_sequence
            && let Some(front) = self.entries.front_mut()
        {
            front.insert_str(0, &text);
            self.index = 0;
            return;
        }
        self.kill(text);
    }

    /// Ends the current run of kills, so the next one starts a fresh entry.
    pub fn end_kill_sequence(&mut self) {
        self.in_kill_sequence = false;
    }

    /// Returns the entry a yank would insert.
    #[must_use]
    pub fn yank(&self) -> Option<&str> {
        self.entries.get(self.index).map(String::as_str)
    }

    /// Steps to the next older entry and returns it, wrapping at the end.
    pub fn yank_pop(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        self.index = (self.index + 1) % self.entries.len();
        self.entries.get(self.index).map(String::as_str)
    }

    /// Returns the entries, newest first. Used by tests and by a ring browser.
    #[must_use]
    pub fn entries(&self) -> Vec<&str> {
        self.entries.iter().map(String::as_str).collect()
    }
}

/// The Emacs mark, and whether the region it defines is active.
///
/// Deactivating keeps the mark where it is: `C-x C-x` after `C-g` still finds
/// it, which is the behaviour Emacs users expect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkState {
    mark: Option<Position>,
    active: bool,
}

impl MarkState {
    /// Creates state with no mark set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mark: None,
            active: false,
        }
    }

    /// Sets the mark and activates the region.
    pub fn set_mark(&mut self, pos: Position) {
        self.mark = Some(pos);
        self.active = true;
    }

    /// Returns the mark, set or not, active or not.
    #[must_use]
    pub const fn mark(&self) -> Option<Position> {
        self.mark
    }

    /// Returns true when the region between point and mark is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active && self.mark.is_some()
    }

    /// Deactivates the region without forgetting the mark.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Forgets the mark entirely.
    pub fn clear(&mut self) {
        self.mark = None;
        self.active = false;
    }

    /// Swaps point and mark, returning the position the point moves to.
    ///
    /// Returns `None` when no mark is set, leaving the state untouched.
    pub fn exchange_point_and_mark(&mut self, point: Position) -> Option<Position> {
        let old = self.mark?;
        self.mark = Some(point);
        self.active = true;
        Some(old)
    }

    /// Returns the active region as an ordered pair.
    #[must_use]
    pub fn region(&self, point: Position) -> Option<(Position, Position)> {
        if !self.active {
            return None;
        }
        let mark = self.mark?;
        if (mark.line, mark.col) <= (point.line, point.col) {
            Some((mark, point))
        } else {
            Some((point, mark))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn kills_stack_newest_first() {
        let mut ring = KillRing::new();
        ring.kill("one");
        ring.kill("two");
        assert_eq!(ring.yank(), Some("two"));
        assert_eq!(ring.entries(), vec!["two", "one"]);
    }

    #[test]
    fn consecutive_kills_join_into_one_entry() {
        let mut ring = KillRing::new();
        ring.kill("abc");
        ring.kill_append("def");
        ring.kill_append("ghi");
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.yank(), Some("abcdefghi"));
    }

    #[test]
    fn a_broken_sequence_starts_a_new_entry() {
        let mut ring = KillRing::new();
        ring.kill("abc");
        ring.end_kill_sequence();
        ring.kill_append("def");
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.yank(), Some("def"));
    }

    #[test]
    fn backward_kills_grow_the_entry_leftwards() {
        let mut ring = KillRing::new();
        ring.kill("world");
        ring.kill_prepend("hello ");
        assert_eq!(ring.yank(), Some("hello world"));
    }

    #[test]
    fn yank_pop_cycles_through_the_ring() {
        let mut ring = KillRing::new();
        ring.kill("a");
        ring.kill("b");
        ring.kill("c");
        assert_eq!(ring.yank(), Some("c"));
        assert_eq!(ring.yank_pop(), Some("b"));
        assert_eq!(ring.yank_pop(), Some("a"));
        assert_eq!(ring.yank_pop(), Some("c"));
        assert_eq!(ring.yank(), Some("c"));
    }

    #[test]
    fn a_new_kill_resets_the_yank_position() {
        let mut ring = KillRing::new();
        ring.kill("a");
        ring.kill("b");
        assert_eq!(ring.yank_pop(), Some("a"));
        ring.end_kill_sequence();
        ring.kill("c");
        assert_eq!(ring.yank(), Some("c"));
    }

    #[test]
    fn the_ring_is_bounded_and_evicts_the_oldest() {
        let mut ring = KillRing::with_capacity(2);
        ring.kill("a");
        ring.kill("b");
        ring.kill("c");
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.entries(), vec!["c", "b"]);
        assert_eq!(ring.capacity(), 2);
    }

    #[test]
    fn a_zero_capacity_ring_still_holds_one_entry() {
        let mut ring = KillRing::with_capacity(0);
        ring.kill("a");
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn empty_kills_and_pops_are_no_ops() {
        let mut ring = KillRing::new();
        ring.kill("");
        ring.kill_append("");
        ring.kill_prepend("");
        assert!(ring.is_empty());
        assert_eq!(ring.yank(), None);
        assert_eq!(ring.yank_pop(), None);
    }

    #[test]
    fn clearing_empties_the_ring() {
        let mut ring = KillRing::new();
        ring.kill("a");
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.yank(), None);
    }

    #[test]
    fn the_mark_starts_unset() {
        let state = MarkState::new();
        assert_eq!(state.mark(), None);
        assert!(!state.is_active());
        assert_eq!(state.region(Position::new(0, 0)), None);
    }

    #[test]
    fn setting_the_mark_activates_an_ordered_region() {
        let mut state = MarkState::new();
        state.set_mark(Position::new(2, 4));
        assert!(state.is_active());
        assert_eq!(
            state.region(Position::new(5, 0)),
            Some((Position::new(2, 4), Position::new(5, 0)))
        );
        // A point before the mark still comes back in order.
        assert_eq!(
            state.region(Position::new(1, 0)),
            Some((Position::new(1, 0), Position::new(2, 4)))
        );
    }

    #[test]
    fn exchange_swaps_point_and_mark() {
        let mut state = MarkState::new();
        state.set_mark(Position::new(1, 1));
        let new_point = state.exchange_point_and_mark(Position::new(7, 3));
        assert_eq!(new_point, Some(Position::new(1, 1)));
        assert_eq!(state.mark(), Some(Position::new(7, 3)));
    }

    #[test]
    fn exchange_without_a_mark_changes_nothing() {
        let mut state = MarkState::new();
        assert_eq!(state.exchange_point_and_mark(Position::new(3, 0)), None);
        assert_eq!(state.mark(), None);
    }

    #[test]
    fn deactivating_keeps_the_mark_but_drops_the_region() {
        let mut state = MarkState::new();
        state.set_mark(Position::new(2, 0));
        state.deactivate();
        assert!(!state.is_active());
        assert_eq!(state.mark(), Some(Position::new(2, 0)));
        assert_eq!(state.region(Position::new(4, 0)), None);
        // Exchange reactivates it.
        assert_eq!(
            state.exchange_point_and_mark(Position::new(4, 0)),
            Some(Position::new(2, 0))
        );
        assert!(state.is_active());
    }

    #[test]
    fn clearing_forgets_the_mark() {
        let mut state = MarkState::new();
        state.set_mark(Position::new(1, 1));
        state.clear();
        assert_eq!(state.mark(), None);
        assert!(!state.is_active());
    }
}
