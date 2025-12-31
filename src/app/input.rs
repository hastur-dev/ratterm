//! Input handling for the application.
//!
//! Handles key events for different application modes.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::is_windows_11;
use crate::ui::layout::FocusedPane;
use crate::ui::popup::PopupKind;

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
        if self.handle_global_key(key) {
            return;
        }

        if self.layout.focused() == FocusedPane::Editor && self.handle_editor_global_key(key) {
            return;
        }

        match self.layout.focused() {
            FocusedPane::Terminal => self.handle_terminal_key(key),
            FocusedPane::Editor => self.handle_editor_key(key),
        }
    }

    /// Handles global keybindings. Returns true if handled.
    fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('i')) => {
                self.toggle_ide();
                true
            }
            (KeyModifiers::ALT, KeyCode::Left) => {
                self.layout.set_focused(FocusedPane::Terminal);
                true
            }
            (KeyModifiers::ALT, KeyCode::Right) => {
                if self.layout.ide_visible() {
                    self.layout.set_focused(FocusedPane::Editor);
                }
                true
            }
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
                self.move_split_left();
                true
            }
            (KeyModifiers::ALT, KeyCode::Char(']')) => {
                self.move_split_right();
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
            // Command palette: F1 on Windows 11, Ctrl+Shift+P on other platforms
            (KeyModifiers::NONE, KeyCode::F(1)) if is_windows_11() => {
                self.show_popup(PopupKind::CommandPalette);
                true
            }
            (m, KeyCode::Char('p') | KeyCode::Char('P'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT && !is_windows_11() =>
            {
                self.show_popup(PopupKind::CommandPalette);
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p')) if !is_windows_11() => {
                self.show_popup(PopupKind::CommandPalette);
                true
            }
            (m, KeyCode::Tab) | (m, KeyCode::BackTab)
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                if self.is_mode_switcher_active() {
                    self.cycle_mode_next();
                } else {
                    self.show_mode_switcher();
                    self.cycle_mode_next();
                }
                true
            }
            (m, KeyCode::Char('c') | KeyCode::Char('C'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.copy_selection();
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                self.paste_clipboard();
                true
            }
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
            (m, KeyCode::Char('u') | KeyCode::Char('U'))
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.show_ssh_manager();
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Char(c @ '1'..='9'))
                if self.config.ssh_number_setting =>
            {
                let idx = (c as u8 - b'1') as usize;
                self.ssh_connect_by_index(idx);
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
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                self.new_editor_tab();
                true
            }
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
        // Check if we're browsing remote or local
        if self.remote_file_browser.is_some() {
            self.handle_remote_file_browser_key(key);
        } else {
            self.handle_local_file_browser_key(key);
        }
    }

    /// Handles keys for the local file browser.
    fn handle_local_file_browser_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.hide_file_browser(),
            (KeyModifiers::NONE, KeyCode::Up)
            | (KeyModifiers::NONE, KeyCode::Char('k'))
            | (KeyModifiers::NONE, KeyCode::Char('w')) => self.file_browser.move_up(),
            (KeyModifiers::NONE, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::Char('j'))
            | (KeyModifiers::NONE, KeyCode::Char('s')) => self.file_browser.move_down(),
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
            (KeyModifiers::NONE, KeyCode::PageUp) => self.file_browser.page_up(),
            (KeyModifiers::NONE, KeyCode::PageDown) => self.file_browser.page_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.file_browser.move_to_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.file_browser.move_to_end(),
            (KeyModifiers::NONE, KeyCode::Char('/')) => self.show_popup(PopupKind::SearchFiles),
            _ => {}
        }
    }

    /// Handles keys for the remote file browser.
    fn handle_remote_file_browser_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.hide_file_browser(),
            (KeyModifiers::NONE, KeyCode::Up)
            | (KeyModifiers::NONE, KeyCode::Char('k'))
            | (KeyModifiers::NONE, KeyCode::Char('w')) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.move_up();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::Char('j'))
            | (KeyModifiers::NONE, KeyCode::Char('s')) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.move_down();
                }
            }
            (KeyModifiers::NONE, KeyCode::Left)
            | (KeyModifiers::NONE, KeyCode::Char('h'))
            | (KeyModifiers::NONE, KeyCode::Char('a')) => {
                self.remote_browser_go_up();
            }
            (KeyModifiers::NONE, KeyCode::Right)
            | (KeyModifiers::NONE, KeyCode::Char('l'))
            | (KeyModifiers::NONE, KeyCode::Char('d'))
            | (KeyModifiers::NONE, KeyCode::Enter) => {
                self.remote_browser_enter_selected();
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.page_up();
                }
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.page_down();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.move_to_start();
                }
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                if let Some(ref mut browser) = self.remote_file_browser {
                    browser.move_to_end();
                }
            }
            _ => {}
        }
    }

    /// Goes up a directory in the remote file browser.
    fn remote_browser_go_up(&mut self) {
        // We need to take ownership temporarily to call methods that need &mut self
        if let Some(mut browser) = self.remote_file_browser.take() {
            if let Err(e) = browser.go_up(&mut self.remote_manager) {
                self.set_status(format!("Failed to go up: {}", e));
            } else {
                // Update status with new directory
                let dir = browser.current_dir().to_string();
                let ctx = browser.ssh_context();
                self.set_status(format!("[SSH] {}@{}: {}", ctx.username, ctx.hostname, dir));
            }
            self.remote_file_browser = Some(browser);
        }
    }

    /// Enters the selected item in the remote file browser.
    fn remote_browser_enter_selected(&mut self) {
        // We need to take ownership temporarily
        if let Some(mut browser) = self.remote_file_browser.take() {
            // Capture context and CWD before the blocking operation
            let ctx = browser.ssh_context().clone();
            let cwd = browser.current_dir().to_string();

            match browser.enter_selected(&mut self.remote_manager) {
                Ok(Some(path)) => {
                    // File was selected - open it
                    // Don't put the browser back - open_remote_file_with_cwd will clear it
                    self.remote_file_browser = Some(browser);
                    // Use the known CWD to avoid extra blocking call
                    self.open_remote_file_with_cwd(&ctx, &path, Some(&cwd));
                }
                Ok(None) => {
                    // Directory was entered
                    let dir = browser.current_dir().to_string();
                    self.set_status(format!("[SSH] {}@{}: {}", ctx.username, ctx.hostname, dir));
                    self.remote_file_browser = Some(browser);
                }
                Err(e) => {
                    self.set_status(format!("Failed to open: {}", e));
                    self.remote_file_browser = Some(browser);
                }
            }
        }
    }

    /// Handles keys in popup mode.
    fn handle_popup_key(&mut self, key: KeyEvent) {
        if self.popup.kind().is_confirmation() {
            self.handle_confirmation_key(key);
            return;
        }
        if self.popup.kind().is_keybinding_notification() {
            self.handle_keybinding_notification_key(key);
            return;
        }
        if self.popup.kind().is_mode_switcher() {
            self.handle_mode_switcher_key(key);
            return;
        }
        if self.popup.kind().is_shell_selector() {
            self.handle_shell_selector_key(key);
            return;
        }
        if self.popup.kind().is_shell_install_prompt() {
            self.handle_shell_install_prompt_key(key);
            return;
        }
        if self.popup.kind().is_theme_selector() {
            self.handle_theme_selector_key(key);
            return;
        }
        if self.popup.kind().is_ssh_manager() {
            self.handle_ssh_manager_key(key);
            return;
        }
        if self.popup.kind().is_ssh_credential_prompt() {
            self.handle_ssh_credential_key(key);
            return;
        }
        if self.popup.kind().is_ssh_subnet_entry() {
            self.handle_ssh_subnet_key(key);
            return;
        }
        if self.popup.kind().is_ssh_master_password() {
            self.handle_ssh_master_password_key(key);
            return;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.hide_popup(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.execute_popup_action(),
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.popup.backspace();
                self.update_popup_results();
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                self.popup.delete();
                self.update_popup_results();
            }
            (KeyModifiers::NONE, KeyCode::Left) => self.popup.move_left(),
            (KeyModifiers::NONE, KeyCode::Right) => self.popup.move_right(),
            (KeyModifiers::NONE, KeyCode::Up) => self.popup.result_up(),
            (KeyModifiers::NONE, KeyCode::Down) => self.popup.result_down(),
            (KeyModifiers::NONE, KeyCode::Home) => self.popup.move_to_start(),
            (KeyModifiers::NONE, KeyCode::End) => self.popup.move_to_end(),
            (KeyModifiers::NONE, KeyCode::Tab) => self.popup.accept_suggestion(),
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

    fn handle_mode_switcher_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.cancel_mode_switch(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.apply_mode_switch(),
            (KeyModifiers::NONE, KeyCode::Tab)
            | (KeyModifiers::NONE, KeyCode::Down)
            | (KeyModifiers::NONE, KeyCode::Char('j')) => self.cycle_mode_next(),
            (KeyModifiers::SHIFT, KeyCode::Tab)
            | (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::Up)
            | (KeyModifiers::NONE, KeyCode::Char('k')) => self.cycle_mode_prev(),
            (m, KeyCode::Tab) | (m, KeyCode::BackTab)
                if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT =>
            {
                self.cycle_mode_next();
            }
            _ => {}
        }
    }

    fn handle_shell_selector_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.cancel_shell_selection(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.apply_shell_selection(),
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.cycle_shell_next();
            }
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.cycle_shell_prev();
            }
            _ => {}
        }
    }

    fn handle_shell_install_prompt_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Enter) => {
                self.hide_popup();
            }
            _ => {}
        }
    }

    fn handle_theme_selector_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.cancel_theme_selection(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.apply_theme_selection(),
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.cycle_theme_next();
            }
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.cycle_theme_prev();
            }
            _ => {}
        }
    }

    fn cycle_theme_next(&mut self) {
        if let Some(ref mut selector) = self.theme_selector {
            selector.next();
        }
    }

    fn cycle_theme_prev(&mut self) {
        if let Some(ref mut selector) = self.theme_selector {
            selector.prev();
        }
    }

    fn handle_confirmation_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('y'))
            | (KeyModifiers::SHIFT, KeyCode::Char('Y')) => {
                self.hide_popup();
                self.save_and_quit();
            }
            (KeyModifiers::NONE, KeyCode::Char('n'))
            | (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
                self.hide_popup();
                self.force_quit();
            }
            (KeyModifiers::NONE, KeyCode::Char('c'))
            | (KeyModifiers::SHIFT, KeyCode::Char('C'))
            | (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_popup();
            }
            _ => {}
        }
    }

    fn handle_keybinding_notification_key(&mut self, key: KeyEvent) {
        // Dismiss the notification on any key press
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc)
            | (KeyModifiers::NONE, KeyCode::Enter)
            | (KeyModifiers::NONE, KeyCode::Char(_)) => {
                self.hide_popup();
                self.mark_win11_notification_shown();
            }
            _ => {}
        }
    }
}
