//! Terminal multiplexer for managing multiple terminal tabs with grid support.
//!
//! Provides tab management and 2x2 grid layouts for terminal instances.

mod grid;
mod tab;
mod types;

pub use grid::TerminalGrid;
pub use tab::TerminalTab;
pub use types::{
    DummyTerminalRef, GridDirection, MAX_GRID_TERMINALS, MAX_TABS, SplitDirection, SplitFocus,
    TabInfo,
};

use crate::terminal::{Terminal, pty::PtyError};

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

    /// Returns a reference to a tab by index.
    #[must_use]
    pub fn get_tab(&self, index: usize) -> Option<&TerminalTab> {
        self.tabs.get(index)
    }

    /// Returns a reference to the active terminal (focused one in grid).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&Terminal> {
        self.tabs
            .get(self.active_tab)
            .and_then(|t| t.focused_terminal())
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

    /// Adds a new terminal tab with an SSH connection.
    ///
    /// # Errors
    /// Returns error if maximum tabs reached or SSH session creation fails.
    pub fn add_ssh_tab(&mut self, user: &str, host: &str, port: u16) -> Result<usize, PtyError> {
        self.add_ssh_tab_with_password(user, host, port, None)
    }

    /// Adds a new SSH terminal tab with optional password for auto-login.
    ///
    /// # Errors
    /// Returns error if maximum tabs reached or SSH session creation fails.
    pub fn add_ssh_tab_with_password(
        &mut self,
        user: &str,
        host: &str,
        port: u16,
        password: Option<&str>,
    ) -> Result<usize, PtyError> {
        if self.tabs.len() >= MAX_TABS {
            return Err(PtyError::MaxTabsReached);
        }

        let mut grid = TerminalGrid::new_ssh(self.cols, self.rows, user, host, port)?;

        if let Some(pwd) = password {
            if let Some(terminal) = grid.focused_mut() {
                terminal.set_pending_password(pwd.to_string());
                terminal.set_ssh_password(pwd.to_string());
            }
        }

        let index = self.tabs.len();

        let tab_name = if port == 22 {
            format!("SSH: {}@{}", user, host)
        } else {
            format!("SSH: {}@{}:{}", user, host, port)
        };

        let tab = TerminalTab::new(grid, tab_name, index);

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

        if let Some(tab) = self.tabs.get_mut(closing_index) {
            tab.grid.shutdown();
        }

        self.tabs.remove(closing_index);

        for (i, tab) in self.tabs.iter_mut().enumerate() {
            tab.index = i;
            tab.name = format!("Terminal {}", i + 1);
        }

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

    /// Sets the name of a tab by index.
    pub fn set_tab_name(&mut self, index: usize, name: String) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.name = name;
        }
    }

    /// Splits the current terminal grid.
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

    /// Creates a horizontal split with a specific shell (legacy).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_horizontal_with_shell(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<(), PtyError> {
        self.split_with_shell(shell_path)
    }

    /// Splits the current terminal grid, inheriting SSH context if applicable.
    ///
    /// # Returns
    /// Returns `Ok(true)` if SSH was inherited, `Ok(false)` for local shell.
    ///
    /// # Errors
    /// Returns error if terminal creation fails or at max capacity.
    pub fn split_with_ssh_inheritance(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<bool, PtyError> {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            let is_ssh = tab.grid.split_with_ssh_inheritance(shell_path)?;
            tab.update_split_state();
            Ok(is_ssh)
        } else {
            Ok(false)
        }
    }

    /// Creates a vertical split in the current tab (legacy compatibility).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_vertical(&mut self) -> Result<(), PtyError> {
        self.split_vertical_with_shell(None)
    }

    /// Creates a vertical split with a specific shell (legacy).
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
        self.tabs
            .get(self.active_tab)
            .map(|t| t.grid.focused_index())
    }

    /// Returns a reference to a terminal by grid index in the active tab.
    #[must_use]
    pub fn get_terminal(&self, index: usize) -> Option<&Terminal> {
        self.tabs
            .get(self.active_tab)
            .and_then(|t| t.grid.get(index))
    }

    /// Returns a mutable reference to a terminal by grid index.
    pub fn get_terminal_mut(&mut self, index: usize) -> Option<&mut Terminal> {
        self.tabs
            .get_mut(self.active_tab)
            .and_then(|t| t.grid.get_mut(index))
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

impl From<&str> for PtyError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}
