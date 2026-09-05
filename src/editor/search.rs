//! Search and replace: the state behind the find bar.
//!
//! [`SearchState`] owns the query, the replacement, which field the user is
//! typing into, and where the matches are. It never touches a buffer itself —
//! [`Editor`](super::Editor) applies the replacements — so every rule here,
//! including how wrapping behaves at each end of the document, is testable on
//! its own.

use super::buffer::{Buffer, Position};

/// Matches beyond this count are not tracked.
///
/// A query like `e` over a large file has no useful "match 40,000 of 90,000",
/// and collecting them all would stall the frame that started the search.
pub const MAX_TRACKED_MATCHES: usize = 20_000;

/// Which field of the find bar has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchField {
    /// The text being looked for.
    #[default]
    Query,
    /// The text it is replaced with.
    Replacement,
}

impl SearchField {
    /// Returns the other field.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Query => Self::Replacement,
            Self::Replacement => Self::Query,
        }
    }
}

/// Which way `Enter` moves through the matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// Towards the end of the document.
    #[default]
    Forward,
    /// Towards the start.
    Backward,
}

/// The find bar's state.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    active: bool,
    replacing: bool,
    field: SearchField,
    query: String,
    replacement: String,
    case_sensitive: bool,
    direction: SearchDirection,
    matches: Vec<Position>,
    current: Option<usize>,
    /// True when the buffer changed since the matches were computed.
    stale: bool,
    /// True when the last navigation ran off an end and came back round.
    wrapped: bool,
}

impl SearchState {
    /// Creates a closed find bar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            case_sensitive: false,
            ..Self::default()
        }
    }

    /// Returns true when the find bar is open.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Returns true when the replace field is shown.
    #[must_use]
    pub const fn is_replacing(&self) -> bool {
        self.replacing
    }

    /// Returns which field has focus.
    #[must_use]
    pub const fn field(&self) -> SearchField {
        self.field
    }

    /// Returns the query text.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the replacement text.
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns true when the search distinguishes case.
    #[must_use]
    pub const fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Returns the direction `Enter` moves in.
    #[must_use]
    pub const fn direction(&self) -> SearchDirection {
        self.direction
    }

    /// Returns every match, in document order.
    #[must_use]
    pub fn matches(&self) -> &[Position] {
        &self.matches
    }

    /// Returns how many matches the query has.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns the position of the current match, if there is one.
    #[must_use]
    pub fn current_match(&self) -> Option<Position> {
        self.current.and_then(|i| self.matches.get(i)).copied()
    }

    /// Returns the current match's 1-based ordinal, for display.
    #[must_use]
    pub fn current_ordinal(&self) -> Option<usize> {
        self.current.map(|i| i + 1)
    }

    /// Returns true when the last move wrapped around an end of the document.
    #[must_use]
    pub const fn wrapped(&self) -> bool {
        self.wrapped
    }

    /// Returns the count as the find bar shows it, such as `3/12`.
    #[must_use]
    pub fn count_label(&self) -> String {
        if self.query.is_empty() {
            return String::new();
        }
        if self.matches.is_empty() {
            return "0/0".to_string();
        }
        format!(
            "{}/{}",
            self.current_ordinal().unwrap_or(0),
            self.matches.len()
        )
    }

    /// Returns the length of the query in characters, which is how wide a match
    /// is on screen.
    #[must_use]
    pub fn query_len(&self) -> usize {
        self.query.chars().count()
    }

    /// Returns the columns a match occupies on `line`, as `(start, end)` pairs.
    ///
    /// Only matches that begin on that line are reported; a query containing a
    /// newline is highlighted from its first line only.
    #[must_use]
    pub fn matches_on_line(&self, line: usize) -> Vec<(usize, usize, bool)> {
        let width = self.query_len();
        self.matches
            .iter()
            .enumerate()
            .filter(|(_, p)| p.line == line)
            .map(|(i, p)| (p.col, p.col + width, Some(i) == self.current))
            .collect()
    }

    /// Opens the find bar, optionally with the replace field.
    pub fn open(&mut self, replacing: bool) {
        self.active = true;
        self.replacing = replacing;
        self.field = SearchField::Query;
        self.stale = true;
    }

    /// Closes the find bar, keeping the query for next time.
    pub fn close(&mut self) {
        self.active = false;
        self.replacing = false;
        self.matches.clear();
        self.current = None;
        self.wrapped = false;
    }

    /// Closes the bar and forgets the query too.
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Moves focus to the other field. Does nothing unless replace is shown.
    pub fn toggle_field(&mut self) {
        if self.replacing {
            self.field = self.field.toggled();
        }
    }

    /// Turns case sensitivity on or off, marking the matches stale.
    pub fn set_case_sensitive(&mut self, on: bool) {
        if self.case_sensitive != on {
            self.case_sensitive = on;
            self.stale = true;
        }
    }

    /// Sets the direction `Enter` moves in.
    pub fn set_direction(&mut self, direction: SearchDirection) {
        self.direction = direction;
    }

    /// Replaces the query text.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();
        if self.query != query {
            self.query = query;
            self.stale = true;
        }
    }

    /// Replaces the replacement text.
    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        self.replacement = replacement.into();
    }

    /// Appends a character to the focused field.
    pub fn push_char(&mut self, c: char) {
        match self.field {
            SearchField::Query => {
                self.query.push(c);
                self.stale = true;
            }
            SearchField::Replacement => self.replacement.push(c),
        }
    }

    /// Removes the last character of the focused field.
    pub fn pop_char(&mut self) {
        match self.field {
            SearchField::Query => {
                self.query.pop();
                self.stale = true;
            }
            SearchField::Replacement => {
                self.replacement.pop();
            }
        }
    }

    /// Marks the match list as needing recomputation.
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Returns true when the match list no longer reflects the buffer.
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        self.stale
    }

    /// Recomputes the matches against `buffer`.
    ///
    /// The current match is kept if the same position still matches, so typing
    /// another character of the query does not jump the view somewhere else.
    pub fn refresh(&mut self, buffer: &Buffer) {
        let previous = self.current_match();
        self.matches = self.scan(buffer);
        self.current = previous
            .and_then(|p| self.matches.iter().position(|m| *m == p))
            .or(if self.matches.is_empty() {
                None
            } else {
                Some(0)
            });
        self.stale = false;
    }

    /// Collects the matches, honouring the case-sensitivity flag.
    fn scan(&self, buffer: &Buffer) -> Vec<Position> {
        if self.query.is_empty() {
            return Vec::new();
        }
        if self.case_sensitive {
            buffer.find(&self.query).take(MAX_TRACKED_MATCHES).collect()
        } else {
            buffer
                .find_case_insensitive(&self.query)
                .take(MAX_TRACKED_MATCHES)
                .collect()
        }
    }

    /// Selects the first match at or after `from`.
    ///
    /// Used when the bar opens so the first `Enter` moves on from where the
    /// cursor already is rather than jumping to the top of the file.
    pub fn select_from(&mut self, from: Position) {
        self.current = self
            .matches
            .iter()
            .position(|m| (m.line, m.col) >= (from.line, from.col))
            .or(if self.matches.is_empty() {
                None
            } else {
                Some(0)
            });
        self.wrapped = false;
    }

    /// Moves to the next match, wrapping at the end of the document.
    ///
    /// Returns the new current match. Wrapping is recorded in
    /// [`SearchState::wrapped`] so the bar can say so.
    pub fn next_match(&mut self) -> Option<Position> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let next = match self.current {
            Some(i) if i + 1 < self.matches.len() => {
                self.wrapped = false;
                i + 1
            }
            Some(_) => {
                self.wrapped = true;
                0
            }
            None => {
                self.wrapped = false;
                0
            }
        };
        self.current = Some(next);
        self.matches.get(next).copied()
    }

    /// Moves to the previous match, wrapping at the start of the document.
    pub fn prev_match(&mut self) -> Option<Position> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let last = self.matches.len() - 1;
        let prev = match self.current {
            Some(0) => {
                self.wrapped = true;
                last
            }
            Some(i) => {
                self.wrapped = false;
                i - 1
            }
            None => {
                self.wrapped = false;
                last
            }
        };
        self.current = Some(prev);
        self.matches.get(prev).copied()
    }

    /// Moves in the bar's current direction.
    pub fn advance(&mut self) -> Option<Position> {
        match self.direction {
            SearchDirection::Forward => self.next_match(),
            SearchDirection::Backward => self.prev_match(),
        }
    }

    /// Drops the current match from the list after it has been replaced.
    ///
    /// Positions after the replacement shift by the length difference; the
    /// caller re-runs [`SearchState::refresh`] for a full recount, but this
    /// keeps the index sane in between.
    pub fn forget_current(&mut self) {
        let Some(index) = self.current else { return };
        if index >= self.matches.len() {
            self.current = None;
            return;
        }
        self.matches.remove(index);
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(index.min(self.matches.len() - 1))
        };
    }
}

#[cfg(test)]
mod tests;
