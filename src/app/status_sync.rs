//! Bridges health dashboard metrics into SSH manager connection statuses.
//!
//! The health dashboard collects [`MetricStatus`] from daemon/SSH collectors,
//! while the SSH manager displays [`ConnectionStatus`]. This module provides
//! the pure mapping between the two and the sync logic that keeps
//! `App::host_statuses` up-to-date.

use crate::ssh::{ConnectionStatus, MetricStatus};
use crate::ui::health_dashboard::DashboardHost;

/// Maps a [`MetricStatus`] to a [`ConnectionStatus`].
///
/// # Mapping
/// - `Online` / `Collecting` → `Reachable`
/// - `Offline` / `Error` → `Unreachable`
/// - `Unknown` → `Unknown`
#[must_use]
pub fn metric_to_connection_status(metric: MetricStatus) -> ConnectionStatus {
    match metric {
        MetricStatus::Online | MetricStatus::Collecting => ConnectionStatus::Reachable,
        MetricStatus::Offline | MetricStatus::Error => ConnectionStatus::Unreachable,
        MetricStatus::Unknown => ConnectionStatus::Unknown,
    }
}

/// Extracts `(host_id, ConnectionStatus)` pairs from dashboard hosts.
///
/// Only includes hosts whose derived status differs from [`ConnectionStatus::Unknown`],
/// so callers can avoid needless updates.
#[must_use]
pub fn statuses_from_dashboard(hosts: &[DashboardHost]) -> Vec<(u32, ConnectionStatus)> {
    assert!(
        hosts.len() <= 50,
        "Dashboard should not exceed MAX_DASHBOARD_HOSTS"
    );

    hosts
        .iter()
        .map(|h| (h.host_id, metric_to_connection_status(h.metrics.status)))
        .filter(|(_, status)| *status != ConnectionStatus::Unknown)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::DeviceMetrics;
    use std::time::Instant;

    // ── metric_to_connection_status ──────────────────────────────────

    #[test]
    fn test_online_maps_to_reachable() {
        assert_eq!(
            metric_to_connection_status(MetricStatus::Online),
            ConnectionStatus::Reachable,
        );
    }

    #[test]
    fn test_collecting_maps_to_reachable() {
        assert_eq!(
            metric_to_connection_status(MetricStatus::Collecting),
            ConnectionStatus::Reachable,
        );
    }

    #[test]
    fn test_offline_maps_to_unreachable() {
        assert_eq!(
            metric_to_connection_status(MetricStatus::Offline),
            ConnectionStatus::Unreachable,
        );
    }

    #[test]
    fn test_error_maps_to_unreachable() {
        assert_eq!(
            metric_to_connection_status(MetricStatus::Error),
            ConnectionStatus::Unreachable,
        );
    }

    #[test]
    fn test_unknown_maps_to_unknown() {
        assert_eq!(
            metric_to_connection_status(MetricStatus::Unknown),
            ConnectionStatus::Unknown,
        );
    }

    // ── statuses_from_dashboard ─────────────────────────────────────

    fn make_dashboard_host(id: u32, status: MetricStatus) -> DashboardHost {
        let mut metrics = DeviceMetrics::new(id);
        metrics.status = status;
        metrics.timestamp = Instant::now();
        DashboardHost {
            host_id: id,
            display_name: format!("host-{}", id),
            hostname: format!("192.168.1.{}", id),
            port: 22,
            metrics,
        }
    }

    #[test]
    fn test_extracts_non_unknown_statuses() {
        let hosts = vec![
            make_dashboard_host(1, MetricStatus::Online),
            make_dashboard_host(2, MetricStatus::Unknown),
            make_dashboard_host(3, MetricStatus::Offline),
        ];

        let result = statuses_from_dashboard(&hosts);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (1, ConnectionStatus::Reachable));
        assert_eq!(result[1], (3, ConnectionStatus::Unreachable));
    }

    #[test]
    fn test_empty_dashboard_returns_empty() {
        let result = statuses_from_dashboard(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_all_unknown_returns_empty() {
        let hosts = vec![
            make_dashboard_host(1, MetricStatus::Unknown),
            make_dashboard_host(2, MetricStatus::Unknown),
        ];

        let result = statuses_from_dashboard(&hosts);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mixed_statuses() {
        let hosts = vec![
            make_dashboard_host(10, MetricStatus::Online),
            make_dashboard_host(20, MetricStatus::Collecting),
            make_dashboard_host(30, MetricStatus::Error),
            make_dashboard_host(40, MetricStatus::Offline),
        ];

        let result = statuses_from_dashboard(&hosts);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], (10, ConnectionStatus::Reachable));
        assert_eq!(result[1], (20, ConnectionStatus::Reachable));
        assert_eq!(result[2], (30, ConnectionStatus::Unreachable));
        assert_eq!(result[3], (40, ConnectionStatus::Unreachable));
    }
}
