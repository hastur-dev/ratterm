//! Cursor position, filter editing and layout arithmetic for the fleet view.
//!
//! Split out of `fleet_view.rs`. Nothing here touches a terminal or a daemon,
//! so the scrolling rule, the selection rule and the whole key map are tested
//! directly. [`handle_key`] is the entry point the application dispatches to:
//! it moves the cursor itself and returns what the caller has to do about
//! anything it cannot do alone.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// How many recent events the pane shows.
pub const EVENT_PANE_ROWS: usize = 5;

/// What the caller must do after a key was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetAction {
    /// Nothing beyond the cursor movement already applied.
    None,
    /// Leave the fleet view.
    Close,
    /// Reconnect and refresh every host.
    Refresh,
    /// The sort order should move on; read it back from the fleet.
    CycleSort,
    /// The filter text changed; read it back with [`FleetViewState::filter`].
    FilterChanged,
    /// Act on the selected row — open a shell in that container.
    Activate,
    /// Start the selected container.
    Start,
    /// Stop the selected container.
    Stop,
    /// Restart the selected container.
    Restart,
}

/// Where the cursor is in the fleet list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetViewState {
    selected: usize,
    show_events: bool,
    filter: String,
    editing_filter: bool,
}

impl Default for FleetViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl FleetViewState {
    /// A state with the first row selected and the events pane open.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            selected: 0,
            show_events: true,
            filter: String::new(),
            editing_filter: false,
        }
    }

    /// The filter text as typed.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// True while the filter line is taking keystrokes.
    #[must_use]
    pub const fn editing_filter(&self) -> bool {
        self.editing_filter
    }

    /// The selected row index.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// True when the events pane is drawn.
    #[must_use]
    pub const fn events_shown(&self) -> bool {
        self.show_events
    }

    /// Shows or hides the events pane.
    pub const fn toggle_events(&mut self) {
        self.show_events = !self.show_events;
    }

    /// Moves down one row, wrapping at the end.
    ///
    /// An empty list and the last row both land on zero: there is nowhere
    /// below either of them.
    pub const fn select_next(&mut self, len: usize) {
        if len == 0 || self.selected + 1 >= len {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Moves up one row, wrapping at the start.
    pub const fn select_prev(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
        } else if self.selected == 0 {
            self.selected = len - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Selects the first row.
    pub const fn select_first(&mut self) {
        self.selected = 0;
    }

    /// Selects the last row.
    pub const fn select_last(&mut self, len: usize) {
        self.selected = len.saturating_sub(1);
    }

    /// Pulls the selection back inside the list.
    ///
    /// Called after a refresh or a filter change: the list can shrink under
    /// the cursor, and a selection past the end would render nothing.
    pub const fn clamp(&mut self, len: usize) {
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
    }
}

/// Applies one key press to the fleet view.
///
/// Cursor movement, the events pane and the filter buffer are all applied
/// here, because they need nothing but this state. Anything that needs a
/// daemon — refreshing, acting on a container — comes back as a
/// [`FleetAction`] for the caller to carry out.
///
/// While the filter line is open it takes every printable key, so a container
/// named `r` can be searched for without triggering a refresh. Escape closes
/// the filter first and the view second.
pub fn handle_key(state: &mut FleetViewState, key: KeyEvent, len: usize) -> FleetAction {
    if state.editing_filter {
        return handle_filter_key(state, key);
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => FleetAction::Close,
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_next(len);
            FleetAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_prev(len);
            FleetAction::None
        }
        KeyCode::Home | KeyCode::Char('g') => {
            state.select_first();
            FleetAction::None
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.select_last(len);
            FleetAction::None
        }
        KeyCode::Char('r') if !ctrl => FleetAction::Refresh,
        KeyCode::Char('s') => FleetAction::CycleSort,
        KeyCode::Char('e') => {
            state.toggle_events();
            FleetAction::None
        }
        KeyCode::Char('/') => {
            state.editing_filter = true;
            FleetAction::None
        }
        KeyCode::Char('c') => {
            if state.filter.is_empty() {
                FleetAction::None
            } else {
                state.filter.clear();
                state.selected = 0;
                FleetAction::FilterChanged
            }
        }
        KeyCode::Enter => FleetAction::Activate,
        KeyCode::Char('S') => FleetAction::Start,
        KeyCode::Char('X') => FleetAction::Stop,
        KeyCode::Char('R') => FleetAction::Restart,
        _ => FleetAction::None,
    }
}

/// Handles a key while the filter line is open.
fn handle_filter_key(state: &mut FleetViewState, key: KeyEvent) -> FleetAction {
    match key.code {
        KeyCode::Esc => {
            state.editing_filter = false;
            if state.filter.is_empty() {
                FleetAction::None
            } else {
                state.filter.clear();
                state.selected = 0;
                FleetAction::FilterChanged
            }
        }
        KeyCode::Enter => {
            state.editing_filter = false;
            FleetAction::None
        }
        KeyCode::Backspace => {
            if state.filter.pop().is_some() {
                state.selected = 0;
                FleetAction::FilterChanged
            } else {
                FleetAction::None
            }
        }
        KeyCode::Char(c) => {
            state.filter.push(c);
            state.selected = 0;
            FleetAction::FilterChanged
        }
        _ => FleetAction::None,
    }
}

/// The slice of rows a list of `height` lines should draw.
///
/// Keeps the selected row on screen, scrolling only as far as it must, and
/// returns a half-open `(start, end)` range that is always inside the list.
#[must_use]
pub fn visible_window(total: usize, selected: usize, height: usize) -> (usize, usize) {
    if total == 0 || height == 0 {
        return (0, 0);
    }

    let selected = selected.min(total - 1);
    let start = if selected >= height {
        selected + 1 - height
    } else {
        0
    };
    let start = start.min(total.saturating_sub(height));
    (start, (start + height).min(total))
}

/// Shortens `text` to fit `width`, ending in an ellipsis when it had to.
#[must_use]
pub fn fit(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 3 {
        return text.chars().take(width).collect();
    }
    let kept: String = text.chars().take(width - 3).collect();
    format!("{kept}...")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "fleet_nav_tests.rs"]
mod tests;
