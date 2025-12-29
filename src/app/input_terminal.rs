//! Terminal input handling for the application.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;
use super::keymap::key_to_bytes;

impl App {
    /// Handles key events for the terminal pane.
    pub(super) fn handle_terminal_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                self.add_terminal_tab();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                self.close_terminal_tab();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.prev_terminal_tab();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.next_terminal_tab();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                self.split_terminal_horizontal();
                return;
            }
            (m, KeyCode::Char('s') | KeyCode::Char('S'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.split_terminal_vertical();
                return;
            }
            (m, KeyCode::Char('w') | KeyCode::Char('W'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.close_terminal_split();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Tab) => {
                self.toggle_terminal_split_focus();
                return;
            }
            (m, KeyCode::Char('c') | KeyCode::Char('C'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.copy_terminal_selection();
                return;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                self.paste_to_terminal();
                return;
            }
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

        let cmd_result = self.process_terminal_input(key);
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
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                let _ = terminal.send_interrupt();
                None
            }
            (KeyModifiers::SHIFT, KeyCode::PageUp) => {
                terminal.scroll_view_up(10);
                None
            }
            (KeyModifiers::SHIFT, KeyCode::PageDown) => {
                terminal.scroll_view_down(10);
                None
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                match terminal.process_input(c) {
                    Ok(Some(cmd)) => Some(cmd),
                    Ok(None) | Err(_) => None,
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => match terminal.process_input('\r') {
                Ok(Some(cmd)) => Some(cmd),
                Ok(None) | Err(_) => None,
            },
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let _ = terminal.process_input('\x7f');
                None
            }
            _ => {
                let bytes = key_to_bytes(key);
                if !bytes.is_empty() {
                    terminal.scroll_to_cursor();
                    let _ = terminal.write(&bytes);
                }
                None
            }
        }
    }

    /// Copies terminal selection to clipboard (or current line if no selection).
    pub(super) fn copy_terminal_selection(&mut self) {
        let text_to_copy: Option<(String, bool)> = {
            if let Some(ref terminals) = self.terminals {
                if let Some(terminal) = terminals.active_terminal() {
                    if let Some(text) = terminal.selected_text() {
                        if !text.is_empty() {
                            Some((text, true))
                        } else {
                            None
                        }
                    } else {
                        let grid = terminal.grid();
                        let (_, row) = grid.cursor_pos();
                        if let Some(line) = grid.row(row as usize) {
                            let text: String = line.cells().iter().map(|c| c.character()).collect();
                            let text = text.trim_end().to_string();
                            if !text.is_empty() {
                                Some((text, false))
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

        if let Some((text, from_selection)) = text_to_copy {
            self.copy_to_clipboard(&text);
            if from_selection {
                self.set_status("Copied selection");
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
    pub(super) fn paste_to_terminal(&mut self) {
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
            let debug_state = self.debug_ssh_state();
            if let Some(ssh_context) = self.get_active_ssh_context() {
                self.set_status(format!(
                    "REMOTE FILE BROWSER: {}@{} [{}]",
                    ssh_context.username, ssh_context.hostname, debug_state
                ));
                self.show_ide();
                self.show_remote_file_browser(&ssh_context);
            } else {
                self.set_status(format!("LOCAL FILE BROWSER [{}]", debug_state));
                self.show_ide();
                self.show_file_browser();
            }
        } else if let Some(filename) = cmd.strip_prefix("open ") {
            self.handle_open_file_command(filename.trim());
        } else if cmd == "update" {
            self.handle_update_command();
        } else if cmd == "debug ssh" {
            let state = self.debug_ssh_state();
            self.set_status(format!("SSH State: {}", state));
        } else if cmd == "debug tabs" {
            let info = self.debug_all_tabs();
            self.set_status(info);
        } else if let Some(buffer) = cmd.strip_prefix("debug buffer:") {
            self.set_status(format!("Buffer: [{}]", buffer));
        }
    }

    /// Handles the 'open <filename>' command.
    fn handle_open_file_command(&mut self, filename: &str) {
        let debug_state = self.debug_ssh_state();

        if let Some(ssh_context) = self.get_active_ssh_context() {
            self.set_status(format!(
                "REMOTE: Opening '{}' from {}@{} [{}]",
                filename, ssh_context.username, ssh_context.hostname, debug_state
            ));
            self.show_ide();
            self.open_remote_file(&ssh_context, filename);
        } else {
            self.set_status(format!("LOCAL: Opening '{}' [{}]", filename, debug_state));

            let cwd = self
                .terminals
                .as_ref()
                .and_then(|t| t.active_terminal())
                .map(|t| t.current_working_dir())
                .unwrap_or_else(|| self.file_browser.path().to_path_buf());

            let path = if std::path::Path::new(filename).is_absolute() {
                std::path::PathBuf::from(filename)
            } else {
                cwd.join(filename)
            };

            if path.exists() {
                if path.is_file() {
                    self.show_ide();
                    let _ = self.open_file(path);
                } else if path.is_dir() {
                    self.show_ide();
                    let _ = self.file_browser.change_dir(&path);
                    self.show_file_browser();
                }
            } else {
                self.set_status(format!(
                    "LOCAL: File not found: {} [{}]",
                    path.display(),
                    debug_state
                ));
            }
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

                if let Err(e) = self.save_session() {
                    self.set_status(format!("Failed to save session: {e}"));
                    return;
                }

                match updater.update_and_restart(&version) {
                    Ok(true) => {
                        self.set_status(format!("Updated to v{version}! Restarting..."));
                        self.request_restart_after_update = true;
                        self.running = false;
                    }
                    Ok(false) => {
                        self.set_status(format!("Already running v{VERSION} (latest version)"));
                    }
                    Err(e) => {
                        self.set_status(format!("Update failed: {e}"));
                    }
                }
            }
            UpdateStatus::UpToDate => {
                self.set_status(format!("Already up to date (v{VERSION})"));
            }
            UpdateStatus::Failed(e) => {
                self.set_status(format!("Update check failed: {e}"));
            }
            UpdateStatus::Disabled => {
                self.set_status("Updates disabled via RATTERM_NO_UPDATE".to_string());
            }
        }
    }
}
