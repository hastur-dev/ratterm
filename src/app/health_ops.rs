//! SSH Health Dashboard operations for the App.

use tracing::{debug, info, warn};

use crate::ui::health_dashboard::HealthDashboard;

use super::{App, AppMode};

impl App {
    /// Opens the SSH health dashboard.
    ///
    /// Loads SSH hosts and creates a new dashboard instance.
    /// Only hosts with saved credentials will be shown (credentials are required for SSH).
    pub fn open_health_dashboard(&mut self) {
        info!("Opening SSH health dashboard");

        // Reload SSH hosts to ensure fresh data
        self.load_ssh_hosts();
        let total_hosts = self.ssh_hosts.len();
        let hosts_with_creds = self
            .ssh_hosts
            .hosts()
            .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
            .count();

        info!(
            "Loaded {} SSH hosts, {} have credentials",
            total_hosts, hosts_with_creds
        );

        // Log hosts without credentials for debugging
        for host in self.ssh_hosts.hosts() {
            let has_creds = self.ssh_hosts.get_credentials(host.id).is_some();
            debug!(
                "Host {}: {} (has_creds={})",
                host.id, host.hostname, has_creds
            );
            if !has_creds {
                info!(
                    "Skipping host {} ({}) - no credentials saved",
                    host.id, host.hostname
                );
            }
        }

        // Create the dashboard with current hosts
        let mut dashboard = HealthDashboard::new(&self.ssh_hosts);
        info!("Created dashboard with {} hosts", dashboard.host_count());

        // Start initial metrics collection
        dashboard.refresh(&self.ssh_hosts);
        info!("Started initial metrics collection");

        self.health_dashboard = Some(dashboard);

        // Hide SSH manager popup if visible
        if self.ssh_manager.is_some() {
            self.hide_ssh_manager();
        }

        // Show informative status based on configuration
        let status_msg = if hosts_with_creds == 0 {
            "SSH Health Dashboard - No hosts with credentials. Add credentials in SSH Manager.".to_string()
        } else if hosts_with_creds < total_hosts {
            format!(
                "SSH Health Dashboard - Monitoring {}/{} hosts (others need credentials)",
                hosts_with_creds, total_hosts
            )
        } else {
            format!(
                "SSH Health Dashboard - Monitoring {} hosts",
                hosts_with_creds
            )
        };
        self.set_status(status_msg);
    }

    /// Closes the SSH health dashboard.
    ///
    /// Ensures proper cleanup of dashboard state and resets app mode to Normal.
    pub fn close_health_dashboard(&mut self) {
        info!(
            "Closing SSH health dashboard (current mode={:?}, popup_visible={})",
            self.mode,
            self.popup.is_visible()
        );

        // Stop dashboard collector threads
        if let Some(ref mut dashboard) = self.health_dashboard {
            info!("Stopping dashboard collector threads");
            dashboard.stop();
            info!("Dashboard collector stopped");
        } else {
            warn!("close_health_dashboard called but dashboard was None");
        }

        // Clear the dashboard
        self.health_dashboard = None;
        debug!("Dashboard set to None");

        // CRITICAL: Use hide_popup() to properly clean up ALL popup-related state
        // This fixes the issue where hotkeys stop working after closing the dashboard
        // because popup state (mode_switcher, shell_selector, etc.) wasn't being cleared
        if self.popup.is_visible() {
            info!("Hiding popup with full state cleanup");
            self.hide_popup();
        } else {
            // Even if popup isn't visible, ensure mode is reset to Normal
            // in case something else changed it
            if self.mode != AppMode::Normal {
                info!("Resetting app mode from {:?} to Normal", self.mode);
                self.mode = AppMode::Normal;
            }
        }

        // Clear SSH manager if somehow still set (shouldn't happen after hide_popup)
        if self.ssh_manager.is_some() {
            info!("Clearing lingering SSH manager state");
            self.ssh_manager = None;
        }

        // Force a redraw to ensure UI updates properly
        self.needs_redraw = true;

        info!(
            "Dashboard closed, final state: mode={:?}, popup_visible={}, ssh_manager={}",
            self.mode,
            self.popup.is_visible(),
            self.ssh_manager.is_some()
        );
        self.set_status("Dashboard closed");
    }

    /// Returns whether the health dashboard is open.
    #[must_use]
    pub fn is_health_dashboard_open(&self) -> bool {
        self.health_dashboard.is_some()
    }

    /// Returns a reference to the health dashboard.
    #[must_use]
    pub fn health_dashboard(&self) -> Option<&HealthDashboard> {
        self.health_dashboard.as_ref()
    }

    /// Returns a mutable reference to the health dashboard.
    pub fn health_dashboard_mut(&mut self) -> Option<&mut HealthDashboard> {
        self.health_dashboard.as_mut()
    }

    /// Polls the health dashboard for metric updates.
    ///
    /// This should be called in the main update loop.
    pub fn poll_health_dashboard(&mut self) {
        if let Some(ref mut dashboard) = self.health_dashboard {
            // Poll for new metrics
            dashboard.poll();

            // Check if we need an auto-refresh
            if dashboard.needs_refresh() {
                dashboard.refresh(&self.ssh_hosts);
            }
        }
    }

    /// Refreshes the health dashboard metrics manually.
    pub fn refresh_health_dashboard(&mut self) {
        if let Some(ref mut dashboard) = self.health_dashboard {
            dashboard.refresh(&self.ssh_hosts);
            self.set_status("Refreshing dashboard metrics...");
        }
    }

    /// Toggles auto-refresh on the health dashboard.
    pub fn toggle_dashboard_auto_refresh(&mut self) {
        if let Some(ref mut dashboard) = self.health_dashboard {
            dashboard.toggle_auto_refresh();
            let status = if dashboard.auto_refresh() {
                "Auto-refresh enabled"
            } else {
                "Auto-refresh disabled"
            };
            self.set_status(status);
        }
    }
}
