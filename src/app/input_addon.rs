//! Add-on manager input handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tracing::info;

use crate::ui::addon_manager::{AddonListSection, AddonManagerMode};

use super::App;

impl App {
    /// Handles key events for the add-on manager popup.
    pub fn handle_addon_manager_key(&mut self, key: KeyEvent) {
        let Some(ref manager) = self.addon_manager else {
            return;
        };

        match manager.mode() {
            AddonManagerMode::List => {
                self.handle_addon_list_key(key);
            }
            AddonManagerMode::Fetching => {
                // Allow escape during fetch
                self.handle_addon_loading_key(key);
            }
            AddonManagerMode::Installing => {
                // No input during installation
            }
            AddonManagerMode::ConfirmUninstall => {
                self.handle_addon_uninstall_confirm_key(key);
            }
            AddonManagerMode::Error => {
                self.handle_addon_error_key(key);
            }
        }
    }

    /// Handles key events in addon list mode.
    fn handle_addon_list_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Close manager
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.hide_addon_manager();
            }

            // Navigation
            (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.select_prev();
                }
            }
            (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.select_next();
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.select_first();
                }
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.select_last();
                }
            }

            // Section toggle
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.toggle_section();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                if let Some(ref mut manager) = self.addon_manager {
                    if manager.section() != AddonListSection::Available {
                        manager.toggle_section();
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                if let Some(ref mut manager) = self.addon_manager {
                    if manager.section() != AddonListSection::Installed {
                        manager.toggle_section();
                    }
                }
            }

            // Refresh
            (KeyModifiers::NONE, KeyCode::F(5)) => {
                self.refresh_addon_list(true);
            }

            // Install selected (from available section only)
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.handle_addon_enter();
            }

            // Uninstall
            (KeyModifiers::NONE, KeyCode::Char('d') | KeyCode::Delete) => {
                self.handle_addon_uninstall();
            }

            _ => {}
        }
    }

    /// Handles Enter key in addon list.
    fn handle_addon_enter(&mut self) {
        let (section, addon_id, is_installable) = match self.addon_manager.as_ref() {
            Some(m) => {
                let section = m.section();
                let addon = m.selected_addon();
                let id = addon.map(|a| a.addon.id.clone());
                let installable = addon.map(|a| a.addon.is_installable()).unwrap_or(false);
                (section, id, installable)
            }
            None => return,
        };

        let addon_id = match addon_id {
            Some(id) => id,
            None => return,
        };

        match section {
            AddonListSection::Available => {
                // Start installation if installable
                if is_installable {
                    info!("Installing addon: {}", addon_id);
                    self.start_addon_install(&addon_id);
                } else {
                    self.set_status(format!("'{}' is not available for this platform", addon_id));
                }
            }
            AddonListSection::Installed => {
                // Re-install (update) the addon
                info!("Re-installing addon: {}", addon_id);
                self.start_addon_install(&addon_id);
            }
        }
    }

    /// Handles uninstall request.
    fn handle_addon_uninstall(&mut self) {
        let section = match self.addon_manager.as_ref() {
            Some(m) => m.section(),
            None => return,
        };

        // Only allow uninstall from installed section
        if section != AddonListSection::Installed {
            return;
        }

        if let Some(ref mut manager) = self.addon_manager {
            manager.set_mode(AddonManagerMode::ConfirmUninstall);
        }
    }

    /// Handles key events during loading.
    fn handle_addon_loading_key(&mut self, key: KeyEvent) {
        if let (KeyModifiers::NONE, KeyCode::Esc) = (key.modifiers, key.code) {
            self.hide_addon_manager();
        }
    }

    /// Handles key events in uninstall confirmation mode.
    fn handle_addon_uninstall_confirm_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // Cancel
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if let Some(ref mut manager) = self.addon_manager {
                    manager.return_to_list();
                }
            }

            // Confirm uninstall
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let addon_id = self
                    .addon_manager
                    .as_ref()
                    .and_then(|m| m.selected_addon())
                    .map(|a| a.addon.id.clone());

                if let Some(id) = addon_id {
                    self.uninstall_addon(&id);
                }

                if let Some(ref mut manager) = self.addon_manager {
                    manager.return_to_list();
                }
            }

            _ => {}
        }
    }

    /// Handles key events in error mode.
    fn handle_addon_error_key(&mut self, key: KeyEvent) {
        if let (KeyModifiers::NONE, KeyCode::Esc | KeyCode::Enter) = (key.modifiers, key.code) {
            if let Some(ref mut manager) = self.addon_manager {
                manager.clear_error();
            }
        }
    }
}
