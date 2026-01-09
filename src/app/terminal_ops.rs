//! Terminal operations for the App.

use crate::terminal::{ProcessInfo, SSHContext};

use super::App;

impl App {
    /// Returns terminal tab information for the API.
    #[must_use]
    pub fn terminal_tabs(&self) -> Vec<crate::api::protocol::TerminalTabInfo> {
        use crate::api::protocol::TerminalTabInfo;

        if let Some(ref terminals) = self.terminals {
            let tab_info = terminals.tab_info();
            tab_info
                .iter()
                .enumerate()
                .map(|(i, info)| TerminalTabInfo {
                    index: i,
                    name: info.name.clone(),
                    active: info.is_active,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns editor tab information for the API.
    #[must_use]
    pub fn editor_tabs(&self) -> Vec<crate::api::protocol::EditorTabInfo> {
        use crate::api::protocol::EditorTabInfo;

        self.open_files
            .iter()
            .enumerate()
            .map(|(i, file)| EditorTabInfo {
                index: i,
                name: file.name.clone(),
                path: Some(file.path.to_string_lossy().into_owned()),
                modified: i == self.current_file_idx && self.editor.is_modified(),
                active: i == self.current_file_idx,
            })
            .collect()
    }

    /// Switches to the terminal tab at the given index.
    pub fn switch_terminal_tab(&mut self, index: usize) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.switch_to(index);
            self.set_status(format!("Terminal {}", index + 1));
        }
    }

    /// Returns the active terminal (if any).
    #[must_use]
    pub fn active_terminal(&self) -> Option<&crate::terminal::Terminal> {
        self.terminals.as_ref().and_then(|t| t.active_terminal())
    }

    /// Returns mutable reference to the active terminal.
    pub fn active_terminal_mut(&mut self) -> Option<&mut crate::terminal::Terminal> {
        self.terminals
            .as_mut()
            .and_then(|t| t.active_terminal_mut())
    }

    /// Adds a new terminal tab.
    pub fn add_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            let shell_name = self.config.shell.display_name();
            match terminals.add_tab_with_shell(shell_path.clone()) {
                Ok(idx) => {
                    if let Some(ref path) = shell_path {
                        self.set_status(format!(
                            "Created terminal {} with {} ({})",
                            idx + 1,
                            shell_name,
                            path.display()
                        ));
                    } else {
                        self.set_status(format!(
                            "Created terminal {} with system default",
                            idx + 1
                        ));
                    }
                }
                Err(e) => self.set_status(format!("Cannot create tab: {}", e)),
            }
        }
    }

    /// Closes the current terminal tab.
    pub fn close_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            if terminals.close_tab() {
                self.set_status("Closed terminal tab");
            } else {
                self.set_status("Cannot close last terminal tab");
            }
        }
    }

    /// Switches to next terminal tab.
    pub fn next_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.next_tab();
            let idx = terminals.active_tab_index();
            self.set_status(format!("Terminal {}", idx + 1));
        }
    }

    /// Switches to previous terminal tab.
    pub fn prev_terminal_tab(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.prev_tab();
            let idx = terminals.active_tab_index();
            self.set_status(format!("Terminal {}", idx + 1));
        }
    }

    /// Creates a horizontal split in the terminal.
    pub fn split_terminal_horizontal(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            match terminals.split_with_ssh_inheritance(shell_path) {
                Ok(is_ssh) => {
                    if is_ssh {
                        self.set_status("Split (SSH inherited)");
                    } else {
                        self.set_status("Split horizontal");
                    }
                }
                Err(e) => self.set_status(format!("Cannot split: {}", e)),
            }
        }
    }

    /// Creates a vertical split in the terminal.
    pub fn split_terminal_vertical(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            match terminals.split_with_ssh_inheritance(shell_path) {
                Ok(is_ssh) => {
                    if is_ssh {
                        self.set_status("Split (SSH inherited)");
                    } else {
                        self.set_status("Split vertical");
                    }
                }
                Err(e) => self.set_status(format!("Cannot split: {}", e)),
            }
        }
    }

    /// Closes the current terminal split.
    pub fn close_terminal_split(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.close_split();
            self.set_status("Closed split");
        }
    }

    /// Toggles focus between split terminal panes.
    pub fn toggle_terminal_split_focus(&mut self) {
        if let Some(ref mut terminals) = self.terminals {
            terminals.toggle_split_focus();
            let focus = terminals.current_split_focus();
            let pane = match focus {
                crate::terminal::SplitFocus::First => "first",
                crate::terminal::SplitFocus::Second => "second",
            };
            self.set_status(format!("Focus: {} pane", pane));
        }
    }

    /// Gets the SSH context from the active terminal, if it's an SSH session.
    #[must_use]
    pub fn get_active_ssh_context(&self) -> Option<SSHContext> {
        let terminals = self.terminals.as_ref()?;
        let terminal = terminals.active_terminal()?;
        terminal.ssh_context().cloned()
    }

    /// Debug helper to check SSH terminal state.
    #[must_use]
    pub fn debug_ssh_state(&self) -> String {
        let Some(ref terminals) = self.terminals else {
            return "No terminals".to_string();
        };

        let Some(terminal) = terminals.active_terminal() else {
            return "No active terminal".to_string();
        };

        let is_ssh = terminal.is_ssh();
        let has_context = terminal.ssh_context().is_some();

        if let Some(ctx) = terminal.ssh_context() {
            format!(
                "is_ssh={}, has_context={}, user={}@{}:{}",
                is_ssh, has_context, ctx.username, ctx.hostname, ctx.port
            )
        } else {
            format!("is_ssh={}, has_context={}", is_ssh, has_context)
        }
    }

    /// Debug helper to show all terminal tabs and their SSH status.
    #[must_use]
    pub fn debug_all_tabs(&self) -> String {
        let Some(ref terminals) = self.terminals else {
            return "No terminals".to_string();
        };

        let active_tab = terminals.active_tab_index();
        let tab_count = terminals.tab_count();

        let mut info = format!("Tabs:{} Active:{} | ", tab_count, active_tab);

        for i in 0..tab_count {
            if let Some(tab) = terminals.get_tab(i) {
                let name = tab.name();
                if let Some(term) = tab.focused_terminal() {
                    let ssh_info = if let Some(ctx) = term.ssh_context() {
                        format!("SSH:{}@{}", ctx.username, ctx.hostname)
                    } else {
                        "Local".to_string()
                    };
                    info.push_str(&format!("[{}:{}:{}] ", i, name, ssh_info));
                } else {
                    info.push_str(&format!("[{}:{}:NoTerm] ", i, name));
                }
            }
        }

        info
    }

    /// Starts a command in the background.
    ///
    /// # Errors
    /// Returns error message if the process cannot be started.
    pub fn start_background_process(&mut self, command: &str) -> Result<u64, String> {
        let id = self.background_manager.start(command)?;
        self.set_status(format!("Started background process {} : {}", id, command));
        Ok(id)
    }

    /// Lists all background processes with counts.
    #[must_use]
    pub fn list_background_processes(&mut self) -> (Vec<ProcessInfo>, usize, usize) {
        self.background_manager.update_counts();
        let processes = self.background_manager.list();
        let running = self.background_manager.running_count();
        let errors = self.background_manager.error_count();
        (processes, running, errors)
    }

    /// Gets information about a specific background process.
    #[must_use]
    pub fn get_background_process_info(&self, id: u64) -> Option<ProcessInfo> {
        self.background_manager.get_info(id)
    }

    /// Gets the output of a specific background process.
    #[must_use]
    pub fn get_background_process_output(&self, id: u64) -> Option<String> {
        self.background_manager.get_output(id)
    }

    /// Kills a background process.
    ///
    /// # Errors
    /// Returns error message if the process cannot be killed.
    pub fn kill_background_process(&mut self, id: u64) -> Result<(), String> {
        self.background_manager.kill(id)?;
        self.set_status(format!("Killed background process {}", id));
        Ok(())
    }

    /// Clears finished background processes.
    pub fn clear_finished_background_processes(&mut self) {
        self.background_manager.clear_finished();
        self.background_manager.clear_errors();
        self.set_status("Cleared finished background processes".to_string());
    }

    /// Returns the number of running background processes.
    #[must_use]
    pub fn background_running_count(&self) -> usize {
        self.background_manager.running_count()
    }

    /// Returns the number of background processes with errors.
    #[must_use]
    pub fn background_error_count(&self) -> usize {
        self.background_manager.error_count()
    }

    /// Runs an addon command in a new terminal tab.
    ///
    /// Creates a new terminal tab and immediately executes the given command.
    pub fn run_addon_command(&mut self, addon_name: &str, command: &str) {
        if let Some(ref mut terminals) = self.terminals {
            let shell_path = self.config.shell.get_shell_path();
            match terminals.add_tab_with_shell(shell_path) {
                Ok(idx) => {
                    // Get the active terminal and send the command
                    if let Some(terminal) = terminals.active_terminal_mut() {
                        // Write the command followed by Enter
                        let cmd_with_newline = format!("{}\r", command);
                        if let Err(e) = terminal.write(cmd_with_newline.as_bytes()) {
                            self.set_status(format!("Addon {}: failed to send command: {}", addon_name, e));
                            return;
                        }
                    }
                    self.set_status(format!("Addon {}: started in terminal {}", addon_name, idx + 1));
                }
                Err(e) => {
                    self.set_status(format!("Addon {}: cannot create tab: {}", addon_name, e));
                }
            }
        }
    }
}
