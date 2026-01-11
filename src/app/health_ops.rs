//! SSH Health Dashboard operations for the App.

use tracing::{info, warn};

use crate::ui::health_dashboard::HealthDashboard;

use super::{App, AppMode};

impl App {
    /// Opens the SSH health dashboard.
    ///
    /// Loads SSH hosts and creates a new dashboard instance.
    /// Only hosts with saved credentials will be shown (credentials are required for SSH).
    pub fn open_health_dashboard(&mut self) {
        info!("=== OPENING SSH HEALTH DASHBOARD ===");
        info!(
            "open_health_dashboard: Storage path={:?}, mode={:?}",
            self.ssh_storage.path(),
            self.ssh_storage.mode()
        );

        // Log pre-load state
        let pre_load_hosts = self.ssh_hosts.len();
        let pre_load_creds = self
            .ssh_hosts
            .hosts()
            .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
            .count();
        info!(
            "open_health_dashboard: PRE-LOAD state: {} hosts, {} with credentials",
            pre_load_hosts, pre_load_creds
        );

        // Reload SSH hosts to ensure fresh data
        self.load_ssh_hosts();

        // Log post-load state
        let total_hosts = self.ssh_hosts.len();
        let hosts_with_creds = self
            .ssh_hosts
            .hosts()
            .filter(|h| self.ssh_hosts.get_credentials(h.id).is_some())
            .count();

        info!(
            "open_health_dashboard: POST-LOAD state: {} hosts, {} with credentials",
            total_hosts, hosts_with_creds
        );

        // Detect if load caused credential loss
        if pre_load_creds > 0 && hosts_with_creds == 0 {
            warn!(
                "open_health_dashboard: CREDENTIAL LOSS DETECTED! Had {} creds before load, now have 0. \
                 This suggests the storage file doesn't contain credentials or failed to parse.",
                pre_load_creds
            );
        }

        // Log detailed host/credential state for debugging
        info!("open_health_dashboard: Host/Credential Details:");
        for host in self.ssh_hosts.hosts() {
            let creds = self.ssh_hosts.get_credentials(host.id);
            match creds {
                Some(c) => {
                    info!(
                        "  [OK] Host {} '{}' (id={}): username='{}', has_password={}, has_key={}, save={}",
                        host.hostname,
                        host.display_name.as_deref().unwrap_or("-"),
                        host.id,
                        c.username,
                        c.password.is_some(),
                        c.key_path.is_some(),
                        c.save
                    );
                }
                None => {
                    warn!(
                        "  [MISSING] Host {} '{}' (id={}): NO CREDENTIALS - will be excluded from dashboard",
                        host.hostname,
                        host.display_name.as_deref().unwrap_or("-"),
                        host.id
                    );
                }
            }
        }

        // Create the dashboard with current hosts
        let mut dashboard = HealthDashboard::new(&self.ssh_hosts);
        info!(
            "open_health_dashboard: Created dashboard with {} hosts (filtered from {} total)",
            dashboard.host_count(),
            total_hosts
        );

        if dashboard.host_count() == 0 && total_hosts > 0 {
            warn!(
                "open_health_dashboard: Dashboard has 0 hosts but {} exist! \
                 All hosts are missing credentials.",
                total_hosts
            );
        }

        // Start initial metrics collection
        dashboard.refresh(&self.ssh_hosts);
        info!("open_health_dashboard: Started initial metrics collection");

        self.health_dashboard = Some(dashboard);

        // Hide SSH manager popup if visible
        if self.ssh_manager.is_some() {
            self.hide_ssh_manager();
        }

        // Use dedicated HealthDashboard mode for proper key handling
        self.mode = AppMode::HealthDashboard;

        // Show informative status based on configuration
        let status_msg = if hosts_with_creds == 0 {
            if total_hosts == 0 {
                "SSH Health Dashboard - No hosts configured. Add hosts in SSH Manager.".to_string()
            } else {
                format!(
                    "SSH Health Dashboard - {} hosts found but NONE have credentials. Save credentials in SSH Manager.",
                    total_hosts
                )
            }
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
        info!("=== HEALTH DASHBOARD OPENED ===");
    }

    /// Closes the SSH health dashboard.
    pub fn close_health_dashboard(&mut self) {
        info!("Closing SSH health dashboard (mode={:?})", self.mode);

        // Stop dashboard collector threads
        if let Some(ref mut dashboard) = self.health_dashboard {
            info!("Stopping dashboard collector threads");
            dashboard.stop();
        }

        // Clear the dashboard state
        self.health_dashboard = None;

        // Reset mode to Normal
        self.mode = AppMode::Normal;

        self.set_status("Dashboard closed");
        info!("Dashboard closed, mode={:?}", self.mode);
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
