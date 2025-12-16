//! Terminal emulator module.
//!
//! Provides PTY-based terminal emulation with ANSI escape sequence parsing.

pub mod action;
pub mod background;
pub mod cell;
pub mod grid;
pub mod multiplexer;
pub mod parser;
pub mod pty;
pub mod selection;
pub mod style;

pub use background::{BackgroundManager, ProcessInfo, ProcessStatus};
pub use multiplexer::{
    GridDirection, SplitDirection, SplitFocus, TabInfo, TerminalGrid, TerminalMultiplexer,
    TerminalTab,
};
pub use selection::{Selection, SelectionMode};

use std::path::PathBuf;

use self::grid::Grid;
use self::parser::AnsiParser;
use self::pty::{Pty, PtyConfig, PtyError};
use self::style::Style;

// Re-export types for public use
pub use self::action::ParsedAction;
pub use self::cell::CursorShape;

/// Maximum processing iterations per update.
const MAX_PROCESS_ITERATIONS: usize = 10_000;

/// Terminal emulator combining PTY and grid.
pub struct Terminal {
    /// The PTY instance.
    pty: Pty,
    /// The terminal grid.
    grid: Grid,
    /// ANSI parser.
    parser: AnsiParser,
    /// Window title.
    title: String,
    /// Pending bell.
    bell: bool,
    /// Scroll view offset (0 = at cursor, positive = viewing scrollback).
    scroll_offset: usize,
    /// Current input line buffer for command interception.
    input_buffer: String,
    /// Current working directory (tracked via OSC 7 or process).
    cwd: Option<PathBuf>,
    /// Initial working directory (set at creation).
    initial_cwd: PathBuf,
}

impl Terminal {
    /// Creates a new terminal with default configuration.
    ///
    /// # Errors
    /// Returns error if PTY creation fails.
    pub fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        let config = PtyConfig::default().size(cols, rows);
        Self::with_config(config)
    }

    /// Creates a new terminal with a specific shell.
    ///
    /// # Arguments
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    /// * `shell_path` - Path to the shell executable, or None for system default
    ///
    /// # Errors
    /// Returns error if PTY creation fails.
    pub fn with_shell(cols: u16, rows: u16, shell_path: Option<PathBuf>) -> Result<Self, PtyError> {
        let mut config = PtyConfig::default().size(cols, rows);
        if let Some(ref path) = shell_path {
            let path_str = path.to_string_lossy().to_string();
            config.shell = Some(path_str.clone());

            // Add appropriate arguments for different shells on Windows
            #[cfg(windows)]
            {
                let path_lower = path_str.to_lowercase();
                if path_lower.contains("bash") {
                    // Git Bash/MSYS2 bash needs --login to initialize properly
                    config.args = vec!["--login".to_string(), "-i".to_string()];
                } else if path_lower.contains("powershell") || path_lower.contains("pwsh") {
                    // PowerShell can use -NoLogo for cleaner startup
                    config.args = vec!["-NoLogo".to_string()];
                }
            }
        }
        Self::with_config(config)
    }

    /// Creates a new terminal with custom configuration.
    ///
    /// # Errors
    /// Returns error if PTY creation fails.
    pub fn with_config(config: PtyConfig) -> Result<Self, PtyError> {
        assert!(config.cols > 0, "Columns must be positive");
        assert!(config.rows > 0, "Rows must be positive");

        // Get initial CWD - either from config or current directory
        let initial_cwd = config
            .working_dir
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let pty = Pty::new(config.clone())?;
        let grid = Grid::new(config.cols, config.rows);
        let parser = AnsiParser::new();

        Ok(Self {
            pty,
            grid,
            parser,
            title: String::new(),
            bell: false,
            scroll_offset: 0,
            input_buffer: String::new(),
            cwd: None,
            initial_cwd,
        })
    }

    /// Returns the terminal grid.
    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Returns the window title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns true if a bell was triggered since last check.
    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.bell)
    }

    /// Returns true if the terminal is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.pty.is_running()
    }

    /// Returns the current working directory of the terminal.
    ///
    /// This returns the CWD in the following priority:
    /// 1. CWD tracked via OSC 7 escape sequences
    /// 2. CWD read from the PTY process (on supported platforms)
    /// 3. The initial working directory
    #[must_use]
    pub fn current_working_dir(&self) -> PathBuf {
        // First, check if we have a CWD from OSC 7
        if let Some(ref cwd) = self.cwd {
            return cwd.clone();
        }

        // Try to get CWD from the PTY process
        if let Some(cwd) = self.pty.current_working_dir() {
            return cwd;
        }

        // Fall back to initial CWD
        self.initial_cwd.clone()
    }

    /// Writes input to the terminal.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn write(&mut self, data: &[u8]) -> Result<(), PtyError> {
        self.pty.write(data)
    }

    /// Writes a character to the terminal.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn write_char(&mut self, c: char) -> Result<(), PtyError> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.pty.write(s.as_bytes())
    }

    /// Resizes the terminal.
    ///
    /// # Errors
    /// Returns error if resize fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.pty.resize(cols, rows)?;
        self.grid.resize(cols, rows);
        Ok(())
    }

    /// Processes pending PTY output.
    ///
    /// # Errors
    /// Returns error if read fails.
    pub fn process(&mut self) -> Result<(), PtyError> {
        let data = self.pty.read()?;

        if data.is_empty() {
            return Ok(());
        }

        let actions = self.parser.parse(&data);

        for (iterations, action) in actions.into_iter().enumerate() {
            if iterations >= MAX_PROCESS_ITERATIONS {
                break;
            }
            self.apply_action(action);
        }

        Ok(())
    }

    /// Applies a parsed action to the grid.
    fn apply_action(&mut self, action: ParsedAction) {
        match action {
            ParsedAction::Print(text) => {
                for c in text.chars() {
                    self.grid.write_char(c);
                }
            }
            ParsedAction::CursorUp(n) => {
                self.grid.move_cursor_up(n);
            }
            ParsedAction::CursorDown(n) => {
                self.grid.move_cursor_down(n);
            }
            ParsedAction::CursorForward(n) => {
                self.grid.move_cursor_right(n);
            }
            ParsedAction::CursorBack(n) => {
                self.grid.move_cursor_left(n);
            }
            ParsedAction::CursorPosition(row, col) => {
                // Terminal positions are 1-indexed
                self.grid
                    .set_cursor_pos(col.saturating_sub(1), row.saturating_sub(1));
            }
            ParsedAction::SetAttr(attrs) => {
                let mut style = Style::new();
                for attr in attrs {
                    style = style.add_attr(attr);
                }
                self.grid.set_style(style);
            }
            ParsedAction::SetFg(color) => {
                let current = Style::new().fg(color);
                self.grid.set_style(current);
            }
            ParsedAction::SetBg(color) => {
                let current = Style::new().bg(color);
                self.grid.set_style(current);
            }
            ParsedAction::EraseDisplay(mode) => match mode {
                0 => self.grid.clear_to_eos(),
                1 => self.grid.clear_to_bos(),
                2 | 3 => self.grid.clear(),
                _ => {}
            },
            ParsedAction::EraseLine(mode) => match mode {
                0 => self.grid.clear_to_eol(),
                1 => self.grid.clear_to_bol(),
                2 => self.grid.clear_line(),
                _ => {}
            },
            ParsedAction::ScrollUp(n) => {
                self.grid.scroll_up(n);
            }
            ParsedAction::ScrollDown(n) => {
                self.grid.scroll_down(n);
            }
            ParsedAction::SaveCursor => {
                self.grid.save_cursor();
            }
            ParsedAction::RestoreCursor => {
                self.grid.restore_cursor();
            }
            ParsedAction::HideCursor => {
                self.grid.set_cursor_visible(false);
            }
            ParsedAction::ShowCursor => {
                self.grid.set_cursor_visible(true);
            }
            ParsedAction::EnterAlternateScreen => {
                self.grid.enter_alternate_screen();
            }
            ParsedAction::ExitAlternateScreen => {
                self.grid.exit_alternate_screen();
            }
            ParsedAction::Bell => {
                self.bell = true;
            }
            ParsedAction::Backspace => {
                self.grid.backspace();
            }
            ParsedAction::Tab => {
                self.grid.tab();
            }
            ParsedAction::LineFeed => {
                self.grid.newline();
            }
            ParsedAction::CarriageReturn => {
                self.grid.carriage_return();
            }
            ParsedAction::SetTitle(title) => {
                self.title = title;
            }
            ParsedAction::SetCursorShape(shape) => {
                let cursor_shape = match shape {
                    0..=2 => CursorShape::Block,
                    3 | 4 => CursorShape::Underline,
                    5 | 6 => CursorShape::Bar,
                    _ => CursorShape::Block,
                };
                self.grid.set_cursor_shape(cursor_shape);
            }
            ParsedAction::InsertLines(n) => {
                self.grid.insert_lines(n);
            }
            ParsedAction::DeleteLines(n) => {
                self.grid.delete_lines(n);
            }
            ParsedAction::InsertChars(n) => {
                self.grid.insert_chars(n);
            }
            ParsedAction::DeleteChars(n) => {
                self.grid.delete_chars(n);
            }
            ParsedAction::DeviceStatusReport => {
                // Send cursor position report
                let (col, row) = self.grid.cursor_pos();
                let response = format!("\x1b[{};{}R", row + 1, col + 1);
                let _ = self.pty.write(response.as_bytes());
            }
            ParsedAction::Hyperlink { .. } => {
                // Hyperlinks not yet supported in rendering
            }
            ParsedAction::SetCwd(path) => {
                // Update the tracked current working directory
                self.cwd = Some(PathBuf::from(path));
            }
            ParsedAction::Unknown(_) => {
                // Ignore unknown sequences
            }
        }
    }

    /// Shuts down the terminal.
    ///
    /// # Errors
    /// Returns error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<(), PtyError> {
        self.pty.shutdown()
    }

    /// Returns the current scroll view offset.
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the view up (into scrollback history).
    pub fn scroll_view_up(&mut self, lines: usize) {
        let max_offset = self.grid.scrollback_len();
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    /// Scrolls the view down (toward current output).
    pub fn scroll_view_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scrolls view to show the cursor (resets scroll offset).
    pub fn scroll_to_cursor(&mut self) {
        self.scroll_offset = 0;
    }

    /// Sends interrupt signal (Ctrl+C) and resets view to cursor.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn send_interrupt(&mut self) -> Result<(), PtyError> {
        self.scroll_to_cursor();
        self.input_buffer.clear();
        self.pty.write(&[0x03]) // ETX (Ctrl+C)
    }

    /// Processes a character input, checking for command interception.
    /// Returns Some(command) if a special command was entered.
    ///
    /// # Errors
    /// Returns error if write fails.
    pub fn process_input(&mut self, c: char) -> Result<Option<String>, PtyError> {
        // Reset scroll when typing
        self.scroll_to_cursor();

        match c {
            '\r' | '\n' => {
                // Check if input buffer contains an intercepted command
                let command = self.check_command_intercept();
                self.input_buffer.clear();

                if command.is_some() {
                    // Clear the shell's input line since we're intercepting the command
                    // Move cursor to start of line and clear from there to end of screen
                    // This aggressively clears any visual artifacts
                    self.grid.carriage_return();
                    self.grid.clear_to_eos();

                    // Send Ctrl+C to cancel any pending input
                    // Ctrl+C is more universally supported across shells than Ctrl+U
                    self.pty.write(&[0x03])?; // Ctrl+C - cancel/interrupt

                    // Small delay to let shell process the interrupt
                    std::thread::sleep(std::time::Duration::from_millis(30));

                    // Read and discard any shell output from the interrupt
                    // This prevents escape sequences from corrupting our grid
                    let _ = self.pty.read();

                    return Ok(command);
                }

                // Normal enter - send to PTY
                self.pty.write(&[b'\r'])?;
            }
            '\x7f' | '\x08' => {
                // Backspace
                self.input_buffer.pop();
                self.pty.write(&[0x7f])?;
            }
            _ => {
                self.input_buffer.push(c);
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                self.pty.write(s.as_bytes())?;
            }
        }

        Ok(None)
    }

    /// Checks if the input buffer matches an interceptable command.
    fn check_command_intercept(&self) -> Option<String> {
        let trimmed = self.input_buffer.trim();

        // Check for "open" command
        if trimmed == "open" {
            return Some("open".to_string());
        }

        // Check for "open <filename>" command
        if let Some(rest) = trimmed.strip_prefix("open ") {
            let filename = rest.trim();
            if !filename.is_empty() {
                return Some(format!("open {}", filename));
            }
        }

        // Check for "update" command
        if trimmed == "update" {
            return Some("update".to_string());
        }

        None
    }

    /// Clears the input buffer (e.g., after Ctrl+C).
    pub fn clear_input_buffer(&mut self) {
        self.input_buffer.clear();
    }

    /// Clears the visible grid area (not scrollback).
    /// Use this to prevent visual artifacts when switching modes.
    pub fn clear_visible(&mut self) {
        self.grid.clear();
    }

    // ========== Selection Methods ==========

    /// Starts a new selection at the given grid position.
    pub fn start_selection(&mut self, col: u16, row: u16) {
        self.grid.start_selection(col, row);
    }

    /// Updates the selection end position.
    pub fn update_selection(&mut self, col: u16, row: u16) {
        self.grid.update_selection(col, row);
    }

    /// Finalizes the selection (e.g., mouse released).
    pub fn finalize_selection(&mut self) {
        self.grid.finalize_selection();
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.grid.clear_selection();
    }

    /// Returns whether there is an active selection.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.grid.has_selection()
    }

    /// Returns the selected text from the terminal.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        self.grid.selected_text()
    }

    /// Extends selection left by one character (keyboard selection).
    pub fn select_left(&mut self) {
        self.grid.extend_selection_left();
    }

    /// Extends selection right by one character (keyboard selection).
    pub fn select_right(&mut self) {
        self.grid.extend_selection_right();
    }

    /// Extends selection up by one row (keyboard selection).
    pub fn select_up(&mut self) {
        self.grid.extend_selection_up();
    }

    /// Extends selection down by one row (keyboard selection).
    pub fn select_down(&mut self) {
        self.grid.extend_selection_down();
    }
}
