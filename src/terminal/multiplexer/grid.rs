//! Terminal grid for managing multiple terminals in a tab.

use super::types::{GridDirection, MAX_GRID_TERMINALS};
use crate::terminal::{Terminal, pty::PtyError};

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

    /// Creates a new grid with an SSH terminal.
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new_ssh(
        cols: u16,
        rows: u16,
        user: &str,
        host: &str,
        port: u16,
    ) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let terminal = Terminal::with_ssh(cols, rows, user, host, port)?;

        Ok(Self {
            terminals: [Some(terminal), None, None, None],
            focus: 0,
            cols: 1,
            rows: 1,
            width: cols,
            height: rows,
        })
    }

    /// Creates a new grid with a Docker exec terminal (local container).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    pub fn new_docker_exec(
        cols: u16,
        rows: u16,
        container_id: &str,
        container_name: &str,
        shell: &str,
    ) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let terminal = Terminal::with_docker_exec(cols, rows, container_id, container_name, shell)?;

        Ok(Self {
            terminals: [Some(terminal), None, None, None],
            focus: 0,
            cols: 1,
            rows: 1,
            width: cols,
            height: rows,
        })
    }

    /// Creates a new grid with a Docker exec terminal via SSH (remote container).
    ///
    /// # Errors
    /// Returns error if terminal creation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn new_docker_exec_ssh(
        cols: u16,
        rows: u16,
        container_id: &str,
        container_name: &str,
        shell: &str,
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        host_id: u32,
    ) -> Result<Self, PtyError> {
        assert!(cols > 0, "Columns must be positive");
        assert!(rows > 0, "Rows must be positive");

        let terminal = Terminal::with_docker_exec_ssh(
            cols,
            rows,
            container_id,
            container_name,
            shell,
            ssh_host,
            ssh_port,
            ssh_user,
            host_id,
        )?;

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
        self.terminals
            .iter_mut()
            .filter_map(|t| t.as_mut())
            .collect()
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
                // Second split: create 2x2 grid
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 2);

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
                self.focus = if self.focus == 0 { 2 } else { 3 };
                Ok(())
            }
            3 | 4 => Err(PtyError::Other(
                "Grid is at maximum capacity (2x2)".to_string(),
            )),
            _ => Err(PtyError::Other("Invalid grid state".to_string())),
        }
    }

    /// Splits the grid, inheriting SSH context from the focused terminal.
    ///
    /// # Errors
    /// Returns error if at max capacity or terminal creation fails.
    pub fn split_with_ssh_inheritance(
        &mut self,
        shell_path: Option<std::path::PathBuf>,
    ) -> Result<bool, PtyError> {
        let count = self.terminal_count();
        let ssh_context = self.focused().and_then(|t| t.ssh_context().cloned());

        match count {
            1 => {
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 1);

                let (terminal, is_ssh) = if let Some(ref ctx) = ssh_context {
                    let mut term = Terminal::with_ssh(
                        term_cols,
                        term_rows,
                        &ctx.username,
                        &ctx.hostname,
                        ctx.port,
                    )?;

                    if let Some(ref pwd) = ctx.password {
                        term.set_pending_password(pwd.clone());
                        term.set_ssh_password(pwd.clone());
                    }
                    (term, true)
                } else {
                    (
                        Terminal::with_shell(term_cols, term_rows, shell_path)?,
                        false,
                    )
                };

                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(term_cols, term_rows);
                }

                self.terminals[1] = Some(terminal);
                self.cols = 2;
                self.rows = 1;
                self.focus = 1;
                Ok(is_ssh)
            }
            2 => {
                let (term_cols, term_rows) = self.calculate_terminal_size(2, 2);

                let (term2, term3, is_ssh) = if let Some(ref ctx) = ssh_context {
                    let mut t2 = Terminal::with_ssh(
                        term_cols,
                        term_rows,
                        &ctx.username,
                        &ctx.hostname,
                        ctx.port,
                    )?;
                    let mut t3 = Terminal::with_ssh(
                        term_cols,
                        term_rows,
                        &ctx.username,
                        &ctx.hostname,
                        ctx.port,
                    )?;

                    if let Some(ref pwd) = ctx.password {
                        t2.set_pending_password(pwd.clone());
                        t2.set_ssh_password(pwd.clone());
                        t3.set_pending_password(pwd.clone());
                        t3.set_ssh_password(pwd.clone());
                    }
                    (t2, t3, true)
                } else {
                    let t2 = Terminal::with_shell(term_cols, term_rows, shell_path.clone())?;
                    let t3 = Terminal::with_shell(term_cols, term_rows, shell_path)?;
                    (t2, t3, false)
                };

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
                self.focus = if self.focus == 0 { 2 } else { 3 };
                Ok(is_ssh)
            }
            3 | 4 => Err(PtyError::Other(
                "Grid is at maximum capacity (2x2)".to_string(),
            )),
            _ => Err(PtyError::Other("Invalid grid state".to_string())),
        }
    }

    /// Closes the focused terminal pane.
    /// Returns true if closed, false if it's the last pane.
    pub fn close_focused(&mut self) -> bool {
        if self.terminal_count() <= 1 {
            return false;
        }

        if let Some(ref mut t) = self.terminals[self.focus] {
            let _ = t.shutdown();
        }
        self.terminals[self.focus] = None;

        self.reorganize_after_close();
        true
    }

    /// Reorganizes the grid after closing a terminal.
    fn reorganize_after_close(&mut self) {
        let count = self.terminal_count();

        match count {
            0 => {
                self.cols = 1;
                self.rows = 1;
                self.focus = 0;
            }
            1 => {
                let remaining_idx = self.terminals.iter().position(|t| t.is_some()).unwrap_or(0);
                if remaining_idx != 0 {
                    self.terminals.swap(0, remaining_idx);
                }

                if let Some(ref mut t) = self.terminals[0] {
                    let _ = t.resize(self.width, self.height);
                }

                self.cols = 1;
                self.rows = 1;
                self.focus = 0;
            }
            2 => {
                let mut found = Vec::new();
                for (i, t) in self.terminals.iter().enumerate() {
                    if t.is_some() {
                        found.push(i);
                    }
                }

                if found[0] != 0 {
                    self.terminals.swap(0, found[0]);
                }
                if found.len() > 1 && found[1] != 1 {
                    let src = if found[0] == 1 { found[0] } else { found[1] };
                    if src != 1 {
                        self.terminals.swap(1, src);
                    }
                }

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
                self.cols = 2;
                self.rows = 2;

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
                if row > 0 {
                    (col, row - 1)
                } else {
                    (col, row)
                }
            }
            GridDirection::Down => {
                if row < self.rows - 1 {
                    (col, row + 1)
                } else {
                    (col, row)
                }
            }
            GridDirection::Left => {
                if col > 0 {
                    (col - 1, row)
                } else {
                    (col, row)
                }
            }
            GridDirection::Right => {
                if col < self.cols - 1 {
                    (col + 1, row)
                } else {
                    (col, row)
                }
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

    /// Toggles focus between panes.
    pub fn toggle_focus(&mut self) {
        let count = self.terminal_count();
        if count <= 1 {
            return;
        }

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
