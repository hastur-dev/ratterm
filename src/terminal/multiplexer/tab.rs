//! Terminal tab containing a grid of terminals.

use crate::terminal::Terminal;

use super::grid::TerminalGrid;
use super::types::{DummyTerminalRef, SplitDirection, SplitFocus};

/// A terminal tab containing a grid of terminals (up to 2x2).
pub struct TerminalTab {
    /// Terminal grid (replaces old split system).
    pub grid: TerminalGrid,
    /// Tab name/title.
    pub name: String,
    /// Tab index.
    pub index: usize,
    // Legacy fields for backward compatibility in rendering
    /// Primary terminal instance (reference to grid\[0\]).
    pub terminal: DummyTerminalRef,
    /// Secondary terminal (for backward compat - always None now).
    pub split_terminal: Option<DummyTerminalRef>,
    /// Split direction (computed from grid state).
    pub split: SplitDirection,
    /// Which split pane is focused (computed from grid state).
    pub split_focus: SplitFocus,
}

impl TerminalTab {
    /// Creates a new terminal tab with a grid.
    pub fn new(grid: TerminalGrid, name: String, index: usize) -> Self {
        let (split, split_focus) = Self::compute_split_state(&grid);
        Self {
            grid,
            name,
            index,
            terminal: DummyTerminalRef,
            split_terminal: None,
            split,
            split_focus,
        }
    }

    /// Computes legacy split state from grid.
    fn compute_split_state(grid: &TerminalGrid) -> (SplitDirection, SplitFocus) {
        let (cols, rows) = grid.dimensions();
        let focus = grid.focused_index();

        let split = match (cols, rows) {
            (1, 1) => SplitDirection::None,
            (2, 1) => SplitDirection::Vertical,
            (1, 2) => SplitDirection::Horizontal,
            (2, 2) => SplitDirection::Vertical,
            _ => SplitDirection::None,
        };

        let split_focus = if focus == 0 {
            SplitFocus::First
        } else {
            SplitFocus::Second
        };

        (split, split_focus)
    }

    /// Updates legacy split state from grid.
    pub fn update_split_state(&mut self) {
        let (split, split_focus) = Self::compute_split_state(&self.grid);
        self.split = split;
        self.split_focus = split_focus;
    }

    /// Returns the tab name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the focused terminal.
    pub fn focused_terminal(&self) -> Option<&Terminal> {
        self.grid.focused()
    }

    /// Returns the focused terminal mutably.
    pub fn focused_terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.grid.focused_mut()
    }

    /// Returns all terminals in the grid.
    pub fn terminals(&self) -> Vec<&Terminal> {
        self.grid.all_terminals()
    }

    /// Returns all terminals mutably.
    pub fn terminals_mut(&mut self) -> Vec<&mut Terminal> {
        self.grid.all_terminals_mut()
    }
}
