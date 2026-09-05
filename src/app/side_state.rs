//! State for the Git and debugger screens.
//!
//! Both used to sit as loose fields on `App`, next to everything else, so
//! nothing said which fields belonged together or had to change together. Git
//! blame in particular was two fields — a flag and the data — that could
//! disagree: the flag on with no data is a blank column, and data with the
//! flag off is memory held for a view nobody can see.
//!
//! Grouping them makes those pairs impossible to separate by accident, and
//! gives each a place for the small rules that were previously repeated at
//! every call site.

use std::collections::HashMap;

use crate::debugger::breakpoints::BreakpointStore;
use crate::debugger::session::DebugSession;
use crate::git::BlameLine;
use crate::git::gutter::GutterMark;

/// What the Git integration is showing for the current file.
#[derive(Debug, Default)]
pub struct GitUiState {
    /// Change marks by line number, for the gutter.
    gutter: HashMap<usize, GutterMark>,
    /// Blame lines, present only while blame is showing.
    ///
    /// One field rather than a flag and a vector: "showing blame" and "having
    /// blame to show" were separate before, and could disagree.
    blame: Option<Vec<BlameLine>>,
}

impl GitUiState {
    /// A state with nothing loaded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The gutter marks for the current file.
    #[must_use]
    pub const fn gutter(&self) -> &HashMap<usize, GutterMark> {
        &self.gutter
    }

    /// Replaces the gutter marks.
    pub fn set_gutter(&mut self, marks: HashMap<usize, GutterMark>) {
        self.gutter = marks;
    }

    /// Forgets the gutter marks.
    pub fn clear_gutter(&mut self) {
        self.gutter.clear();
    }

    /// The mark on one line, if there is one.
    #[must_use]
    pub fn mark(&self, line: usize) -> Option<GutterMark> {
        self.gutter.get(&line).copied()
    }

    /// True while blame is showing.
    #[must_use]
    pub const fn is_blaming(&self) -> bool {
        self.blame.is_some()
    }

    /// The blame lines, or an empty slice when blame is off.
    #[must_use]
    pub fn blame(&self) -> &[BlameLine] {
        self.blame.as_deref().unwrap_or(&[])
    }

    /// Shows blame for `lines`.
    pub fn show_blame(&mut self, lines: Vec<BlameLine>) {
        self.blame = Some(lines);
    }

    /// Hides blame and releases its data.
    pub fn hide_blame(&mut self) {
        self.blame = None;
    }

    /// Forgets everything about the current file.
    ///
    /// Called when the editor changes file: gutter marks and blame both
    /// describe a specific document, and showing one file's marks beside
    /// another's text is worse than showing none.
    pub fn clear(&mut self) {
        self.gutter.clear();
        self.blame = None;
    }
}

/// What the debugger is doing.
#[derive(Debug)]
pub struct DebugUiState {
    /// The running session, if one is attached.
    session: Option<DebugSession>,
    /// Breakpoints, which outlive any one session.
    breakpoints: BreakpointStore,
    /// Whether the debug panel is on screen.
    panel_visible: bool,
}

impl DebugUiState {
    /// A state with no session, holding `breakpoints`.
    #[must_use]
    pub const fn new(breakpoints: BreakpointStore) -> Self {
        Self {
            session: None,
            breakpoints,
            panel_visible: false,
        }
    }

    /// The running session, if there is one.
    #[must_use]
    pub const fn session(&self) -> Option<&DebugSession> {
        self.session.as_ref()
    }

    /// The running session, mutably.
    pub const fn session_mut(&mut self) -> Option<&mut DebugSession> {
        self.session.as_mut()
    }

    /// True while a session is attached.
    #[must_use]
    pub const fn is_debugging(&self) -> bool {
        self.session.is_some()
    }

    /// Attaches a session and shows the panel.
    ///
    /// Showing the panel is part of starting: a debug session with its panel
    /// hidden gives no sign that anything happened.
    pub fn start(&mut self, session: DebugSession) {
        self.session = Some(session);
        self.panel_visible = true;
    }

    /// Detaches the session, returning it so the caller can shut it down.
    ///
    /// The panel stays as it was: the last stack trace is often the reason
    /// the session ended, and closing the panel would take it away.
    pub fn stop(&mut self) -> Option<DebugSession> {
        self.session.take()
    }

    /// The breakpoints, which are kept whether or not a session is running.
    #[must_use]
    pub const fn breakpoints(&self) -> &BreakpointStore {
        &self.breakpoints
    }

    /// The breakpoints, mutably.
    pub const fn breakpoints_mut(&mut self) -> &mut BreakpointStore {
        &mut self.breakpoints
    }

    /// True while the debug panel is on screen.
    #[must_use]
    pub const fn is_panel_visible(&self) -> bool {
        self.panel_visible
    }

    /// Shows or hides the debug panel.
    pub const fn set_panel_visible(&mut self, visible: bool) {
        self.panel_visible = visible;
    }

    /// Toggles the debug panel, returning its new state.
    pub const fn toggle_panel(&mut self) -> bool {
        self.panel_visible = !self.panel_visible;
        self.panel_visible
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A blame line for a given line number.
    fn blame_line(line: usize) -> BlameLine {
        BlameLine {
            short_hash: "abc1234".to_string(),
            author: "someone".to_string(),
            timestamp: 1_757_000_000,
            line_number: line,
            content: "a line".to_string(),
        }
    }

    #[test]
    fn a_new_git_state_shows_nothing() {
        let state = GitUiState::new();
        assert!(state.gutter().is_empty());
        assert!(!state.is_blaming());
        assert!(state.blame().is_empty());
        assert_eq!(state.mark(1), None);
    }

    #[test]
    fn gutter_marks_are_looked_up_by_line() {
        let mut state = GitUiState::new();
        let mut marks = HashMap::new();
        marks.insert(3, GutterMark::Added);
        marks.insert(7, GutterMark::Modified);
        state.set_gutter(marks);

        assert_eq!(state.mark(3), Some(GutterMark::Added));
        assert_eq!(state.mark(7), Some(GutterMark::Modified));
        assert_eq!(state.mark(4), None, "an unchanged line has no mark");
    }

    #[test]
    fn showing_blame_makes_it_available_and_hiding_releases_it() {
        let mut state = GitUiState::new();
        assert!(!state.is_blaming());

        state.show_blame(vec![blame_line(1), blame_line(2)]);
        assert!(state.is_blaming());
        assert_eq!(state.blame().len(), 2);

        state.hide_blame();
        assert!(!state.is_blaming());
        assert!(
            state.blame().is_empty(),
            "hiding blame should not keep its data"
        );
    }

    #[test]
    fn showing_blame_for_an_empty_file_is_still_showing_blame() {
        // A file with no commits has no blame lines, and that is a result to
        // display, not a reason to look as though nothing happened.
        let mut state = GitUiState::new();
        state.show_blame(Vec::new());
        assert!(state.is_blaming());
        assert!(state.blame().is_empty());
    }

    #[test]
    fn changing_file_forgets_both_the_marks_and_the_blame() {
        let mut state = GitUiState::new();
        let mut marks = HashMap::new();
        marks.insert(1, GutterMark::Added);
        state.set_gutter(marks);
        state.show_blame(vec![blame_line(1)]);

        state.clear();

        assert!(state.gutter().is_empty());
        assert!(!state.is_blaming());
    }

    #[test]
    fn clearing_the_gutter_leaves_blame_alone() {
        let mut state = GitUiState::new();
        state.show_blame(vec![blame_line(1)]);
        let mut marks = HashMap::new();
        marks.insert(1, GutterMark::Added);
        state.set_gutter(marks);

        state.clear_gutter();

        assert!(state.gutter().is_empty());
        assert!(state.is_blaming(), "they are refreshed independently");
    }

    #[test]
    fn a_new_debug_state_is_not_debugging() {
        let state = DebugUiState::new(BreakpointStore::new());
        assert!(!state.is_debugging());
        assert!(state.session().is_none());
        assert!(!state.is_panel_visible());
    }

    #[test]
    fn the_panel_toggles_and_reports_its_new_state() {
        let mut state = DebugUiState::new(BreakpointStore::new());

        assert!(state.toggle_panel());
        assert!(state.is_panel_visible());

        assert!(!state.toggle_panel());
        assert!(!state.is_panel_visible());
    }

    #[test]
    fn the_panel_can_be_set_directly() {
        let mut state = DebugUiState::new(BreakpointStore::new());
        state.set_panel_visible(true);
        assert!(state.is_panel_visible());
        state.set_panel_visible(true);
        assert!(state.is_panel_visible(), "setting it twice is not a toggle");
    }

    #[test]
    fn stopping_a_session_that_never_started_yields_nothing() {
        let mut state = DebugUiState::new(BreakpointStore::new());
        assert!(state.stop().is_none());
    }

    #[test]
    fn breakpoints_are_reachable_with_no_session() {
        // Setting a breakpoint before starting is the normal order of events.
        let mut state = DebugUiState::new(BreakpointStore::new());
        assert_eq!(state.breakpoints().count(), 0);
        let _ = state.breakpoints_mut();
        assert!(!state.is_debugging());
    }
}
