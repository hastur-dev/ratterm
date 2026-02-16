//! SSH-related input handling for the application.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::dashboard_nav::{NavResult, apply_dashboard_navigation};
use crate::ui::ssh_manager::SSHManagerMode;

use super::App;

impl App {
    /// Handles SSH manager key events.
    pub(super) fn handle_ssh_manager_key(&mut self, key: KeyEvent) {
        let Some(ref mut manager) = self.ssh_manager else {
            return;
        };

        match manager.mode() {
            SSHManagerMode::List => self.handle_ssh_list_key(key),
            SSHManagerMode::Scanning | SSHManagerMode::AuthenticatedScanning => {
                if let (KeyModifiers::NONE, KeyCode::Esc) = (key.modifiers, key.code) {
                    self.cancel_ssh_scan();
                }
            }
            SSHManagerMode::CredentialEntry => self.handle_ssh_credential_input(key),
            SSHManagerMode::Connecting => {}
            SSHManagerMode::AddHost => self.handle_ssh_add_host_input(key),
            SSHManagerMode::ScanCredentialEntry => self.handle_scan_credential_input(key),
            SSHManagerMode::EditName => self.handle_edit_name_input(key),
        }
    }

    /// Handles SSH manager list mode keys.
    fn handle_ssh_list_key(&mut self, key: KeyEvent) {
        // Unified dashboard navigation layer
        if let Some(ref mut manager) = self.ssh_manager {
            match apply_dashboard_navigation(manager, &key) {
                NavResult::Handled => return,
                NavResult::ShowHelp => {
                    self.toggle_hotkey_overlay_ssh();
                    return;
                }
                NavResult::Close => {
                    self.hide_ssh_manager();
                    return;
                }
                NavResult::Activate => {
                    self.ssh_connect_selected();
                    return;
                }
                NavResult::Unhandled => {}
            }
        }

        // SSH-specific keys layered on top
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('s')) => self.start_ssh_scan(),
            (KeyModifiers::SHIFT, KeyCode::Char('S')) => self.show_ssh_subnet_prompt(),
            (KeyModifiers::NONE, KeyCode::Char('a'))
            | (KeyModifiers::SHIFT, KeyCode::Char('A')) => self.show_ssh_add_host(),
            (KeyModifiers::NONE, KeyCode::Char('d'))
            | (KeyModifiers::SHIFT, KeyCode::Char('D'))
            | (KeyModifiers::NONE, KeyCode::Delete) => self.delete_selected_ssh_host(),
            (KeyModifiers::NONE, KeyCode::Char('c')) => self.show_scan_credential_entry(),
            (KeyModifiers::NONE, KeyCode::Char('e')) => {
                if let Some(ref mut m) = self.ssh_manager {
                    m.start_edit_name();
                }
            }
            // Open health dashboard
            (KeyModifiers::NONE, KeyCode::Char('h')) => {
                self.open_health_dashboard();
            }
            _ => {}
        }
    }

    /// Handles SSH credential key events (wrapper).
    pub(super) fn handle_ssh_credential_key(&mut self, key: KeyEvent) {
        self.handle_ssh_credential_input(key);
    }

    /// Handles SSH credential input.
    fn handle_ssh_credential_input(&mut self, key: KeyEvent) {
        let Some(ref mut manager) = self.ssh_manager else {
            return;
        };

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => manager.cancel_credential_entry(),
            (KeyModifiers::NONE, KeyCode::Tab) => manager.next_credential_field(),
            (KeyModifiers::SHIFT, KeyCode::Tab) | (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                manager.prev_credential_field();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => self.submit_ssh_credentials(),
            (KeyModifiers::NONE, KeyCode::Backspace) => manager.credential_backspace(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                manager.credential_insert(c);
            }
            _ => {}
        }
    }

    /// Handles SSH add host input.
    fn handle_ssh_add_host_input(&mut self, key: KeyEvent) {
        use crate::ui::ssh_manager::AddHostField;

        let Some(ref mut manager) = self.ssh_manager else {
            return;
        };

        // Check if we're on the JumpHost field for special handling
        let is_jump_host_field = manager.add_host_field() == AddHostField::JumpHost;

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => manager.cancel_add_host(),
            (KeyModifiers::NONE, KeyCode::Tab) => manager.next_add_host_field(),
            (KeyModifiers::SHIFT, KeyCode::Tab) | (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                manager.prev_add_host_field();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => self.submit_add_ssh_host(),
            (KeyModifiers::NONE, KeyCode::Backspace) => manager.add_host_backspace(),
            // Left/Right arrows cycle through jump hosts when on JumpHost field
            (KeyModifiers::NONE, KeyCode::Left) if is_jump_host_field => {
                manager.prev_jump_host();
            }
            (KeyModifiers::NONE, KeyCode::Right) if is_jump_host_field => {
                manager.next_jump_host();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                manager.add_host_insert(c);
            }
            _ => {}
        }
    }

    /// Handles SSH subnet key events.
    pub(super) fn handle_ssh_subnet_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.hide_popup(),
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let subnet = self.popup.input().to_string();
                self.hide_popup();
                if subnet.is_empty() {
                    self.start_ssh_scan();
                } else {
                    self.start_ssh_scan_subnet(&subnet);
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => self.popup.backspace(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.popup.insert_char(c);
            }
            _ => {}
        }
    }

    /// Handles SSH master password key events.
    pub(super) fn handle_ssh_master_password_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => self.hide_popup(),
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let password = self.popup.input().to_string();
                self.hide_popup();
                self.unlock_ssh_storage(&password);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => self.popup.backspace(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.popup.insert_char(c);
            }
            _ => {}
        }
    }

    /// Initiates SSH connection for selected host.
    pub(super) fn ssh_connect_selected(&mut self) {
        self.show_ssh_credential_prompt();
    }

    /// Shows the scan credential entry form.
    pub(super) fn show_scan_credential_entry(&mut self) {
        if let Some(ref mut manager) = self.ssh_manager {
            manager.start_scan_credential_entry();
        }
    }

    /// Handles scan credential input.
    fn handle_scan_credential_input(&mut self, key: KeyEvent) {
        let Some(ref mut manager) = self.ssh_manager else {
            return;
        };

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => manager.cancel_scan_credential_entry(),
            (KeyModifiers::NONE, KeyCode::Tab) => manager.next_scan_credential_field(),
            (KeyModifiers::SHIFT, KeyCode::Tab) | (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                manager.prev_scan_credential_field();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => self.start_authenticated_ssh_scan(),
            (KeyModifiers::NONE, KeyCode::Backspace) => manager.scan_credential_backspace(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                manager.scan_credential_insert(c);
            }
            _ => {}
        }
    }

    /// Handles edit name input.
    fn handle_edit_name_input(&mut self, key: KeyEvent) {
        let Some(ref mut manager) = self.ssh_manager else {
            return;
        };

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => manager.cancel_edit_name(),
            (KeyModifiers::NONE, KeyCode::Enter) => self.save_host_name(),
            (KeyModifiers::NONE, KeyCode::Backspace) => manager.edit_name_backspace(),
            (KeyModifiers::NONE, KeyCode::Char(c)) | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                manager.edit_name_insert(c);
            }
            _ => {}
        }
    }
}
