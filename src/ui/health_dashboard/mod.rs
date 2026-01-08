//! SSH Health Dashboard module.
//!
//! Provides a dashboard view for monitoring health metrics of registered SSH hosts.

mod widget;

pub use widget::HealthDashboardWidget;

use std::collections::HashMap;
use std::time::Instant;

use crate::ssh::{
    DeviceMetrics, MetricStatus, MetricsCollector, SSHHost, SSHHostList, build_collection_info,
};

/// Maximum hosts displayed in dashboard.
pub const MAX_DASHBOARD_HOSTS: usize = 50;

/// Default refresh interval in seconds.
pub const REFRESH_INTERVAL_SECS: u64 = 1;

/// Dashboard display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DashboardMode {
    /// Overview mode showing all hosts.
    #[default]
    Overview,
    /// Detail mode showing single host metrics.
    Detail,
}

/// A host displayed in the dashboard.
#[derive(Debug, Clone)]
pub struct DashboardHost {
    /// Host ID from SSHHostList.
    pub host_id: u32,
    /// Display name (or hostname if not set).
    pub display_name: String,
    /// SSH hostname or IP.
    pub hostname: String,
    /// SSH port.
    pub port: u16,
    /// Current metrics for this host.
    pub metrics: DeviceMetrics,
}

impl DashboardHost {
    /// Creates a dashboard host from an SSH host.
    #[must_use]
    pub fn from_ssh_host(host: &SSHHost) -> Self {
        Self {
            host_id: host.id,
            display_name: host
                .display_name
                .clone()
                .unwrap_or_else(|| host.hostname.clone()),
            hostname: host.hostname.clone(),
            port: host.port,
            metrics: DeviceMetrics::new(host.id),
        }
    }

    /// Updates this host's metrics.
    pub fn update_metrics(&mut self, metrics: DeviceMetrics) {
        assert_eq!(metrics.host_id, self.host_id, "Metrics host ID must match");
        self.metrics = metrics;
    }

    /// Returns the connection string for display.
    #[must_use]
    pub fn connection_string(&self) -> String {
        if self.port == 22 {
            self.hostname.clone()
        } else {
            format!("{}:{}", self.hostname, self.port)
        }
    }
}

/// SSH Health Dashboard state.
pub struct HealthDashboard {
    /// All hosts with their metrics.
    hosts: Vec<DashboardHost>,
    /// Currently selected host index.
    selected_index: usize,
    /// Scroll offset for long lists.
    scroll_offset: usize,
    /// Current display mode.
    mode: DashboardMode,
    /// Metrics collector.
    collector: MetricsCollector,
    /// Last refresh timestamp.
    last_refresh: Instant,
    /// Auto-refresh enabled.
    auto_refresh: bool,
    /// Error message if any.
    error: Option<String>,
}

impl HealthDashboard {
    /// Creates a new health dashboard from SSH hosts.
    #[must_use]
    pub fn new(ssh_hosts: &SSHHostList) -> Self {
        let hosts: Vec<DashboardHost> = ssh_hosts
            .hosts()
            .filter(|h| ssh_hosts.get_credentials(h.id).is_some())
            .take(MAX_DASHBOARD_HOSTS)
            .map(DashboardHost::from_ssh_host)
            .collect();

        Self {
            hosts,
            selected_index: 0,
            scroll_offset: 0,
            mode: DashboardMode::Overview,
            collector: MetricsCollector::new(),
            last_refresh: Instant::now(),
            auto_refresh: true,
            error: None,
        }
    }

    /// Updates the dashboard with new SSH hosts.
    pub fn update_hosts(&mut self, ssh_hosts: &SSHHostList) {
        let new_hosts: Vec<DashboardHost> = ssh_hosts
            .hosts()
            .filter(|h| ssh_hosts.get_credentials(h.id).is_some())
            .take(MAX_DASHBOARD_HOSTS)
            .map(DashboardHost::from_ssh_host)
            .collect();

        // Preserve existing metrics for hosts that still exist
        let old_metrics: HashMap<u32, DeviceMetrics> = self
            .hosts
            .iter()
            .map(|h| (h.host_id, h.metrics.clone()))
            .collect();

        self.hosts = new_hosts;

        for host in &mut self.hosts {
            if let Some(metrics) = old_metrics.get(&host.host_id) {
                host.metrics = metrics.clone();
            }
        }

        // Ensure selected index is valid
        if self.selected_index >= self.hosts.len() {
            self.selected_index = self.hosts.len().saturating_sub(1);
        }
    }

    /// Starts a metrics collection cycle.
    pub fn refresh(&mut self, ssh_hosts: &SSHHostList) {
        let collection_info = build_collection_info(ssh_hosts);
        if !collection_info.is_empty() {
            self.collector.collect(&collection_info);
            self.last_refresh = Instant::now();
            self.error = None;
        } else {
            self.error = Some("No hosts with credentials to collect".to_string());
        }
    }

    /// Polls the collector and updates metrics.
    pub fn poll(&mut self) {
        let all_metrics = self.collector.get_all_metrics();

        for host in &mut self.hosts {
            if let Some(metrics) = all_metrics.get(&host.host_id) {
                host.metrics = metrics.clone();
            }
        }
    }

    /// Returns the number of hosts.
    #[must_use]
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Returns the number of online hosts.
    #[must_use]
    pub fn online_count(&self) -> usize {
        self.hosts
            .iter()
            .filter(|h| h.metrics.status.is_online())
            .count()
    }

    /// Returns the number of offline hosts.
    #[must_use]
    pub fn offline_count(&self) -> usize {
        self.hosts
            .iter()
            .filter(|h| h.metrics.status == MetricStatus::Offline)
            .count()
    }

    /// Returns all hosts.
    #[must_use]
    pub fn hosts(&self) -> &[DashboardHost] {
        &self.hosts
    }

    /// Returns the selected host.
    #[must_use]
    pub fn selected_host(&self) -> Option<&DashboardHost> {
        self.hosts.get(self.selected_index)
    }

    /// Returns the selected index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns the current mode.
    #[must_use]
    pub fn mode(&self) -> DashboardMode {
        self.mode
    }

    /// Returns whether auto-refresh is enabled.
    #[must_use]
    pub fn auto_refresh(&self) -> bool {
        self.auto_refresh
    }

    /// Returns the error message if any.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Returns the time since last refresh.
    #[must_use]
    pub fn time_since_refresh(&self) -> u64 {
        self.last_refresh.elapsed().as_secs()
    }

    /// Returns whether a refresh is needed (for auto-refresh).
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        self.auto_refresh
            && self.last_refresh.elapsed().as_secs() >= REFRESH_INTERVAL_SECS
            && self.collector.is_collection_complete()
    }

    /// Moves selection up.
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.ensure_visible();
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        if self.selected_index < self.hosts.len().saturating_sub(1) {
            self.selected_index += 1;
            self.ensure_visible();
        }
    }

    /// Moves to first host.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Moves to last host.
    pub fn select_last(&mut self) {
        self.selected_index = self.hosts.len().saturating_sub(1);
        self.ensure_visible();
    }

    /// Ensures the selected item is visible.
    fn ensure_visible(&mut self) {
        // Assume 10 visible items - this will be adjusted during render
        let visible_count = 10;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(visible_count - 1);
        }
    }

    /// Toggles between overview and detail mode.
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            DashboardMode::Overview => DashboardMode::Detail,
            DashboardMode::Detail => DashboardMode::Overview,
        };
    }

    /// Enters detail mode for selected host.
    pub fn enter_detail(&mut self) {
        if !self.hosts.is_empty() {
            self.mode = DashboardMode::Detail;
        }
    }

    /// Returns to overview mode.
    pub fn exit_detail(&mut self) {
        self.mode = DashboardMode::Overview;
    }

    /// Toggles auto-refresh.
    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh = !self.auto_refresh;
    }

    /// Stops the collector.
    pub fn stop(&mut self) {
        self.collector.stop();
    }
}

impl Drop for HealthDashboard {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_host_connection_string() {
        let host = DashboardHost {
            host_id: 1,
            display_name: "Test".to_string(),
            hostname: "192.168.1.1".to_string(),
            port: 22,
            metrics: DeviceMetrics::new(1),
        };
        assert_eq!(host.connection_string(), "192.168.1.1");

        let host_custom_port = DashboardHost {
            host_id: 2,
            display_name: "Test".to_string(),
            hostname: "192.168.1.1".to_string(),
            port: 2222,
            metrics: DeviceMetrics::new(2),
        };
        assert_eq!(host_custom_port.connection_string(), "192.168.1.1:2222");
    }

    #[test]
    fn test_dashboard_mode_toggle() {
        let mut dashboard = HealthDashboard {
            hosts: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            mode: DashboardMode::Overview,
            collector: MetricsCollector::new(),
            last_refresh: Instant::now(),
            auto_refresh: true,
            error: None,
        };

        assert_eq!(dashboard.mode(), DashboardMode::Overview);
        dashboard.toggle_mode();
        assert_eq!(dashboard.mode(), DashboardMode::Detail);
        dashboard.toggle_mode();
        assert_eq!(dashboard.mode(), DashboardMode::Overview);
    }
}
