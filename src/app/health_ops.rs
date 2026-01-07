//! SSH Health Dashboard operations for the App.

use crate::ui::health_dashboard::HealthDashboard;

use super::App;

impl App {
    /// Opens the SSH health dashboard.
    ///
    /// Loads SSH hosts and creates a new dashboard instance.
    pub fn open_health_dashboard(&mut self) {
        // Reload SSH hosts to ensure fresh data
        self.load_ssh_hosts();

        // Create the dashboard with current hosts
        let mut dashboard = HealthDashboard::new(&self.ssh_hosts);

        // Start initial metrics collection
        dashboard.refresh(&self.ssh_hosts);

        self.health_dashboard = Some(dashboard);

        // Hide SSH manager popup if visible
        if self.ssh_manager.is_some() {
            self.hide_ssh_manager();
        }

        self.set_status("SSH Health Dashboard - Press 'q' or Esc to close");
    }

    /// Closes the SSH health dashboard.
    pub fn close_health_dashboard(&mut self) {
        if let Some(ref mut dashboard) = self.health_dashboard {
            dashboard.stop();
        }
        self.health_dashboard = None;
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
