//! Command execution for the App.

use crate::theme::ThemePreset;
use crate::ui::{layout::FocusedPane, popup::PopupKind};

use super::App;

impl App {
    /// Executes a command by its ID.
    pub(crate) fn execute_command(&mut self, command_id: &str) {
        match command_id {
            // File commands
            "file.new" => self.show_popup(PopupKind::CreateFile),
            "file.newFolder" => self.show_popup(PopupKind::CreateFolder),
            "file.open" => self.show_file_browser(),
            "file.save" => {
                self.save_current_file();
            }
            "file.close" => self.close_current_file(),

            // Edit commands
            "edit.undo" => self.editor.undo(),
            "edit.redo" => self.editor.redo(),
            "edit.copy" => {
                if let Some(text) = self.editor.selected_text() {
                    self.copy_to_clipboard(&text);
                }
            }
            "edit.paste" => {
                if let Some(text) = self.paste_from_clipboard() {
                    self.editor.insert_str(&text);
                }
            }
            "edit.selectAll" => self.editor.select_all(),
            "edit.selectLine" => self.editor.select_line(),
            "edit.duplicateLine" => self.editor.duplicate_line(),
            "edit.deleteLine" => self.editor.delete_line(),
            "edit.moveLineUp" => self.editor.move_line_up(),
            "edit.moveLineDown" => self.editor.move_line_down(),
            "edit.toggleComment" => self.editor.toggle_comment(),
            "edit.indent" => self.editor.indent(),
            "edit.outdent" => self.editor.outdent(),

            // Search commands
            "search.inFile" => self.show_popup(PopupKind::SearchInFile),
            "search.inFiles" => self.show_popup(PopupKind::SearchInFiles),
            "search.files" => self.show_popup(PopupKind::SearchFiles),
            "search.directories" => self.show_popup(PopupKind::SearchDirectories),

            // View commands
            "view.focusTerminal" => self.layout.set_focused(FocusedPane::Terminal),
            "view.focusEditor" => self.layout.set_focused(FocusedPane::Editor),
            "view.toggleFocus" => self.layout.toggle_focus(),
            "view.splitLeft" => self.move_split_left(),
            "view.splitRight" => self.move_split_right(),

            // Terminal commands
            "terminal.new" => self.add_terminal_tab(),
            "terminal.split" => self.split_terminal_horizontal(),
            "terminal.close" => self.close_terminal_tab(),
            "terminal.nextTab" => {
                if let Some(ref mut terminals) = self.terminals {
                    terminals.next_tab();
                }
            }
            "terminal.prevTab" => {
                if let Some(ref mut terminals) = self.terminals {
                    terminals.prev_tab();
                }
            }
            "terminal.selectShell" => self.show_shell_selector(),

            // SSH commands
            "ssh.manager" => self.show_ssh_manager(),
            "ssh.scan" => {
                self.show_ssh_manager();
                self.show_ssh_subnet_prompt();
            }
            "ssh.addHost" => {
                self.show_ssh_manager();
                self.show_ssh_add_host();
            }
            "ssh.connect1" => self.ssh_connect_by_index(0),
            "ssh.connect2" => self.ssh_connect_by_index(1),
            "ssh.connect3" => self.ssh_connect_by_index(2),

            // Docker commands
            "docker.manager" => self.show_docker_manager(),
            "docker.refresh" => self.refresh_docker_discovery(),
            "docker.connect1" => self.docker_connect_by_index(0),
            "docker.connect2" => self.docker_connect_by_index(1),
            "docker.connect3" => self.docker_connect_by_index(2),
            "docker.connect4" => self.docker_connect_by_index(3),
            "docker.connect5" => self.docker_connect_by_index(4),
            "docker.connect6" => self.docker_connect_by_index(5),
            "docker.connect7" => self.docker_connect_by_index(6),
            "docker.connect8" => self.docker_connect_by_index(7),
            "docker.connect9" => self.docker_connect_by_index(8),
            "docker.stats" => self.show_docker_stats(),
            "docker.logs" => self.show_docker_logs(),

            // Theme commands
            "theme.select" => self.show_theme_selector(),
            "theme.dark" => self.set_theme(ThemePreset::Dark),
            "theme.light" => self.set_theme(ThemePreset::Light),
            "theme.dracula" => self.set_theme(ThemePreset::Dracula),
            "theme.gruvbox" => self.set_theme(ThemePreset::Gruvbox),
            "theme.nord" => self.set_theme(ThemePreset::Nord),

            // Extension commands
            "extension.list" => self.show_installed_extensions(),
            "extension.install" => {
                self.set_status("Use CLI: rat ext install <user/repo>".to_string());
            }
            "extension.update" => {
                self.set_status("Use CLI: rat ext update [name]".to_string());
            }
            "extension.remove" => {
                self.set_status("Use CLI: rat ext remove <name>".to_string());
            }

            // Application commands
            "app.quit" => self.running = false,
            "app.commandPalette" => self.show_popup(PopupKind::CommandPalette),
            "app.switchEditorMode" => self.show_mode_switcher(),

            _ => self.set_status(format!("Unknown command: {}", command_id)),
        }
    }
}
