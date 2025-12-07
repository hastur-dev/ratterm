//! Terminal multiplexer for managing multiple terminal tabs with split support.
//!
//! Provides tab management and split views for terminal instances.

use super::{pty::PtyError, Terminal};

/// Maximum number of terminal tabs.
const MAX_TABS: usize = 10;

/// Split direction for terminal panes.
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

/// Which pane is focused in a split.
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

/// A terminal tab that can contain one or two terminals (split).
pub struct TerminalTab {
    /// Primary terminal instance.
    pub terminal: Terminal,
    /// Secondary terminal (if split).
    pub split_terminal: Option<Terminal>,
    /// Split direction.
    pub split: SplitDirection,
    /// Which split pane is focused.
    pub split_focus: SplitFocus,
    /// Tab name/title.
    pub name: String,
    /// Tab index.
    pub index: usize,
}

impl TerminalTab {
    /// Returns the focused terminal.
    pub fn focused_terminal(&self) -> &Terminal {
        match (self.split, self.split_focus) {
            (SplitDirection::None, _) => &self.terminal,
            (_, SplitFocus::First) => &self.terminal,
            (_, SplitFocus::Second) => {
                self.split_terminal.as_ref().unwrap_or(&self.terminal)
            }
        }
    }

    /// Returns the focused terminal mutably.
    pub fn focused_terminal_mut(&mut self) -> &mut Terminal {
        match (self.split, self.split_focus) {
            (SplitDirection::None, _) => &mut self.terminal,
            (_, SplitFocus::First) => &mut self.terminal,
            (_, SplitFocus::Second) => {
                if self.split_terminal.is_some() {
                    self.split_terminal.as_mut().expect("split terminal exists")
                } else {
                    &mut self.terminal
                }
            }
        }
    }

    /// Returns both terminals if split, or just the primary.
    pub fn terminals(&self) -> Vec<&Terminal> {
        let mut result = vec![&self.terminal];
        if let Some(ref split) = self.split_terminal {
            result.push(split);
        }
        result
    }

    /// Returns both terminals mutably if split.
    pub fn terminals_mut(&mut self) -> Vec<&mut Terminal> {
        let mut result = vec![&mut self.terminal];
        if let Some(ref mut split) = self.split_terminal {
            result.push(split);
        }
        result
    }
}

impl TerminalMultiplexer {
    /// Creates a new terminal multiplexer with one initial tab.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        let terminal = Terminal::new(cols, rows)?;
        let tab = TerminalTab {
            terminal,
            split_terminal: None,
            split: SplitDirection::None,
            split_focus: SplitFocus::First,
            name: "Terminal 1".to_string(),
            index: 0,
        };

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

    /// Returns a reference to the active terminal (focused one in split).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&Terminal> {
        self.tabs.get(self.active_tab).map(|t| t.focused_terminal())
    }

    /// Returns a mutable reference to the active terminal.
    pub fn active_terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.tabs
            .get_mut(self.active_tab)
            .map(|t| t.focused_terminal_mut())
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
        if self.tabs.len() >= MAX_TABS {
            return Err(PtyError::MaxTabsReached);
        }

        let terminal = Terminal::new(self.cols, self.rows)?;
        let index = self.tabs.len();
        let tab = TerminalTab {
            terminal,
            split_terminal: None,
            split: SplitDirection::None,
            split_focus: SplitFocus::First,
            name: format!("Terminal {}", index + 1),
            index,
        };

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

        // Shutdown the terminals
        if let Some(tab) = self.tabs.get_mut(closing_index) {
            let _ = tab.terminal.shutdown();
            if let Some(ref mut split) = tab.split_terminal {
                let _ = split.shutdown();
            }
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

    /// Creates a horizontal split in the current tab.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_horizontal(&mut self) -> Result<(), PtyError> {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.split_terminal.is_some() {
                // Already split - just change direction
                tab.split = SplitDirection::Horizontal;
                return Ok(());
            }

            // Calculate split dimensions
            let split_rows = self.rows / 2;
            let terminal = Terminal::new(self.cols, split_rows)?;

            // Resize existing terminal
            let _ = tab.terminal.resize(self.cols, split_rows);

            tab.split_terminal = Some(terminal);
            tab.split = SplitDirection::Horizontal;
            tab.split_focus = SplitFocus::Second;
        }
        Ok(())
    }

    /// Creates a vertical split in the current tab.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn split_vertical(&mut self) -> Result<(), PtyError> {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.split_terminal.is_some() {
                // Already split - just change direction
                tab.split = SplitDirection::Vertical;
                return Ok(());
            }

            // Calculate split dimensions
            let split_cols = self.cols / 2;
            let terminal = Terminal::new(split_cols, self.rows)?;

            // Resize existing terminal
            let _ = tab.terminal.resize(split_cols, self.rows);

            tab.split_terminal = Some(terminal);
            tab.split = SplitDirection::Vertical;
            tab.split_focus = SplitFocus::Second;
        }
        Ok(())
    }

    /// Closes the current split (removes secondary terminal).
    pub fn close_split(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if let Some(ref mut split) = tab.split_terminal {
                let _ = split.shutdown();
            }
            tab.split_terminal = None;
            tab.split = SplitDirection::None;
            tab.split_focus = SplitFocus::First;

            // Resize primary terminal to full size
            let _ = tab.terminal.resize(self.cols, self.rows);
        }
    }

    /// Toggles focus between split panes.
    pub fn toggle_split_focus(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.split != SplitDirection::None && tab.split_terminal.is_some() {
                tab.split_focus = tab.split_focus.toggle();
            }
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

    /// Resizes all terminals.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.cols = cols;
        self.rows = rows;

        for tab in &mut self.tabs {
            match tab.split {
                SplitDirection::None => {
                    tab.terminal.resize(cols, rows)?;
                }
                SplitDirection::Horizontal => {
                    let split_rows = rows / 2;
                    tab.terminal.resize(cols, split_rows)?;
                    if let Some(ref mut split) = tab.split_terminal {
                        split.resize(cols, split_rows)?;
                    }
                }
                SplitDirection::Vertical => {
                    let split_cols = cols / 2;
                    tab.terminal.resize(split_cols, rows)?;
                    if let Some(ref mut split) = tab.split_terminal {
                        split.resize(split_cols, rows)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Processes all terminals (reads PTY output).
    ///
    /// # Errors
    /// Returns error if processing fails.
    pub fn process_all(&mut self) -> Result<(), PtyError> {
        for tab in &mut self.tabs {
            tab.terminal.process()?;
            if let Some(ref mut split) = tab.split_terminal {
                split.process()?;
            }
        }
        Ok(())
    }

    /// Shuts down all terminals.
    pub fn shutdown(&mut self) {
        for tab in &mut self.tabs {
            let _ = tab.terminal.shutdown();
            if let Some(ref mut split) = tab.split_terminal {
                let _ = split.shutdown();
            }
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
