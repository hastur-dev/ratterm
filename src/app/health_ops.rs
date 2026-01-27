//! SSH Health Dashboard operations for the App.

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use tracing::{debug, info, warn};

use crate::daemon::DaemonManager;
use crate::ui::health_dashboard::HealthDashboard;

use super::{App, AppMode};

impl App {
    /// Opens the SSH health dashboard.
    ///
    /// Loads SSH hosts and creates a new dashboard instance.
    /// Only hosts with saved credentials will be shown (credentials are required for SSH).
    ///
    /// Optionally starts the daemon manager for real-time metrics collection
    /// (falls back to SSH-based collection if daemon is unavailable).
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

        // Start daemon manager for real-time metrics (optional enhancement)
        self.start_daemon_manager();

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

        // Clean up any existing popup/manager state before setting mode
        // This ensures we have a clean transition
        if self.ssh_manager.is_some() {
            self.ssh_manager = None;
            self.ssh_scanner = None;
        }

        // Explicitly hide and reset popup state to avoid stale state
        self.popup.hide();
        self.popup.clear();

        // Use dedicated HealthDashboard mode for proper key handling
        // This MUST be set after all cleanup to ensure correct routing
        self.mode = AppMode::HealthDashboard;

        // Show informative status based on configuration
        let daemon_status = if self.daemon_manager.as_ref().is_some_and(|d| d.is_active()) {
            " (daemon active)"
        } else {
            ""
        };

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
                "SSH Health Dashboard - Monitoring {}/{} hosts{}",
                hosts_with_creds, total_hosts, daemon_status
            )
        } else {
            format!(
                "SSH Health Dashboard - Monitoring {} hosts{}",
                hosts_with_creds, daemon_status
            )
        };
        self.set_status(status_msg);
        info!("=== HEALTH DASHBOARD OPENED ===");
    }

    /// Starts the daemon manager for real-time metrics collection.
    ///
    /// This is optional - if it fails, the dashboard will fall back to
    /// SSH-based collection.
    fn start_daemon_manager(&mut self) {
        // Don't start if already active
        if self.daemon_manager.as_ref().is_some_and(|d| d.is_active()) {
            debug!("Daemon manager already active");
            return;
        }

        info!("Starting daemon manager for real-time metrics");

        let mut manager = DaemonManager::new();
        match manager.start() {
            Ok(()) => {
                info!("Daemon manager started successfully");
                self.daemon_manager = Some(manager);
            }
            Err(e) => {
                warn!(
                    "Failed to start daemon manager (will use SSH collection): {}",
                    e
                );
                // Continue without daemon - fall back to SSH collection
            }
        }
    }

    /// Stops the daemon manager.
    fn stop_daemon_manager(&mut self) {
        if let Some(ref mut manager) = self.daemon_manager {
            info!("Stopping daemon manager");
            manager.stop();
        }
        self.daemon_manager = None;
    }

    /// Closes the SSH health dashboard.
    pub fn close_health_dashboard(&mut self) {
        info!("Closing SSH health dashboard (mode={:?})", self.mode);

        // Stop dashboard collector threads
        if let Some(ref mut dashboard) = self.health_dashboard {
            info!("Stopping dashboard collector threads");
            dashboard.stop();
        }

        // Stop daemon manager
        self.stop_daemon_manager();

        // Clear the dashboard state
        self.health_dashboard = None;

        // Ensure popup state is fully reset to avoid stale state affecting input
        self.popup.hide();
        self.popup.clear();

        // Reset mode to Normal - this must happen AFTER all cleanup
        self.mode = AppMode::Normal;

        // CRITICAL: Reset terminal raw mode to fix keyboard input
        // The daemon/collector threads can corrupt Windows console input mode,
        // causing special keys (Escape, Ctrl, arrows) to only send Release events.
        // Re-enabling raw mode restores proper keyboard handling.
        if let Err(e) = disable_raw_mode() {
            warn!("Failed to disable raw mode: {}", e);
        }
        if let Err(e) = enable_raw_mode() {
            warn!("Failed to re-enable raw mode: {}", e);
        }
        info!("Terminal raw mode reset");

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
    /// Checks both daemon metrics (if available) and SSH collector.
    pub fn poll_health_dashboard(&mut self) {
        // First, collect daemon metrics if available
        let daemon_metrics = self.collect_daemon_metrics();

        if let Some(ref mut dashboard) = self.health_dashboard {
            // Apply daemon metrics to dashboard hosts
            for (host_id, metrics) in daemon_metrics {
                for host in dashboard.hosts_mut() {
                    if host.host_id == host_id {
                        debug!(
                            "Using daemon metrics for host_id={}: cpu={}%, mem={}%",
                            host_id,
                            metrics.cpu_usage_percent,
                            metrics.memory_percent()
                        );
                        host.update_metrics(metrics);
                        break;
                    }
                }
            }

            // Poll for new metrics from SSH collector
            dashboard.poll();

            // Check if we need an auto-refresh
            if dashboard.needs_refresh() {
                dashboard.refresh(&self.ssh_hosts);
            }
        }
    }

    /// Collects fresh metrics from the daemon manager.
    ///
    /// Returns a list of (host_id, metrics) tuples for hosts with fresh daemon metrics.
    fn collect_daemon_metrics(&self) -> Vec<(u32, crate::ssh::metrics::DeviceMetrics)> {
        let Some(ref manager) = self.daemon_manager else {
            return Vec::new();
        };

        if !manager.is_active() {
            return Vec::new();
        }

        // Get host IDs from dashboard
        let host_ids: Vec<u32> = match self.health_dashboard.as_ref() {
            Some(dashboard) => dashboard.hosts().iter().map(|h| h.host_id).collect(),
            None => Vec::new(),
        };

        // Collect daemon metrics for each host
        let mut results = Vec::new();
        for host_id in host_ids {
            if let Some(metrics) = manager.get_metrics(host_id) {
                // Only use daemon metrics if they're fresh (within 5 seconds)
                if !metrics.is_stale() {
                    results.push((host_id, metrics));
                }
            }
        }

        results
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

    /// Returns a reference to the daemon manager.
    #[must_use]
    pub fn daemon_manager(&self) -> Option<&DaemonManager> {
        self.daemon_manager.as_ref()
    }

    /// Returns a mutable reference to the daemon manager.
    pub fn daemon_manager_mut(&mut self) -> Option<&mut DaemonManager> {
        self.daemon_manager.as_mut()
    }
}
