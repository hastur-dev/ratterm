//! Core SSH manager operations for the App.

use tracing::{debug, info, warn};

use crate::ui::{popup::PopupKind, ssh_manager::SSHManagerMode};

use super::{App, AppMode};

impl App {
    /// Shows the SSH manager popup.
    pub fn show_ssh_manager(&mut self) {
        // Always reload SSH hosts from storage to ensure we have fresh data
        self.load_ssh_hosts();

        // Count credentials for debug
        let creds_count = self
            .ssh_hosts
            .hosts()
            .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
            .count();

        // Create or update the SSH manager selector
        let mut selector = self.ssh_manager.take().unwrap_or_default();
        selector.update_from_list(&self.ssh_hosts);
        selector.set_mode(SSHManagerMode::List);
        selector.clear_error();
        self.ssh_manager = Some(selector);

        // Show the popup
        self.popup.set_kind(PopupKind::SSHManager);
        self.popup.show();
        self.mode = AppMode::Popup;
        self.set_status(format!(
            "SSH Manager - {} hosts, {} with creds | S=scan A=add Enter=connect",
            self.ssh_hosts.len(),
            creds_count
        ));
    }

    /// Hides the SSH manager popup.
    pub fn hide_ssh_manager(&mut self) {
        self.ssh_manager = None;
        self.ssh_scanner = None;
        self.hide_popup();
    }

    /// Toggles the hotkey overlay for the SSH manager.
    ///
    /// Stub — actual overlay implementation is in Phase 2.
    pub fn toggle_hotkey_overlay_ssh(&mut self) {
        info!("SSH hotkey overlay toggled (stub — Phase 2 will implement)");
    }

    /// Loads SSH hosts from storage.
    pub(crate) fn load_ssh_hosts(&mut self) {
        info!(
            "load_ssh_hosts: Starting load from {:?}",
            self.ssh_storage.path()
        );
        info!(
            "load_ssh_hosts: Current storage mode={:?}, needs_master_password={}",
            self.ssh_storage.mode(),
            self.ssh_storage.needs_master_password()
        );

        match self.ssh_storage.load() {
            Ok(hosts) => {
                let total_hosts = hosts.len();
                let creds_count = hosts
                    .hosts()
                    .filter(|h| hosts.get_credentials(h.id).is_some())
                    .count();

                info!(
                    "load_ssh_hosts: SUCCESS - Loaded {} hosts with {} credentials",
                    total_hosts, creds_count
                );

                // Log each host and its credential status
                for host in hosts.hosts() {
                    let creds = hosts.get_credentials(host.id);
                    if let Some(c) = creds {
                        info!(
                            "  - Host {} '{}': has_creds=true, username='{}', has_password={}, has_key={}",
                            host.id,
                            host.hostname,
                            c.username,
                            c.password.is_some(),
                            c.key_path.is_some()
                        );
                    } else {
                        info!("  - Host {} '{}': has_creds=false", host.id, host.hostname);
                    }
                }

                // Log warning if hosts exist but no credentials
                if total_hosts > 0 && creds_count == 0 {
                    warn!(
                        "load_ssh_hosts: {} hosts loaded but NONE have credentials! \
                         Check if credentials were saved with 'save' checkbox enabled.",
                        total_hosts
                    );
                }

                self.set_status(format!(
                    "Loaded {} hosts, {} with credentials",
                    total_hosts, creds_count
                ));
                self.ssh_hosts = hosts;
            }
            Err(e) => {
                // Detailed error logging based on error type
                let error_msg = format!("{}", e);
                warn!("load_ssh_hosts: FAILED - {}", error_msg);

                // Provide specific guidance based on error type
                let user_msg = if error_msg.contains("Master password required") {
                    warn!(
                        "load_ssh_hosts: Storage is encrypted but no master password set. \
                         User must unlock storage first."
                    );
                    "SSH storage locked - enter master password to unlock".to_string()
                } else if error_msg.contains("Parse error") {
                    warn!(
                        "load_ssh_hosts: TOML parse error - storage file may be corrupted. \
                         Check {:?} for syntax errors.",
                        self.ssh_storage.path()
                    );
                    format!("SSH storage file corrupted: {}", error_msg)
                } else if error_msg.contains("IO error") {
                    warn!(
                        "load_ssh_hosts: IO error reading {:?} - check file permissions",
                        self.ssh_storage.path()
                    );
                    format!("Cannot read SSH storage: {}", error_msg)
                } else {
                    format!("Failed to load SSH hosts: {}", error_msg)
                };

                self.set_status(user_msg);

                // IMPORTANT: Don't reset to empty if we already have hosts in memory
                // This preserves in-memory state if disk load fails
                if self.ssh_hosts.is_empty() {
                    info!("load_ssh_hosts: No existing hosts in memory, initializing empty list");
                    self.ssh_hosts = crate::ssh::SSHHostList::new();
                } else {
                    warn!(
                        "load_ssh_hosts: Preserving {} existing in-memory hosts despite load failure",
                        self.ssh_hosts.len()
                    );
                }
            }
        }
    }

    /// Saves SSH hosts to storage.
    pub(crate) fn save_ssh_hosts(&mut self) {
        info!(
            "Saving SSH hosts: {} hosts, {} credentials",
            self.ssh_hosts.len(),
            self.ssh_hosts
                .hosts()
                .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
                .count()
        );
        for host in self.ssh_hosts.hosts() {
            let has_creds = self.ssh_hosts.get_credentials(host.id).is_some();
            info!(
                "  - Host {}: {} (has_creds={})",
                host.id, host.hostname, has_creds
            );
        }

        if let Err(e) = self.ssh_storage.save(&self.ssh_hosts) {
            warn!("Failed to save SSH hosts: {}", e);
            self.set_status(format!("Failed to save SSH hosts: {}", e));
        } else {
            debug!("Saved {} SSH hosts", self.ssh_hosts.len());
        }
    }

    /// Returns whether the SSH manager is currently visible.
    #[must_use]
    pub fn is_ssh_manager_visible(&self) -> bool {
        self.ssh_manager.is_some() && self.popup.kind().is_ssh_popup()
    }

    /// Returns a reference to the SSH manager selector.
    #[must_use]
    pub fn ssh_manager(&self) -> Option<&crate::ui::ssh_manager::SSHManagerSelector> {
        self.ssh_manager.as_ref()
    }

    /// Returns a mutable reference to the SSH manager selector.
    pub fn ssh_manager_mut(&mut self) -> Option<&mut crate::ui::ssh_manager::SSHManagerSelector> {
        self.ssh_manager.as_mut()
    }

    /// Shows the SSH subnet prompt for manual subnet entry.
    pub fn show_ssh_subnet_prompt(&mut self) {
        if let Some(ref mut manager) = self.ssh_manager {
            manager.set_mode(SSHManagerMode::ScanCredentialEntry);
        }
    }

    /// Shows the add host form.
    pub fn show_ssh_add_host(&mut self) {
        if let Some(ref mut manager) = self.ssh_manager {
            manager.set_mode(SSHManagerMode::AddHost);
        }
    }
}
