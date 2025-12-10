//! Terminal multiplexer for managing multiple terminal tabs with grid support.
//!
//! Provides tab management and 2x2 grid layouts for terminal instances.

use super::{Terminal, pty::PtyError};

/// Maximum number of terminal tabs.
const MAX_TABS: usize = 10;

/// Maximum terminals per grid (2x2).
const MAX_GRID_TERMINALS: usize = 4;

/// Direction for grid navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDirection {
    /// Move focus up.
    Up,
    /// Move focus down.
    Down,
    /// Move focus left.
    Left,
    /// Move focus right.
    Right,
}

/// Split direction for terminal panes (kept for backward compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDirection {
    /// No split - single terminal.
    #[default]
    None,
    /// Horizontal split (top/bottom).
    Horizontal,
    /// Vertical split (left/right).
    Vertical,
}

/// Which pane is focused in a split (kept for backward compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitFocus {
    /// First pane (top or left).
    #[default]
    First,
    /// Second pane (bottom or right).
    Second,
}

impl SplitFocus {
    /// Toggles between first and second pane.
    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

/// Grid layout state for terminals.
/// Supports up to 2x2 grid (4 terminals).
///
/// Grid positions:
/// ```text
/// ┌───┬───┐
/// │ 0 │ 1 │
/// ├───┼───┤
/// │ 2 │ 3 │
/// └───┴───┘
/// ```
pub struct TerminalGrid {
    /// Terminals in the grid (0=top-left, 1=top-right, 2=bottom-left, 3=bottom-right).
    terminals: [Option<Terminal>; MAX_GRID_TERMINALS],
    /// Currently focused terminal index (0-3).
    focus: usize,
    /// Number of columns in grid (1 or 2).
    cols: u8,
    /// Number of rows in grid (1 or 2).
    rows: u8,
    /// Grid width in terminal columns.
    width: u16,
    /// Grid height in terminal rows.
    height: u16,
}

impl TerminalGrid {
    /// Creates a new grid with a single terminal.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(
        cols: u16,
        rows: u16,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let terminal = Terminal::with_shell(cols, rows, shell_path)?;

        Ok(Self {
            terminals: [Some(terminal), None, None, None],
            focus: 0,
            cols: 1,
            rows: 1,
            width: cols,
            height: rows,
        })
    }

    /// Returns the number of active terminals in the grid.
    #[must_use]
    pub fn terminal_count(&self) -> usize {
        self.terminals.iter().filter(|t| t.is_some()).count()
    }

    /// Returns the grid dimensions (cols, rows).
    #[must_use]
    pub const fn dimensions(&self) -> (u8, u8) {
        (self.cols, self.rows)
    }

    /// Returns the focused terminal index.
    #[must_use]
    pub const fn focused_index(&self) -> usize {
        self.focus
    }

    /// Returns a reference to the focused terminal.
    #[must_use]
    pub fn focused(&self) -> Option<&Terminal> {
        self.terminals[self.focus].as_ref()
    }

    /// Returns a mutable reference to the focused terminal.
    pub fn focused_mut(&mut self) -> Option<&mut Terminal> {
        self.terminals[self.focus].as_mut()
    }

    /// Returns a reference to a terminal by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Terminal> {
        self.terminals.get(index).and_then(|t| t.as_ref())
    }

    /// Returns a mutable reference to a terminal by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Terminal> {
        self.terminals.get_mut(index).and_then(|t| t.as_mut())
    }

    /// Returns all terminals as a vector of references.
    #[must_use]
    pub fn all_terminals(&self) -> Vec<&Terminal> {
        self.terminals.iter().filter_map(|t| t.as_ref()).collect()
    }

    /// Returns all terminals as a vector of mutable references.
    pub fn all_terminals_mut(&mut self) -> Vec<&mut Terminal> {
        self.terminals.iter_mut().filter_map(|t| t.as_mut()).collect()
    }

    /// Splits the grid, adding a new terminal.
    /// Split progression: 1→2 (vertical), 2→4 (horizontal splits).
    ///
    /// # Errors
    /// Returns error if at max capacity or terminal creation fails.
    pub fn split(&mut self, shell_path: Option<std::path::PathBuf>) -> Result<(), PtyError> {
        let count = self.terminal_count();

        match count {
            1 => {
                // First split: create vertical split (1→2)
                // Position 0 stays, add position 1
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 1);
                let terminal = Terminal::with_shell(term_cols, term_rows, shell_path)?;

                // Resize existing terminal
                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(term_cols, term_rows);
                }

                self.terminals[1] = Some(terminal);
                self.cols = 2;
                self.rows = 1;
                self.focus = 1;
                Ok(())
            }
            2 => {
                // Second split: depends on which pane is focused
                // If focus is 0 or 1 (top row), split horizontally to create bottom row
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 2);

                // Create terminals for bottom row
                let term2 = Terminal::with_shell(term_cols, term_rows, shell_path.clone())?;
                let term3 = Terminal::with_shell(term_cols, term_rows, shell_path)?;

                // Resize existing terminals
                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(term_cols, term_rows);
                }
                if let Some(ref mut t) = self.terminals[1] {
                    let _ = t.resize(term_cols, term_rows);
                }

                self.terminals[2] = Some(term2);
                self.terminals[3] = Some(term3);
                self.cols = 2;
                self.rows = 2;
                // Focus the terminal below the current one
                self.focus = if self.focus == 0 { 2 } else { 3 };
                Ok(())
            }
            3 | 4 => {
                // Already at max capacity
                Err(PtyError::Other("Grid is at maximum capacity (2x2)".to_string()))
            }
            _ => {
                Err(PtyError::Other("Invalid grid state".to_string()))
            }
        }
    }

    /// Closes the focused terminal pane.
    /// Returns true if closed, false if it's the last pane.
    pub fn close_focused(&mut self) -> bool {
        if self.terminal_count() <= 1 {
            return false;
        }

        // Shutdown the focused terminal
        if let Some(ref mut t) = self.terminals[self.focus] {
            let _ = t.shutdown();
        }
        self.terminals[self.focus] = None;

        // Reorganize the grid
        self.reorganize_after_close();
        true
    }

    /// Reorganizes the grid after closing a terminal.
    fn reorganize_after_close(&mut self) {
        let count = self.terminal_count();

        match count {
            0 => {
                // Should not happen, but handle it
                self.cols = 1;
                self.rows = 1;
                self.focus = 0;
            }
            1 => {
                // Find the remaining terminal and move to position 0
                let remaining_idx = self.terminals.iter().position(|t| t.is_some()).unwrap_or(0);
                if remaining_idx != 0 {
                    self.terminals.swap(0, remaining_idx);
                }

                // Resize to full size
                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(self.width, self.height);
                }

                self.cols = 1;
                self.rows = 1;
                self.focus = 0;
            }
            2 => {
                // Compact to positions 0 and 1
                let mut found = Vec::new();
                for (i, t) in self.terminals.iter().enumerate() {
                    if t.is_some() {
                        found.push(i);
                    }
                }

                // Move terminals to 0 and 1 if needed
                if found[0] != 0 {
                    self.terminals.swap(0, found[0]);
                }
                if found.len() > 1 && found[1] != 1 {
                    let src = if found[0] == 1 { found[0] } else { found[1] };
                    if src != 1 {
                        self.terminals.swap(1, src);
                    }
                }

                // Resize to half width
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 1);
                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(term_cols, term_rows);
                }
                if let Some(ref mut t) = self.terminals[1] {
                    let _ = t.resize(term_cols, term_rows);
                }

                self.cols = 2;
                self.rows = 1;
                self.focus = self.focus.min(1);
            }
            3 => {
                // Keep 2x2 layout with one empty slot
                self.cols = 2;
                self.rows = 2;

                // Find a valid focus
                while self.terminals[self.focus].is_none() && self.focus < MAX_GRID_TERMINALS {
                    self.focus = (self.focus + 1) % MAX_GRID_TERMINALS;
                }
            }
            _ => {}
        }
    }

    /// Calculates terminal size based on grid dimensions.
    fn calculate_terminal_size(&self, grid_cols: u8, grid_rows: u8) -> (u16, u16) {
        let term_cols = self.width / u16::from(grid_cols);
        let term_rows = self.height / u16::from(grid_rows);
        (term_cols.max(10), term_rows.max(5))
    }

    /// Moves focus in the given direction.
    pub fn move_focus(&mut self, direction: GridDirection) {
        let (col, row) = self.focus_position();

        let new_pos = match direction {
            GridDirection::Up => {
                if row > 0 { (col, row - 1) } else { (col, row) }
            }
            GridDirection::Down => {
                if row < self.rows - 1 { (col, row + 1) } else { (col, row) }
            }
            GridDirection::Left => {
                if col > 0 { (col - 1, row) } else { (col, row) }
            }
            GridDirection::Right => {
                if col < self.cols - 1 { (col + 1, row) } else { (col, row) }
            }
        };

        let new_idx = self.position_to_index(new_pos.0, new_pos.1);
        if self.terminals[new_idx].is_some() {
            self.focus = new_idx;
        }
    }

    /// Converts focus index to (col, row) position.
    fn focus_position(&self) -> (u8, u8) {
        let col = (self.focus % 2) as u8;
        let row = (self.focus / 2) as u8;
        (col.min(self.cols - 1), row.min(self.rows - 1))
    }

    /// Converts (col, row) to index.
    fn position_to_index(&self, col: u8, row: u8) -> usize {
        (row as usize * 2) + col as usize
    }

    /// Toggles focus between panes (for backward compatibility).
    pub fn toggle_focus(&mut self) {
        let count = self.terminal_count();
        if count <= 1 {
            return;
        }

        // Find next valid terminal
        let mut next = (self.focus + 1) % MAX_GRID_TERMINALS;
        let mut iterations = 0;
        while self.terminals[next].is_none() && iterations < MAX_GRID_TERMINALS {
            next = (next + 1) % MAX_GRID_TERMINALS;
            iterations += 1;
        }

        if self.terminals[next].is_some() {
            self.focus = next;
        }
    }

    /// Resizes all terminals in the grid.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.width = cols;
        self.height = rows;

        let (term_cols, term_rows) = self.calculate_terminal_size(self.cols, self.rows);

        for terminal in self.terminals.iter_mut().flatten() {
            terminal.resize(term_cols, term_rows)?;
        }

        Ok(())
    }

    /// Processes all terminals in the grid.
    ///
    /// # Errors
    /// Returns error if processing fails.
    pub fn process_all(&mut self) -> Result<(), PtyError> {
        for terminal in self.terminals.iter_mut().flatten() {
            terminal.process()?;
        }
        Ok(())
    }

    /// Shuts down all terminals in the grid.
    pub fn shutdown(&mut self) {
        for terminal in self.terminals.iter_mut().flatten() {
            let _ = terminal.shutdown();
        }
    }
}

/// Terminal multiplexer managing multiple terminal tabs.
pub struct TerminalMultiplexer {
    /// List of terminal tabs.
    tabs: Vec<TerminalTab>,
    /// Currently active tab index.
    active_tab: usize,
    /// Terminal dimensions.
    cols: u16,
    /// Terminal rows.
    rows: u16,
}

/// A terminal tab containing a grid of terminals (up to 2x2).
pub struct TerminalTab {
    /// Terminal grid (replaces old split system).
    pub grid: TerminalGrid,
    /// Tab name/title.
    pub name: String,
    /// Tab index.
    pub index: usize,
    // Legacy fields for backward compatibility in rendering
    /// Primary terminal instance (reference to grid[0]).
    /// Note: This is kept for backward compatibility with rendering code.
    pub terminal: DummyTerminalRef,
    /// Secondary terminal (for backward compat - always None now).
    pub split_terminal: Option<DummyTerminalRef>,
    /// Split direction (computed from grid state).
    pub split: SplitDirection,
    /// Which split pane is focused (computed from grid state).
    pub split_focus: SplitFocus,
}

/// Dummy type for backward compatibility - actual terminals are in grid.
#[derive(Debug, Clone, Copy)]
pub struct DummyTerminalRef;

impl TerminalTab {
    /// Creates a new terminal tab with a grid.
    fn new(grid: TerminalGrid, name: String, index: usize) -> Self {
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
            (2, 2) => SplitDirection::Vertical, // 2x2 shows as vertical for outer split
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
    fn update_split_state(&mut self) {
        let (split, split_focus) = Self::compute_split_state(&self.grid);
        self.split = split;
        self.split_focus = split_focus;
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

impl TerminalMultiplexer {
    /// Creates a new terminal multiplexer with one initial tab.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        Self::with_shell(cols, rows, None)
    }

    /// Creates a new terminal multiplexer with a specific shell.
    ///
    /// # Arguments
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    /// * `shell_path` - Path to the shell executable, or None for system default
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn with_shell(
        cols: u16,
        rows: u16,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<Self, PtyError> {
        let grid = TerminalGrid::new(cols, rows, shell_path)?;
        let tab = TerminalTab::new(grid, "Terminal 1".to_string(), 0);

        Ok(Self {
            tabs: vec![tab],
            active_tab: 0,
            cols,
            rows,
        })
    }

    /// Returns the number of tabs.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns the active tab index.
    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// Returns a reference to the active terminal (focused one in grid).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&Terminal> {
        self.tabs.get(self.active_tab).and_then(|t| t.focused_terminal())
    }

    /// Returns a mutable reference to the active terminal.
    pub fn active_terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.tabs
            .get_mut(self.active_tab)
            .and_then(|t| t.focused_terminal_mut())
    }

    /// Returns a reference to the active tab.
    #[must_use]
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab)
    }

    /// Returns a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active_tab)
    }

    /// Returns information about all tabs.
    #[must_use]
    pub fn tab_info(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .map(|t| TabInfo {
                index: t.index,
                name: t.name.clone(),
                is_active: t.index == self.active_tab,
                split: t.split,
            })
            .collect()
    }

    /// Adds a new terminal tab.
    ///
    /// # Errors
    /// Returns error if max tabs reached or terminal creation fails.
    pub fn add_tab(&mut self) -> Result<usize, PtyError> {
        self.add_tab_with_shell(None)
    }

    /// Adds a new terminal tab with a specific shell.
    ///
    /// # Errors
    /// Returns error if maximum tabs reached or terminal creation fails.
    pub fn add_tab_with_shell(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<usize, PtyError> {
        if self.tabs.len() >= MAX_TABS {
            return Err(PtyError::MaxTabsReached);
        }

        let grid = TerminalGrid::new(self.cols, self.rows, shell_path)?;
        let index = self.tabs.len();
        let tab = TerminalTab::new(grid, format!("Terminal {}", index + 1), index);

        self.tabs.push(tab);
        self.active_tab = index;
        Ok(index)
    }

    /// Closes the current tab.
    ///
    /// Returns false if this is the last tab (cannot close).
    pub fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }

        let closing_index = self.active_tab;

        // Shutdown the terminals in the grid
        if let Some(tab) = self.tabs.get_mut(closing_index) {
            tab.grid.shutdown();
        }

        self.tabs.remove(closing_index);

        // Update indices
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            tab.index = i;
            tab.name = format!("Terminal {}", i + 1);
        }

        // Adjust active tab
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }

        true
    }

    /// Switches to the next tab.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// Switches to the previous tab.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab - 1
            };
        }
    }

    /// Switches to a specific tab by index.
    pub fn switch_to(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    /// Splits the current terminal grid (adds a new terminal pane).
    /// Uses the grid's split progression: 1→2 (vertical), 2→4 (horizontal).
    ///
    /// # Errors
    /// Returns error if terminal creation fails or at max capacity.
    pub fn split(&mut self) -> Result<(), PtyError> {
        self.split_with_shell(None)
    }

    /// Splits the current terminal grid with a specific shell.
    ///
    /// # Errors
    /// Returns error if terminal creation fails or at max capacity.
    pub fn split_with_shell(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<(), PtyError> {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.grid.split(shell_path)?;
            tab.update_split_state();
        }
        Ok(())
    }

    /// Creates a horizontal split in the current tab (legacy compatibility).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_horizontal(&mut self) -> Result<(), PtyError> {
        self.split_horizontal_with_shell(None)
    }

    /// Creates a horizontal split in the current tab with a specific shell (legacy).
    /// Now uses the unified grid split system.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_horizontal_with_shell(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<(), PtyError> {
        self.split_with_shell(shell_path)
    }

    /// Creates a vertical split in the current tab (legacy compatibility).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_vertical(&mut self) -> Result<(), PtyError> {
        self.split_vertical_with_shell(None)
    }

    /// Creates a vertical split in the current tab with a specific shell (legacy).
    /// Now uses the unified grid split system.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_vertical_with_shell(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<(), PtyError> {
        self.split_with_shell(shell_path)
    }

    /// Closes the focused terminal pane in the current tab.
    /// If only one pane remains, does nothing.
    pub fn close_split(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.grid.close_focused();
            tab.update_split_state();
        }
    }

    /// Toggles focus between split panes (cycles through grid).
    pub fn toggle_split_focus(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.grid.toggle_focus();
            tab.update_split_state();
        }
    }

    /// Moves focus in the given direction within the grid.
    pub fn move_grid_focus(&mut self, direction: GridDirection) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.grid.move_focus(direction);
            tab.update_split_state();
        }
    }

    /// Returns the current split direction.
    #[must_use]
    pub fn current_split(&self) -> SplitDirection {
        self.tabs
            .get(self.active_tab)
            .map(|t| t.split)
            .unwrap_or(SplitDirection::None)
    }

    /// Returns the current split focus.
    #[must_use]
    pub fn current_split_focus(&self) -> SplitFocus {
        self.tabs
            .get(self.active_tab)
            .map(|t| t.split_focus)
            .unwrap_or(SplitFocus::First)
    }

    /// Returns the grid dimensions (cols, rows) for the active tab.
    #[must_use]
    pub fn current_grid_dimensions(&self) -> Option<(u8, u8)> {
        self.tabs.get(self.active_tab).map(|t| t.grid.dimensions())
    }

    /// Returns the focused terminal index within the grid.
    #[must_use]
    pub fn current_grid_focus(&self) -> Option<usize> {
        self.tabs.get(self.active_tab).map(|t| t.grid.focused_index())
    }

    /// Returns a reference to a terminal by grid index in the active tab.
    #[must_use]
    pub fn get_terminal(&self, index: usize) -> Option<&Terminal> {
        self.tabs.get(self.active_tab).and_then(|t| t.grid.get(index))
    }

    /// Returns a mutable reference to a terminal by grid index.
    pub fn get_terminal_mut(&mut self, index: usize) -> Option<&mut Terminal> {
        self.tabs.get_mut(self.active_tab).and_then(|t| t.grid.get_mut(index))
    }

    /// Resizes all terminals.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.cols = cols;
        self.rows = rows;

        for tab in &mut self.tabs {
            tab.grid.resize(cols, rows)?;
        }

        Ok(())
    }

    /// Processes all terminals (reads PTY output).
    ///
    /// # Errors
    /// Returns error if processing fails.
    pub fn process_all(&mut self) -> Result<(), PtyError> {
        for tab in &mut self.tabs {
            tab.grid.process_all()?;
        }
        Ok(())
    }

    /// Shuts down all terminals.
    pub fn shutdown(&mut self) {
        for tab in &mut self.tabs {
            tab.grid.shutdown();
        }
    }
}

/// Information about a tab.
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Tab index.
    pub index: usize,
    /// Tab name.
    pub name: String,
    /// Whether this is the active tab.
    pub is_active: bool,
    /// Split direction.
    pub split: SplitDirection,
}

// Add MaxTabsReached error variant to PtyError
impl From<&str> for PtyError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}
