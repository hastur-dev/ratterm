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

    /// Loads SSH hosts from storage.
    pub(crate) fn load_ssh_hosts(&mut self) {
        match self.ssh_storage.load() {
            Ok(hosts) => {
                let creds_count = hosts
                    .hosts()
                    .filter(|h| hosts.get_credentials(h.id).is_some())
                    .count();
                info!(
                    "Loaded {} SSH hosts with {} credentials from storage",
                    hosts.len(),
                    creds_count
                );
                self.set_status(format!(
                    "Loaded {} hosts, {} with credentials",
                    hosts.len(),
                    creds_count
                ));
                for host in hosts.hosts() {
                    let has_creds = hosts.get_credentials(host.id).is_some();
                    info!(
                        "  - Loaded host {}: {} (has_creds={})",
                        host.id, host.hostname, has_creds
                    );
                }
                self.ssh_hosts = hosts;
            }
            Err(e) => {
                warn!("Failed to load SSH hosts: {}", e);
                self.set_status(format!("Failed to load SSH hosts: {}", e));
                self.ssh_hosts = crate::ssh::SSHHostList::new();
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
