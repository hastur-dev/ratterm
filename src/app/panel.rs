//! Shared state for the panels and dashboards.
//!
//! Every list in the interface — references, code actions, symbols,
//! diagnostics, hosts, containers — used to carry its own three fields on
//! `App`: the items, a selected index, and a scroll offset. Nineteen of them
//! were LSP alone. Each set was navigated by its own hand-written code, so a
//! fix to one did not reach the others, and an index could outlive the list it
//! pointed into.
//!
//! [`ListPanel`] is that state once. Selection is bounded by construction: a
//! panel holding no items has no selection, and replacing the items resets it
//! rather than leaving it pointing past the end.
//!
//! [`Panel`] is what the application asks of a panel — a title, whether it is
//! open, and what a key did — so `App` can treat them alike instead of naming
//! each one in every match.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crossterm::event::KeyEvent;

/// A list the user navigates, with its selection and scroll position.
///
/// `None` items means the panel is closed. That is not the same as an open
/// panel with nothing in it, which is what "no references found" looks like,
/// and conflating the two is why an empty result used to render as a blank
/// box with no explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPanel<T> {
    items: Option<Vec<T>>,
    selected: usize,
    scroll: usize,
}

impl<T> Default for ListPanel<T> {
    fn default() -> Self {
        Self::closed()
    }
}

impl<T> ListPanel<T> {
    /// A closed panel.
    #[must_use]
    pub const fn closed() -> Self {
        Self {
            items: None,
            selected: 0,
            scroll: 0,
        }
    }

    /// Opens the panel on `items`, selecting the first.
    pub fn open(&mut self, items: Vec<T>) {
        self.items = Some(items);
        self.selected = 0;
        self.scroll = 0;
    }

    /// Closes the panel and forgets its contents.
    pub fn close(&mut self) {
        self.items = None;
        self.selected = 0;
        self.scroll = 0;
    }

    /// True if the panel is open, whether or not it has anything in it.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.items.is_some()
    }

    /// True if the panel is open and has something to show.
    #[must_use]
    pub fn has_items(&self) -> bool {
        self.len() > 0
    }

    /// The items, or an empty slice when closed.
    #[must_use]
    pub fn items(&self) -> &[T] {
        self.items.as_deref().unwrap_or(&[])
    }

    /// The items, if the panel is open.
    #[must_use]
    pub fn opened(&self) -> Option<&[T]> {
        self.items.as_deref()
    }

    /// How many items there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.as_ref().map_or(0, Vec::len)
    }

    /// True when there is nothing to select.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The selected index, which is always within the list when there is one.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// The scroll offset.
    #[must_use]
    pub const fn scroll(&self) -> usize {
        self.scroll
    }

    /// The selected item, if there is one.
    #[must_use]
    pub fn selected_item(&self) -> Option<&T> {
        self.items.as_ref()?.get(self.selected)
    }

    /// Moves the selection down, stopping at the last item.
    ///
    /// Stopping rather than wrapping: these are result lists, where wrapping
    /// from the last hit back to the first hides that you reached the end.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.len() {
            self.selected += 1;
        }
    }

    /// Moves the selection up, stopping at the first item.
    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Selects the first item.
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    /// Selects the last item.
    pub fn select_last(&mut self) {
        self.selected = self.len().saturating_sub(1);
    }

    /// Selects an index, ignoring one that is out of range.
    pub fn select(&mut self, index: usize) {
        if index < self.len() {
            self.selected = index;
        }
    }

    /// Scrolls so the selection is inside a window `height` rows tall.
    ///
    /// A height of zero leaves the offset alone: there is no window to be
    /// inside, and computing one would divide the row count by nothing.
    pub fn ensure_visible(&mut self, height: usize) {
        if height == 0 {
            return;
        }

        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }

        // A list that shrank can leave the offset past its end.
        let max_scroll = self.len().saturating_sub(height);
        self.scroll = self.scroll.min(max_scroll);
    }

    /// Moves the selection down within `rows`, stopping at the last.
    ///
    /// For a panel whose visible rows are derived from its items rather than
    /// being one per item: a references panel holds one item per file and
    /// draws one row per location, and a symbol outline holds a tree and draws
    /// it flattened. Passing the row count keeps the bound correct without the
    /// panel having to know how the rows are produced.
    pub fn select_next_in(&mut self, rows: usize) {
        if self.selected + 1 < rows {
            self.selected += 1;
        }
    }

    /// Selects the last of `rows`.
    pub fn select_last_in(&mut self, rows: usize) {
        self.selected = rows.saturating_sub(1);
    }

    /// Scrolls so the selection is visible within `rows` rendered rows.
    pub fn ensure_visible_in(&mut self, rows: usize, height: usize) {
        if height == 0 {
            return;
        }

        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }

        self.scroll = self.scroll.min(rows.saturating_sub(height));
    }

    /// Moves the selection inside `rows`, for a list that changed size.
    pub fn clamp_to(&mut self, rows: usize) {
        self.selected = self.selected.min(rows.saturating_sub(1));
    }

    /// Fills the panel, opening it if it was closed.
    ///
    /// What a refresh does: the first result opens the panel, and each one
    /// after keeps the user where they were rather than throwing them back to
    /// the top of a list that mostly did not change.
    pub fn replace_or_open(&mut self, items: Vec<T>) {
        if self.is_open() {
            self.replace(items);
        } else {
            self.open(items);
        }
    }

    /// Replaces the items, keeping the panel open and clamping the selection.
    ///
    /// Used when a list refreshes under the user: the selection should stay
    /// where it was if that row still exists, and land on the last row if the
    /// list got shorter, rather than pointing past the end.
    pub fn replace(&mut self, items: Vec<T>) {
        let length = items.len();
        self.items = Some(items);
        self.selected = self.selected.min(length.saturating_sub(1));
        self.scroll = self.scroll.min(length.saturating_sub(1));
    }
}

/// What a panel did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelOutcome {
    /// The panel used the key.
    Handled,
    /// The panel did not use the key; the caller should try something else.
    Ignored,
    /// The panel used the key and wants to close.
    Close,
}

impl PanelOutcome {
    /// True if the caller should stop looking for a handler.
    #[must_use]
    pub const fn is_consumed(self) -> bool {
        matches!(self, Self::Handled | Self::Close)
    }
}

/// What the application asks of a panel.
///
/// The point is uniformity: `App` decides which panel has focus, asks that one
/// to handle the key and to draw itself, and does not need to know which panel
/// it is. Panels that need more than this keep their extra methods on their own
/// type; this is the part everything shares.
pub trait Panel {
    /// The title shown on the panel's border.
    fn title(&self) -> String;

    /// True while the panel is on screen.
    fn is_open(&self) -> bool;

    /// Closes the panel.
    fn close(&mut self);

    /// Handles one key.
    fn handle_key(&mut self, key: KeyEvent) -> PanelOutcome;

    /// Draws the panel.
    fn render(&self, area: Rect, buf: &mut Buffer, focused: bool);

    /// A one-line hint for the key bar, if the panel has one.
    ///
    /// Defaulted because most panels share the same navigation keys and
    /// repeating the string in each would be one more thing to drift.
    fn key_hints(&self) -> &'static str {
        "[Up/Down] Navigate  [Enter] Select  [Esc] Close"
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A panel holding three rows.
    fn three() -> ListPanel<&'static str> {
        let mut panel = ListPanel::closed();
        panel.open(vec!["one", "two", "three"]);
        panel
    }

    #[test]
    fn a_new_panel_is_closed_and_empty() {
        let panel: ListPanel<u8> = ListPanel::closed();
        assert!(!panel.is_open());
        assert!(panel.is_empty());
        assert_eq!(panel.len(), 0);
        assert_eq!(panel.selected_item(), None);
        assert_eq!(panel.opened(), None);
        assert!(panel.items().is_empty());
    }

    #[test]
    fn the_default_is_closed() {
        let panel: ListPanel<u8> = ListPanel::default();
        assert!(!panel.is_open());
    }

    #[test]
    fn an_open_but_empty_panel_is_not_the_same_as_a_closed_one() {
        // "No references found" and "the references panel is not open" are
        // different states and the interface must be able to tell them apart.
        let mut panel: ListPanel<u8> = ListPanel::closed();
        panel.open(Vec::new());

        assert!(panel.is_open(), "it is open");
        assert!(!panel.has_items(), "with nothing in it");
        assert_eq!(panel.opened(), Some(&[][..]));
    }

    #[test]
    fn opening_selects_the_first_item() {
        let panel = three();
        assert!(panel.is_open());
        assert_eq!(panel.selected(), 0);
        assert_eq!(panel.selected_item(), Some(&"one"));
        assert_eq!(panel.len(), 3);
    }

    #[test]
    fn closing_forgets_everything() {
        let mut panel = three();
        panel.select_last();
        panel.close();

        assert!(!panel.is_open());
        assert_eq!(panel.selected(), 0);
        assert_eq!(panel.scroll(), 0);
        assert_eq!(panel.selected_item(), None);
    }

    #[test]
    fn navigation_stops_at_the_ends() {
        let mut panel = three();

        panel.select_previous();
        assert_eq!(panel.selected(), 0, "already at the top");

        panel.select_next();
        panel.select_next();
        assert_eq!(panel.selected(), 2);
        panel.select_next();
        assert_eq!(panel.selected(), 2, "already at the bottom");
    }

    #[test]
    fn navigating_an_empty_panel_does_nothing_rather_than_panicking() {
        let mut panel: ListPanel<u8> = ListPanel::closed();
        panel.open(Vec::new());

        panel.select_next();
        panel.select_previous();
        panel.select_last();
        panel.select_first();

        assert_eq!(panel.selected(), 0);
        assert_eq!(panel.selected_item(), None);
    }

    #[test]
    fn first_and_last_go_where_they_say() {
        let mut panel = three();
        panel.select_last();
        assert_eq!(panel.selected_item(), Some(&"three"));
        panel.select_first();
        assert_eq!(panel.selected_item(), Some(&"one"));
    }

    #[test]
    fn selecting_out_of_range_is_ignored() {
        let mut panel = three();
        panel.select(2);
        assert_eq!(panel.selected(), 2);
        panel.select(99);
        assert_eq!(panel.selected(), 2, "an impossible row is not selected");
    }

    #[test]
    fn scrolling_follows_the_selection_down() {
        let mut panel: ListPanel<usize> = ListPanel::closed();
        panel.open((0..20).collect());

        panel.select(5);
        panel.ensure_visible(5);
        assert_eq!(panel.scroll(), 1, "row 5 is the last of rows 1..5");

        panel.select(19);
        panel.ensure_visible(5);
        assert_eq!(panel.scroll(), 15);
    }

    #[test]
    fn scrolling_follows_the_selection_up() {
        let mut panel: ListPanel<usize> = ListPanel::closed();
        panel.open((0..20).collect());

        panel.select(19);
        panel.ensure_visible(5);
        panel.select(2);
        panel.ensure_visible(5);
        assert_eq!(panel.scroll(), 2);
    }

    #[test]
    fn a_window_taller_than_the_list_scrolls_to_the_top() {
        let mut panel = three();
        panel.select_last();
        panel.ensure_visible(50);
        assert_eq!(panel.scroll(), 0, "everything already fits");
    }

    #[test]
    fn a_zero_height_window_leaves_the_offset_alone() {
        let mut panel: ListPanel<usize> = ListPanel::closed();
        panel.open((0..20).collect());
        panel.select(10);
        panel.ensure_visible(5);
        let before = panel.scroll();

        panel.ensure_visible(0);
        assert_eq!(panel.scroll(), before);
    }

    #[test]
    fn replacing_keeps_the_selection_where_the_row_still_exists() {
        let mut panel = three();
        panel.select(1);
        panel.replace(vec!["a", "b", "c", "d"]);

        assert!(panel.is_open());
        assert_eq!(panel.selected(), 1);
        assert_eq!(panel.selected_item(), Some(&"b"));
    }

    #[test]
    fn replacing_with_a_shorter_list_moves_the_selection_into_it() {
        // This is the bug the type exists to remove: an index that outlives
        // the list it points into.
        let mut panel = three();
        panel.select_last();
        panel.replace(vec!["only"]);

        assert_eq!(panel.selected(), 0);
        assert_eq!(panel.selected_item(), Some(&"only"));
    }

    #[test]
    fn replacing_with_nothing_leaves_a_selection_that_selects_nothing() {
        let mut panel = three();
        panel.select_last();
        panel.replace(Vec::new());

        assert!(panel.is_open());
        assert_eq!(panel.selected_item(), None);
        assert_eq!(panel.selected(), 0);
    }

    #[test]
    fn a_shrinking_list_does_not_leave_the_scroll_past_the_end() {
        let mut panel: ListPanel<usize> = ListPanel::closed();
        panel.open((0..50).collect());
        panel.select(49);
        panel.ensure_visible(10);
        assert_eq!(panel.scroll(), 40);

        panel.replace((0..12).collect());
        panel.ensure_visible(10);
        assert!(panel.scroll() <= 2, "scroll was {}", panel.scroll());
    }

    #[test]
    fn a_panel_whose_rows_are_derived_is_bounded_by_the_rows() {
        // Three files holding seven references between them: seven rows, not
        // three. Bounding by item count would make four of them unreachable.
        let mut panel = three();

        for _ in 0..10 {
            panel.select_next_in(7);
        }
        assert_eq!(panel.selected(), 6, "the last of seven rows");

        panel.select_last_in(7);
        assert_eq!(panel.selected(), 6);
    }

    #[test]
    fn derived_rows_of_zero_leave_the_selection_at_the_top() {
        let mut panel = three();
        panel.select_next_in(0);
        assert_eq!(panel.selected(), 0);
        panel.select_last_in(0);
        assert_eq!(panel.selected(), 0);
    }

    #[test]
    fn clamping_moves_a_selection_into_a_shorter_list() {
        let mut panel = three();
        panel.select_last_in(20);
        assert_eq!(panel.selected(), 19);

        panel.clamp_to(5);
        assert_eq!(panel.selected(), 4);

        panel.clamp_to(0);
        assert_eq!(panel.selected(), 0);
    }

    #[test]
    fn scrolling_derived_rows_follows_the_selection() {
        let mut panel = three();
        panel.select_last_in(40);
        panel.ensure_visible_in(40, 10);
        assert_eq!(panel.scroll(), 30);

        panel.select(0);
        panel.ensure_visible_in(40, 10);
        assert_eq!(panel.scroll(), 0);
    }

    #[test]
    fn a_refresh_opens_a_closed_panel_and_keeps_an_open_one_in_place() {
        let mut panel: ListPanel<&'static str> = ListPanel::closed();

        panel.replace_or_open(vec!["a", "b", "c"]);
        assert!(panel.is_open());
        assert_eq!(panel.selected(), 0);

        panel.select(2);
        panel.replace_or_open(vec!["a", "b", "c", "d"]);
        assert_eq!(
            panel.selected(),
            2,
            "a refresh should not throw the user back to the top"
        );
    }

    #[test]
    fn an_outcome_says_whether_the_caller_should_stop() {
        assert!(PanelOutcome::Handled.is_consumed());
        assert!(PanelOutcome::Close.is_consumed());
        assert!(!PanelOutcome::Ignored.is_consumed());
    }
}
