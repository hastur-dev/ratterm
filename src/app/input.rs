//! Input handling for the application.
//!
//! Handles key events for different application modes.

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::config::KeybindingMode;
use crate::editor::EditorMode;
use crate::ui::layout::FocusedPane;
use crate::ui::popup::PopupKind;

use super::keymap::key_to_bytes;
use super::{App, AppMode};

impl App {
    /// Handles a key event.
    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.mode {
            AppMode::Normal => self.handle_normal_key(key),
            AppMode::FileBrowser => self.handle_file_browser_key(key),
            AppMode::Popup => self.handle_popup_key(key),
        }
    }

    /// Handles keys in normal mode.
    fn handle_normal_key(&mut self, key: KeyEvent) {
        // Global keybindings
        if self.handle_global_key(key) {
            return;
        }

        // Editor-focused keybindings
        if self.layout.focused() == FocusedPane::Editor && self.handle_editor_global_key(key) {
            return;
        }

        // Route to focused pane
        match self.layout.focused() {
            FocusedPane::Terminal => self.handle_terminal_key(key),
            FocusedPane::Editor => self.handle_editor_key(key),
        }
    }

    /// Handles global keybindings. Returns true if handled.
    fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Ctrl+I: Toggle IDE visibility
            (KeyModifiers::CONTROL, KeyCode::Char('i')) => {
                self.toggle_ide();
                true
            }
            (KeyModifiers::ALT, KeyCode::Left) => {
                self.layout.set_focused(FocusedPane::Terminal);
                true
            }
            (KeyModifiers::ALT, KeyCode::Right) => {
                // Only focus editor if IDE is visible
                if self.layout.ide_visible() {
                    self.layout.set_focused(FocusedPane::Editor);
                }
                true
            }
            // Alt+Up/Down: Switch between split terminal panes
            (KeyModifiers::ALT, KeyCode::Up) | (KeyModifiers::ALT, KeyCode::Down) => {
                if self.layout.focused() == FocusedPane::Terminal {
                    self.toggle_terminal_split_focus();
                }
                true
            }
            (KeyModifiers::ALT, KeyCode::Tab) => {
                self.layout.toggle_focus();
                true
            }
            (KeyModifiers::ALT, KeyCode::Char('[')) => {
                self.layout.move_split_left();
                true
            }
            (KeyModifiers::ALT, KeyCode::Char(']')) => {
                self.layout.move_split_right();
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                self.request_quit();
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                self.show_file_browser();
                true
            }
            // Ctrl+Shift+P or Ctrl+P: Command Palette
            (m, KeyCode::Char('p') | KeyCode::Char('P'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_popup(PopupKind::CommandPalette);
                true
            }
            // Ctrl+P also opens command palette (VSCode style)
            (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                self.show_popup(PopupKind::CommandPalette);
                true
            }
            // Ctrl+Shift+Tab: Mode Switcher
            (m, KeyCode::Tab) | (m, KeyCode::BackTab)
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                if self.is_mode_switcher_active() {
                    // Already open, cycle to next mode
                    self.cycle_mode_next();
                } else {
                    // Open mode switcher and immediately select next mode
                    self.show_mode_switcher();
                    self.cycle_mode_next();
                }
                true
            }
            // Ctrl+Shift+C: Copy (works in both editor and terminal)
            (m, KeyCode::Char('c') | KeyCode::Char('C'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.copy_selection();
                true
            }
            // Ctrl+V: Paste (works in both editor and terminal)
            (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                self.paste_clipboard();
                true
            }
            // Alt+Shift+Right/Left for file switching
            (m, KeyCode::Right) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.next_file();
                true
            }
            (m, KeyCode::Left) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.prev_file();
                true
            }
            (m, KeyCode::Char('l') | KeyCode::Char('L'))
                if m == KeyModifiers::ALT | KeyModifiers::SHIFT =>
            {
                self.next_file();
                true
            }
            (m, KeyCode::Char('h') | KeyCode::Char('H'))
                if m == KeyModifiers::ALT | KeyModifiers::SHIFT =>
            {
                self.prev_file();
                true
            }
            _ => false,
        }
    }

    /// Copies current selection to clipboard.
    fn copy_selection(&mut self) {
        match self.layout.focused() {
            FocusedPane::Terminal => self.copy_terminal_selection(),
            FocusedPane::Editor => self.copy_editor_selection(),
        }
    }

    /// Pastes clipboard content.
    fn paste_clipboard(&mut self) {
        match self.layout.focused() {
            FocusedPane::Terminal => self.paste_to_terminal(),
            FocusedPane::Editor => self.paste_to_editor(),
        }
    }

    /// Copies editor selection (or current line) to clipboard.
    fn copy_editor_selection(&mut self) {
        if let Some(selection) = self.editor.selected_text() {
            self.copy_to_clipboard(&selection);
        } else {
            // Copy current line if no selection
            let line = self.editor.current_line();
            if !line.is_empty() {
                self.copy_to_clipboard(&line);
            }
        }
    }

    /// Pastes clipboard content to editor.
    fn paste_to_editor(&mut self) {
        if let Some(text) = self.paste_from_clipboard() {
            self.editor.insert_str(&text);
            self.set_status("Pasted");
        }
    }

    /// Handles editor-specific global keybindings. Returns true if handled.
    fn handle_editor_global_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            // Ctrl+T: New editor tab (untitled buffer)
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                self.new_editor_tab();
                true
            }
            // Ctrl+W: Close current editor tab
            (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                self.close_editor_tab();
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
                self.show_popup(PopupKind::SearchInFile);
                true
            }
            (m, KeyCode::Char('f') | KeyCode::Char('F'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_popup(PopupKind::SearchInFiles);
                true
            }
            (m, KeyCode::Char('d') | KeyCode::Char('D'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_popup(PopupKind::SearchDirectories);
                true
            }
            (m, KeyCode::Char('e') | KeyCode::Char('E'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_popup(PopupKind::SearchFiles);
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                self.show_popup(PopupKind::CreateFile);
                true
            }
            (m, KeyCode::Char('n') | KeyCode::Char('N'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_popup(PopupKind::CreateFolder);
                true
            }
            _ => false,
        }
    }

    /// Handles keys in file browser mode.
    fn handle_file_browser_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_file_browser();
            }
            (KeyModifiers::NONE, KeyCode::Up)
            | (KeyModifiers::NONE, KeyCode::Char('k'))
            | (KeyModifiers::NONE, KeyCode::Char('w')) => {
                self.file_browser.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::Char('j'))
            | (KeyModifiers::NONE, KeyCode::Char('s')) => {
                self.file_browser.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Left)
            | (KeyModifiers::NONE, KeyCode::Char('h'))
            | (KeyModifiers::NONE, KeyCode::Char('a')) => {
                let _ = self.file_browser.go_up();
            }
            (KeyModifiers::NONE, KeyCode::Right)
            | (KeyModifiers::NONE, KeyCode::Char('l'))
            | (KeyModifiers::NONE, KeyCode::Char('d')) => {
                if let Ok(Some(path)) = self.file_browser.enter_selected() {
                    let _ = self.open_file(path);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Ok(Some(path)) = self.file_browser.enter_selected() {
                    let _ = self.open_file(path);
                }
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.file_browser.page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.file_browser.page_down();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.file_browser.move_to_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.file_browser.move_to_end();
            }
            (KeyModifiers::NONE, KeyCode::Char('/')) => {
                self.show_popup(PopupKind::SearchFiles);
            }
            _ => {}
        }
    }

    /// Handles keys in popup mode.
    fn handle_popup_key(&mut self, key: KeyEvent) {
        // Handle confirmation dialogs specially
        if self.popup.kind().is_confirmation() {
            self.handle_confirmation_key(key);
            return;
        }

        // Handle mode switcher specially
        if self.popup.kind().is_mode_switcher() {
            self.handle_mode_switcher_key(key);
            return;
        }

        // Handle shell selector specially
        if self.popup.kind().is_shell_selector() {
            self.handle_shell_selector_key(key);
            return;
        }

        // Handle shell install prompt specially
        if self.popup.kind().is_shell_install_prompt() {
            self.handle_shell_install_prompt_key(key);
            return;
        }

        // Handle theme selector specially
        if self.popup.kind().is_theme_selector() {
            self.handle_theme_selector_key(key);
            return;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_popup();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.execute_popup_action();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.popup.backspace();
                self.update_popup_results();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.popup.delete();
                self.update_popup_results();
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.popup.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.popup.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.popup.result_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.popup.result_down();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.popup.move_to_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.popup.move_to_end();
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.popup.accept_suggestion();
            }
            (m, KeyCode::Up) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                let _ = self.file_browser.go_up();
                self.update_popup_results();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.popup.insert_char(c);
                self.update_popup_results();
            }
            _ => {}
        }
    }

    /// Handles keys for the mode switcher popup.
    fn handle_mode_switcher_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Escape - Cancel and revert
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.cancel_mode_switch();
            }
            // Enter - Apply selected mode
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.apply_mode_switch();
            }
            // Tab or Down - Cycle to next mode
            (KeyModifiers::NONE, KeyCode::Tab)
            | (KeyModifiers::NONE, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.cycle_mode_next();
            }
            // Shift+Tab or Up - Cycle to previous mode
            (KeyModifiers::SHIFT, KeyCode::Tab)
            | (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::Up)
            | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.cycle_mode_prev();
            }
            // Ctrl+Shift+Tab - Continue cycling (already handled in global keys but just in case)
            (m, KeyCode::Tab) | (m, KeyCode::BackTab)
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.cycle_mode_next();
            }
            _ => {}
        }
    }

    /// Handles keys for the shell selector popup.
    fn handle_shell_selector_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Escape - Cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.cancel_shell_selection();
            }
            // Enter - Apply selected shell
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.apply_shell_selection();
            }
            // Down or j - Next shell
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.cycle_shell_next();
            }
            // Up or k - Previous shell
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.cycle_shell_prev();
            }
            _ => {}
        }
    }

    /// Handles keys for the shell install prompt popup.
    fn handle_shell_install_prompt_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Escape or Enter - Close the prompt
            (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Enter) => {
                self.hide_popup();
            }
            _ => {}
        }
    }

    /// Handles keys for the theme selector popup.
    fn handle_theme_selector_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Escape - Cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.cancel_theme_selection();
            }
            // Enter - Apply selected theme
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.apply_theme_selection();
            }
            // Down or j - Next theme
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.cycle_theme_next();
            }
            // Up or k - Previous theme
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.cycle_theme_prev();
            }
            _ => {}
        }
    }

    /// Cycles to the next theme in the selector.
    fn cycle_theme_next(&mut self) {
        if let Some(ref mut selector) = self.theme_selector {
            selector.next();
        }
    }

    /// Cycles to the previous theme in the selector.
    fn cycle_theme_prev(&mut self) {
        if let Some(ref mut selector) = self.theme_selector {
            selector.prev();
        }
    }

    /// Handles keys for confirmation dialogs.
    fn handle_confirmation_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Y or y - Yes, save and quit
            (KeyModifiers::NONE, KeyCode::Char('y'))
            | (KeyModifiers::SHIFT, KeyCode::Char('Y')) => {
                self.hide_popup();
                self.save_and_quit();
            }
            // N or n - No, quit without saving
            (KeyModifiers::NONE, KeyCode::Char('n'))
            | (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
                self.hide_popup();
                self.force_quit();
            }
            // C, c, or Escape - Cancel, go back
            (KeyModifiers::NONE, KeyCode::Char('c'))
            | (KeyModifiers::SHIFT, KeyCode::Char('C'))
            | (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_popup();
            }
            _ => {}
        }
    }

    /// Handles key events for the terminal pane.
    fn handle_terminal_key(&mut self, key: KeyEvent) {
        // Handle terminal tab management keys first (don't need active terminal)
        match (key.modifiers, key.code) {
            // Ctrl+T: Add new terminal tab
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                self.add_terminal_tab();
                return;
            }
            // Ctrl+W: Close current terminal tab
            (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                self.close_terminal_tab();
                return;
            }
            // Ctrl+Left: Previous terminal tab
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.prev_terminal_tab();
                return;
            }
            // Ctrl+Right: Next terminal tab
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.next_terminal_tab();
                return;
            }
            // Ctrl+S: Horizontal split
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                self.split_terminal_horizontal();
                return;
            }
            // Ctrl+Shift+S: Vertical split
            (m, KeyCode::Char('s') | KeyCode::Char('S'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.split_terminal_vertical();
                return;
            }
            // Ctrl+Shift+W: Close split (different from Ctrl+W which closes tab)
            (m, KeyCode::Char('w') | KeyCode::Char('W'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.close_terminal_split();
                return;
            }
            // Ctrl+Tab: Toggle split focus
            (KeyModifiers::CONTROL, KeyCode::Tab) => {
                self.toggle_terminal_split_focus();
                return;
            }
            // Ctrl+Shift+C: Copy from terminal
            (m, KeyCode::Char('c') | KeyCode::Char('C'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.copy_terminal_selection();
                return;
            }
            // Ctrl+V: Paste to terminal
            (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                self.paste_to_terminal();
                return;
            }
            // Shift+Arrow keys for selection
            (KeyModifiers::SHIFT, KeyCode::Left) => {
                if let Some(ref mut terminals) = self.terminals {
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        terminal.select_left();
                    }
                }
                return;
            }
            (KeyModifiers::SHIFT, KeyCode::Right) => {
                if let Some(ref mut terminals) = self.terminals {
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        terminal.select_right();
                    }
                }
                return;
            }
            (KeyModifiers::SHIFT, KeyCode::Up) => {
                if let Some(ref mut terminals) = self.terminals {
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        terminal.select_up();
                    }
                }
                return;
            }
            (KeyModifiers::SHIFT, KeyCode::Down) => {
                if let Some(ref mut terminals) = self.terminals {
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        terminal.select_down();
                    }
                }
                return;
            }
            _ => {}
        }

        // Process terminal input - need to handle command interception carefully
        // to avoid borrow issues
        let cmd_result = self.process_terminal_input(key);

        // Handle any intercepted command outside of the borrow
        if let Some(cmd) = cmd_result {
            self.handle_terminal_command(&cmd);
        }
    }

    /// Processes terminal input and returns any intercepted command.
    fn process_terminal_input(&mut self, key: KeyEvent) -> Option<String> {
        let Some(ref mut terminals) = self.terminals else {
            return None;
        };
        let Some(terminal) = terminals.active_terminal_mut() else {
            return None;
        };

        match (key.modifiers, key.code) {
            // Ctrl+C: Send interrupt and reset view to cursor
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                let _ = terminal.send_interrupt();
                // Set status after we're done with borrow
                None
            }
            // Shift+PageUp: Scroll view up into scrollback
            (KeyModifiers::SHIFT, KeyCode::PageUp) => {
                terminal.scroll_view_up(10);
                None
            }
            // Shift+PageDown: Scroll view down toward cursor
            (KeyModifiers::SHIFT, KeyCode::PageDown) => {
                terminal.scroll_view_down(10);
                None
            }
            // Character input: use process_input for command interception
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                match terminal.process_input(c) {
                    Ok(Some(cmd)) => Some(cmd),
                    Ok(None) => None,
                    Err(_) => None,
                }
            }
            // Enter: check for command interception
            (KeyModifiers::NONE, KeyCode::Enter) => match terminal.process_input('\r') {
                Ok(Some(cmd)) => Some(cmd),
                Ok(None) => None,
                Err(_) => None,
            },
            // Backspace
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let _ = terminal.process_input('\x7f');
                None
            }
            // Other keys: send directly
            _ => {
                let bytes = key_to_bytes(key);
                if !bytes.is_empty() {
                    // Reset scroll view when sending any key
                    terminal.scroll_to_cursor();
                    let _ = terminal.write(&bytes);
                }
                None
            }
        }
    }

    /// Copies terminal selection to clipboard (or current line if no selection).
    fn copy_terminal_selection(&mut self) {
        // Extract text first to avoid borrow issues
        let text_to_copy: Option<(String, bool)> = {
            if let Some(ref terminals) = self.terminals {
                if let Some(terminal) = terminals.active_terminal() {
                    // Try to get selected text first
                    if let Some(text) = terminal.selected_text() {
                        if !text.is_empty() {
                            Some((text, true)) // true = from selection
                        } else {
                            None
                        }
                    } else {
                        // Fallback: copy current line at cursor
                        let grid = terminal.grid();
                        let (_, row) = grid.cursor_pos();
                        if let Some(line) = grid.row(row as usize) {
                            let text: String = line.cells().iter().map(|c| c.character()).collect();
                            let text = text.trim_end().to_string();
                            if !text.is_empty() {
                                Some((text, false)) // false = from line
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Now copy and update status
        if let Some((text, from_selection)) = text_to_copy {
            self.copy_to_clipboard(&text);
            if from_selection {
                self.set_status("Copied selection");
                // Clear selection after copy
                if let Some(ref mut terminals) = self.terminals {
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        terminal.clear_selection();
                    }
                }
            } else {
                self.set_status("Copied line");
            }
        }
    }

    /// Pastes clipboard content to terminal.
    fn paste_to_terminal(&mut self) {
        if let Some(text) = self.paste_from_clipboard() {
            if let Some(ref mut terminals) = self.terminals {
                if let Some(terminal) = terminals.active_terminal_mut() {
                    let _ = terminal.write(text.as_bytes());
                    self.set_status("Pasted");
                }
            }
        }
    }

    /// Handles intercepted terminal commands.
    fn handle_terminal_command(&mut self, cmd: &str) {
        if cmd == "open" {
            // Open file browser and show IDE
            self.show_ide();
            self.show_file_browser();
        } else if let Some(filename) = cmd.strip_prefix("open ") {
            // Open specific file - use terminal's CWD, not file browser path
            let cwd = self
                .terminals
                .as_ref()
                .and_then(|t| t.active_terminal())
                .map(|t| t.current_working_dir())
                .unwrap_or_else(|| self.file_browser.path().to_path_buf());

            let filename = filename.trim();
            // Handle absolute paths
            let path = if std::path::Path::new(filename).is_absolute() {
                std::path::PathBuf::from(filename)
            } else {
                cwd.join(filename)
            };

            if path.exists() {
                if path.is_file() {
                    // Show IDE when opening a file
                    self.show_ide();
                    let _ = self.open_file(path);
                } else if path.is_dir() {
                    // Show IDE and file browser for directories
                    self.show_ide();
                    let _ = self.file_browser.change_dir(&path);
                    self.show_file_browser();
                }
            } else {
                self.set_status(format!("File not found: {}", path.display()));
            }
        } else if cmd == "update" {
            // Check for updates and auto-update
            self.handle_update_command();
        }
    }

    /// Handles the update command - checks for updates and applies them.
    fn handle_update_command(&mut self) {
        use crate::updater::{UpdateStatus, Updater, VERSION};

        self.set_status("Checking for updates...".to_string());

        let updater = Updater::new();
        match updater.check() {
            UpdateStatus::Available(version) => {
                self.set_status(format!(
                    "Update available: v{} -> v{}. Downloading...",
                    VERSION, version
                ));

                // Save session before updating
                if let Err(e) = self.save_session() {
                    self.set_status(format!("Failed to save session: {}", e));
                    return;
                }

                // Perform update
                match updater.update(&version) {
                    Ok(true) => {
                        self.set_status(format!(
                            "Updated to v{}! Restart ratterm to use new version.",
                            version
                        ));
                    }
                    Ok(false) => {
                        self.set_status(format!(
                            "Already running v{} (latest version)",
                            VERSION
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("Update failed: {}", e));
                    }
                }
            }
            UpdateStatus::UpToDate => {
                self.set_status(format!("Already up to date (v{})", VERSION));
            }
            UpdateStatus::Failed(e) => {
                self.set_status(format!("Update check failed: {}", e));
            }
            UpdateStatus::Disabled => {
                self.set_status("Updates disabled via RATTERM_NO_UPDATE".to_string());
            }
        }
    }

    /// Handles key events for the editor pane.
    fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.keybinding_mode() {
            KeybindingMode::Vim => self.handle_editor_key_vim(key),
            KeybindingMode::Emacs => self.handle_editor_key_emacs(key),
            KeybindingMode::Default => self.handle_editor_key_default(key),
            KeybindingMode::VsCode => self.handle_editor_key_vscode(key),
        }
    }

    /// Handles editor keys in Vim mode (modal editing).
    fn handle_editor_key_vim(&mut self, key: KeyEvent) {
        match self.editor.mode() {
            EditorMode::Normal => self.handle_editor_normal_key(key),
            EditorMode::Insert => self.handle_editor_insert_key(key),
            EditorMode::Visual => self.handle_editor_visual_key(key),
            EditorMode::Command => self.handle_editor_command_key(key),
        }
    }

    /// Handles editor keys in Emacs mode (non-modal).
    fn handle_editor_key_emacs(&mut self, key: KeyEvent) {
        // Emacs mode is always in "insert" mode with Ctrl+key navigation
        match (key.modifiers, key.code) {
            // Navigation
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.editor.move_left();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
                self.editor.move_right();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                self.editor.move_up();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                self.editor.move_down();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.editor.move_to_line_start();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.editor.move_to_line_end();
            }
            // Word navigation
            (KeyModifiers::ALT, KeyCode::Char('f')) => {
                self.editor.move_word_right();
            }
            (KeyModifiers::ALT, KeyCode::Char('b')) => {
                self.editor.move_word_left();
            }
            // Buffer navigation
            (m, KeyCode::Char('<')) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.editor.move_to_buffer_start();
            }
            (m, KeyCode::Char('>')) if m == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                self.editor.move_to_buffer_end();
            }
            // Editing
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.editor.delete();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('/')) => {
                self.editor.undo();
            }
            (m, KeyCode::Char('/')) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                // Kill to end of line
                self.editor.delete_to_line_end();
            }
            // Save
            (KeyModifiers::CONTROL, KeyCode::Char('x')) => {
                // Ctrl+X Ctrl+S pattern - for now just save on Ctrl+X
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                }
            }
            // Standard editing keys
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.editor.backspace();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.editor.delete();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.editor.insert_char('\n');
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.editor.insert_str("    ");
            }
            // Arrow keys still work
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.editor.move_to_line_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.editor.move_to_line_end();
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.editor.page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.editor.page_down();
            }
            // Character input
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in Default mode (non-modal, simple keybindings).
    fn handle_editor_key_default(&mut self, key: KeyEvent) {
        // Default mode is always in "insert" mode with arrow key navigation
        match (key.modifiers, key.code) {
            // Navigation with arrow keys
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.editor.move_to_line_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.editor.move_to_line_end();
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.editor.page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.editor.page_down();
            }
            // Ctrl+arrow for word navigation
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.editor.move_word_left();
            }
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.editor.move_word_right();
            }
            // Ctrl+Home/End for buffer navigation
            (KeyModifiers::CONTROL, KeyCode::Home) => {
                self.editor.move_to_buffer_start();
            }
            (KeyModifiers::CONTROL, KeyCode::End) => {
                self.editor.move_to_buffer_end();
            }
            // Standard shortcuts
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                self.editor.undo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                }
            }
            // Standard editing keys
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.editor.backspace();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.editor.delete();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.editor.insert_char('\n');
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.editor.insert_str("    ");
            }
            // Character input
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in VSCode mode (non-modal, VSCode-like keybindings).
    fn handle_editor_key_vscode(&mut self, key: KeyEvent) {
        // VSCode mode is always in "insert" mode with standard shortcuts
        match (key.modifiers, key.code) {
            // Navigation with arrow keys
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.editor.move_to_line_start();
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.editor.move_to_line_end();
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.editor.page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.editor.page_down();
            }
            // Ctrl+arrow for word navigation
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.editor.move_word_left();
            }
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.editor.move_word_right();
            }
            // Ctrl+Home/End for buffer navigation
            (KeyModifiers::CONTROL, KeyCode::Home) => {
                self.editor.move_to_buffer_start();
            }
            (KeyModifiers::CONTROL, KeyCode::End) => {
                self.editor.move_to_buffer_end();
            }
            // Shift+arrow for selection (VSCode style)
            (KeyModifiers::SHIFT, KeyCode::Left) => {
                self.editor.select_left();
            }
            (KeyModifiers::SHIFT, KeyCode::Right) => {
                self.editor.select_right();
            }
            (KeyModifiers::SHIFT, KeyCode::Up) => {
                self.editor.select_up();
            }
            (KeyModifiers::SHIFT, KeyCode::Down) => {
                self.editor.select_down();
            }
            // Ctrl+Shift+arrow for word selection
            (m, KeyCode::Left) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.select_word_left();
            }
            (m, KeyCode::Right) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                self.editor.select_word_right();
            }
            // Shift+Home/End for line selection
            (KeyModifiers::SHIFT, KeyCode::Home) => {
                self.editor.select_to_line_start();
            }
            (KeyModifiers::SHIFT, KeyCode::End) => {
                self.editor.select_to_line_end();
            }
            // Ctrl+A for select all
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                self.editor.select_all();
            }
            // Ctrl+L for select line (VSCode)
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
                self.editor.select_line();
            }
            // Standard shortcuts
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                self.editor.undo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
                self.editor.redo();
            }
            // Ctrl+Shift+Z also for redo (VSCode alternative)
            (m, KeyCode::Char('z') | KeyCode::Char('Z'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                }
            }
            // Ctrl+D for duplicate line or add selection to next match
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.editor.duplicate_line();
            }
            // Ctrl+Shift+K for delete line (VSCode)
            (m, KeyCode::Char('k') | KeyCode::Char('K'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.editor.delete_line();
            }
            // Alt+Up/Down for move line up/down
            (KeyModifiers::ALT, KeyCode::Up) => {
                self.editor.move_line_up();
            }
            (KeyModifiers::ALT, KeyCode::Down) => {
                self.editor.move_line_down();
            }
            // Ctrl+/ for toggle comment (common VSCode binding)
            (KeyModifiers::CONTROL, KeyCode::Char('/')) => {
                self.editor.toggle_comment();
            }
            // Ctrl+] for indent, Ctrl+[ for outdent
            (KeyModifiers::CONTROL, KeyCode::Char(']')) => {
                self.editor.indent();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('[')) => {
                self.editor.outdent();
            }
            // Standard editing keys
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.editor.backspace();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.editor.delete();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.editor.insert_char('\n');
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.editor.insert_str("    ");
            }
            (KeyModifiers::SHIFT, KeyCode::Tab) => {
                self.editor.outdent();
            }
            // Character input
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles editor keys in normal mode.
    fn handle_editor_normal_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                self.editor.set_mode(EditorMode::Insert);
            }
            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                self.editor.move_right();
                self.editor.set_mode(EditorMode::Insert);
            }
            (KeyModifiers::NONE, KeyCode::Char('v')) => {
                self.editor.cursor_mut().start_selection();
                self.editor.set_mode(EditorMode::Visual);
            }
            (KeyModifiers::NONE, KeyCode::Char(':')) => {
                self.editor.set_mode(EditorMode::Command);
            }
            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Char('0')) => {
                self.editor.move_to_line_start();
            }
            (KeyModifiers::NONE, KeyCode::Char('$')) | (KeyModifiers::NONE, KeyCode::End) => {
                self.editor.move_to_line_end();
            }
            (KeyModifiers::NONE, KeyCode::Char('w')) => {
                self.editor.move_word_right();
            }
            (KeyModifiers::NONE, KeyCode::Char('b')) => {
                self.editor.move_word_left();
            }
            (KeyModifiers::NONE, KeyCode::Char('g')) => {
                self.editor.move_to_buffer_start();
            }
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
                self.editor.move_to_buffer_end();
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.editor.page_up();
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.editor.page_down();
            }
            (KeyModifiers::NONE, KeyCode::Char('x')) => {
                self.editor.delete();
            }
            (KeyModifiers::NONE, KeyCode::Char('u')) => {
                self.editor.undo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
                self.editor.redo();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                }
            }
            _ => {}
        }
    }

    /// Handles editor keys in insert mode.
    fn handle_editor_insert_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.editor.set_mode(EditorMode::Normal);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.editor.backspace();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.editor.delete();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.editor.insert_char('\n');
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.editor.insert_str("    ");
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.editor.move_left();
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.editor.move_right();
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.editor.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.editor.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.editor.insert_char(c);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                if let Err(e) = self.editor.save() {
                    self.set_status(format!("Error saving: {}", e));
                }
            }
            _ => {}
        }
    }

    /// Handles editor keys in visual mode.
    fn handle_editor_visual_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.editor.cursor_mut().clear_selection();
                self.editor.set_mode(EditorMode::Normal);
            }
            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                let buffer = self.editor.buffer();
                let mut cursor = self.editor.cursor().clone();
                cursor.move_left(buffer);
                cursor.extend_to(cursor.position());
                *self.editor.cursor_mut() = cursor;
            }
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                let buffer = self.editor.buffer();
                let mut cursor = self.editor.cursor().clone();
                cursor.move_right(buffer);
                cursor.extend_to(cursor.position());
                *self.editor.cursor_mut() = cursor;
            }
            (KeyModifiers::NONE, KeyCode::Char('d')) | (KeyModifiers::NONE, KeyCode::Char('x')) => {
                self.editor.delete_selection();
                self.editor.set_mode(EditorMode::Normal);
            }
            _ => {}
        }
    }

    /// Handles editor keys in command mode.
    fn handle_editor_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.editor.set_mode(EditorMode::Normal);
            }
            _ => {}
        }
    }

    /// Handles mouse events.
    pub(super) fn handle_mouse(&mut self, event: MouseEvent) {
        // Only handle mouse events in normal mode
        if self.mode != AppMode::Normal {
            return;
        }

        // Get terminal area from cached layout
        let terminal_area = self.cached_terminal_area();

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is in terminal area
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    // Convert to terminal-local coordinates
                    let (local_col, local_row) =
                        self.to_terminal_coords(event.column, event.row, terminal_area);
                    self.start_terminal_selection(local_col, local_row);
                    // Focus terminal pane on click
                    self.layout.set_focused(FocusedPane::Terminal);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Extend selection during drag
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    let (local_col, local_row) =
                        self.to_terminal_coords(event.column, event.row, terminal_area);
                    self.update_terminal_selection(local_col, local_row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Finalize selection on mouse release
                self.finalize_terminal_selection();
            }
            MouseEventKind::ScrollUp => {
                // Scroll terminal up (into scrollback)
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    if let Some(ref mut terminals) = self.terminals {
                        if let Some(terminal) = terminals.active_terminal_mut() {
                            terminal.scroll_view_up(3);
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                // Scroll terminal down (toward cursor)
                if self.is_point_in_area(event.column, event.row, terminal_area) {
                    if let Some(ref mut terminals) = self.terminals {
                        if let Some(terminal) = terminals.active_terminal_mut() {
                            terminal.scroll_view_down(3);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Returns the cached terminal area for mouse coordinate conversion.
    fn cached_terminal_area(&self) -> ratatui::layout::Rect {
        // Returns the area cached during render
        self.last_terminal_area.get()
    }

    /// Checks if a point is within an area.
    fn is_point_in_area(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> bool {
        x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
    }

    /// Converts screen coordinates to terminal-local coordinates.
    fn to_terminal_coords(&self, x: u16, y: u16, area: ratatui::layout::Rect) -> (u16, u16) {
        // Subtract area position and account for borders/padding (1 cell each side)
        let local_x = x.saturating_sub(area.x + 1);
        let local_y = y.saturating_sub(area.y + 2); // +2 for border and tab bar
        (local_x, local_y)
    }

    /// Starts a terminal selection at the given local coordinates.
    fn start_terminal_selection(&mut self, col: u16, row: u16) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.start_selection(col, row);
            }
        }
    }

    /// Updates the terminal selection end position.
    fn update_terminal_selection(&mut self, col: u16, row: u16) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.update_selection(col, row);
            }
        }
    }

    /// Finalizes the terminal selection.
    fn finalize_terminal_selection(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            if let Some(terminal) = terminals.active_terminal_mut() {
                terminal.finalize_selection();
            }
        }
    }
}
