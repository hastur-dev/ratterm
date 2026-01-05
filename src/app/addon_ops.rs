//! Add-on manager operations.
//!
//! App methods for managing add-ons: showing/hiding the popup,
//! fetching addons, and installing/uninstalling.

use tracing::{debug, info, warn};

use crate::addons::{FetchResult, InstalledAddon, ScriptType};
use crate::ui::addon_manager::{AddonManagerMode, AddonManagerSelector};
use crate::ui::popup::PopupKind;

use super::{App, AppMode};

impl App {
    /// Shows the add-on manager popup.
    pub fn show_addon_manager(&mut self) {
        info!("show_addon_manager: opening");

        // Initialize manager if not already
        if self.addon_manager.is_none() {
            let mut selector = AddonManagerSelector::new();
            // Load config values
            selector.set_repository(
                self.addon_config.repository.clone(),
                self.addon_config.branch.clone(),
            );
            self.addon_manager = Some(selector);
        }

        // Start fetching addons
        self.refresh_addon_list(false);

        // Show popup
        self.popup.set_kind(PopupKind::AddonManager);
        self.popup.show();
        self.mode = AppMode::Popup;
    }

    /// Hides the add-on manager popup.
    pub fn hide_addon_manager(&mut self) {
        self.popup.hide();
        self.mode = AppMode::Normal;
        self.request_redraw();

        // Clear any pending state
        if let Some(ref mut manager) = self.addon_manager {
            manager.return_to_list();
        }
    }

    /// Refreshes the addon list from GitHub (non-blocking).
    ///
    /// This sends a request to the background fetcher. Call `poll_addon_fetcher()`
    /// in the event loop to receive results.
    pub fn refresh_addon_list(&mut self, force_refresh: bool) {
        info!("[ADDON-OPS] refresh_addon_list: force={}", force_refresh);

        if let Some(ref mut manager) = self.addon_manager {
            manager.set_mode(AddonManagerMode::Fetching);
        }

        // Send request to background fetcher (non-blocking)
        self.addon_fetcher.request_addon_list(force_refresh);
    }

    /// Polls for results from the background addon fetcher.
    ///
    /// Call this in the main event loop to process async fetch results.
    pub fn poll_addon_fetcher(&mut self) {
        // Check for results from background fetcher
        if let Some(result) = self.addon_fetcher.poll_result() {
            debug!("[ADDON-OPS] Received fetch result");

            match result {
                FetchResult::AddonList(addons) => {
                    info!("[ADDON-OPS] poll_addon_fetcher: received {} addons", addons.len());

                    if let Some(ref mut manager) = self.addon_manager {
                        manager.update_from_config(addons, self.addon_config.installed.clone());
                        manager.set_mode(AddonManagerMode::List);
                    }
                }
                FetchResult::Script { addon_id, script_type, content } => {
                    info!("[ADDON-OPS] poll_addon_fetcher: received script for {} ({:?}), {} bytes",
                        addon_id, script_type, content.len());

                    // Handle the fetched script - continue with installation
                    self.handle_fetched_script(&addon_id, script_type, content);
                }
                FetchResult::Error(e) => {
                    warn!("[ADDON-OPS] poll_addon_fetcher: error: {}", e);
                    self.set_status(format!("Failed to fetch add-ons: {}", e));

                    if let Some(ref mut manager) = self.addon_manager {
                        // set_error automatically sets mode to Error
                        manager.set_error(Some(e.to_string()));
                    }
                }
            }
        }
    }

    /// Handles a fetched script by continuing the installation/uninstallation process.
    fn handle_fetched_script(&mut self, addon_id: &str, script_type: ScriptType, content: String) {
        match script_type {
            ScriptType::Install => {
                // Write script and start background process
                match self.addon_installer.start_install_with_content(
                    addon_id,
                    &content,
                    &mut self.background_manager,
                ) {
                    Ok(progress) => {
                        if let Some(ref mut manager) = self.addon_manager {
                            manager.set_install_progress(Some(progress));
                        }
                        self.set_status(format!("Installing {}...", addon_id));
                    }
                    Err(e) => {
                        warn!("[ADDON-OPS] handle_fetched_script: install error: {}", e);
                        self.set_status(format!("Failed to install: {}", e));

                        if let Some(ref mut manager) = self.addon_manager {
                            manager.install_failed(e.to_string());
                        }
                    }
                }
            }
            ScriptType::Uninstall => {
                // Write script and start background process
                match self.addon_installer.start_uninstall_with_content(
                    addon_id,
                    &content,
                    &mut self.background_manager,
                ) {
                    Ok(progress) => {
                        if let Some(ref mut manager) = self.addon_manager {
                            manager.set_install_progress(Some(progress));
                        }
                        self.set_status(format!("Uninstalling {}...", addon_id));
                    }
                    Err(e) => {
                        warn!("[ADDON-OPS] handle_fetched_script: uninstall error: {}", e);
                        self.set_status(format!("Failed to uninstall: {}", e));

                        if let Some(ref mut manager) = self.addon_manager {
                            manager.install_failed(e.to_string());
                        }
                    }
                }
            }
        }
    }

    /// Starts installing an addon (non-blocking).
    ///
    /// This sends a request to fetch the install script. The actual installation
    /// happens when the script is received via `poll_addon_fetcher()`.
    pub fn start_addon_install(&mut self, addon_id: &str) {
        info!("[ADDON-OPS] start_addon_install: {}", addon_id);

        // Set the pending addon in manager
        if let Some(ref mut manager) = self.addon_manager {
            manager.set_pending_addon_id(Some(addon_id.to_string()));
            manager.set_uninstalling(false);
            manager.set_mode(AddonManagerMode::Installing);
        }

        // Request install script fetch (non-blocking)
        self.addon_fetcher.request_script(addon_id, ScriptType::Install);
        self.set_status(format!("Downloading {}...", addon_id));
    }

    /// Checks and updates addon install/uninstall progress.
    pub fn check_addon_install_progress(&mut self) {
        let (progress, is_uninstalling) = match self.addon_manager.as_ref() {
            Some(m) => (m.install_progress().cloned(), m.is_uninstalling()),
            None => return,
        };

        let progress = match progress {
            Some(p) => p,
            None => return,
        };

        // Check if background process has finished
        if let Some(updated) = self
            .addon_installer
            .check_install_complete(&progress, &self.background_manager)
        {
            let addon_id = updated.addon_id.clone();

            if updated.phase.is_error() {
                let error = updated.error.unwrap_or_else(|| "Unknown error".to_string());
                warn!("check_addon_install_progress: failed: {}", error);

                if let Some(ref mut manager) = self.addon_manager {
                    manager.install_failed(error);
                }
            } else if is_uninstalling {
                info!("check_addon_install_progress: uninstall complete: {}", addon_id);

                // Uninstall complete - remove from config
                self.complete_addon_uninstall(&addon_id);
            } else {
                info!("check_addon_install_progress: install complete: {}", addon_id);

                // Installation complete - save to config and return to list
                self.complete_addon_install(&addon_id);
            }
        }
    }

    /// Completes addon installation by saving to config.
    fn complete_addon_install(&mut self, addon_id: &str) {
        info!("complete_addon_install: {}", addon_id);

        // Create installed addon with display name
        let display_name = addon_id
            .replace(['-', '_'], " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let installed = InstalledAddon::new(addon_id.to_string(), display_name);

        // Add to config
        self.addon_config.add_installed(installed.clone());

        // Save to storage
        if let Err(e) = self.addon_storage.save_addon(&installed) {
            warn!("complete_addon_install: save error: {}", e);
            self.set_status(format!("Warning: Failed to save config: {}", e));
        }

        // Update manager and return to list
        if let Some(ref mut manager) = self.addon_manager {
            manager.update_from_config(
                vec![], // Will be repopulated on next refresh
                self.addon_config.installed.clone(),
            );
            manager.return_to_list();
        }

        self.set_status(format!("Installed: {}", addon_id));

        // Refresh the list to show updated status
        self.refresh_addon_list(false);
    }

    /// Starts uninstalling an addon (non-blocking).
    ///
    /// This sends a request to fetch the uninstall script. The actual uninstallation
    /// happens when the script is received via `poll_addon_fetcher()`.
    pub fn start_addon_uninstall(&mut self, addon_id: &str) {
        info!("[ADDON-OPS] start_addon_uninstall: {}", addon_id);

        // Set the pending addon in manager
        if let Some(ref mut manager) = self.addon_manager {
            manager.set_pending_addon_id(Some(addon_id.to_string()));
            manager.set_uninstalling(true);
            manager.set_mode(AddonManagerMode::Installing);
        }

        // Request uninstall script fetch (non-blocking)
        self.addon_fetcher.request_script(addon_id, ScriptType::Uninstall);
        self.set_status(format!("Downloading uninstall script for {}...", addon_id));
    }

    /// Completes addon uninstallation by removing from config.
    fn complete_addon_uninstall(&mut self, addon_id: &str) {
        info!("complete_addon_uninstall: {}", addon_id);

        // Remove from config
        self.addon_config.remove_installed(addon_id);

        // Remove from storage
        if let Err(e) = self.addon_storage.remove_addon(addon_id) {
            warn!("complete_addon_uninstall: storage error: {}", e);
            self.set_status(format!("Warning: Failed to update config: {}", e));
        }

        // Update manager and return to list
        if let Some(ref mut manager) = self.addon_manager {
            manager.set_installed_addons(self.addon_config.installed.clone());
            manager.return_to_list();
        }

        self.set_status(format!("Uninstalled: {}", addon_id));

        // Refresh the list to show updated status
        self.refresh_addon_list(false);
    }
}
