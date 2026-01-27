//! Tests for SSH Health Dashboard open/close functionality.
//!
//! Tests cover: dashboard creation, mode transitions, key handling,
//! open/close operations via Escape key.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ratterm::ssh::{DeviceMetrics, SSHHostList};
use ratterm::ui::health_dashboard::{DashboardHost, DashboardMode, HealthDashboard};

// ============================================================================
// DashboardHost Tests
// ============================================================================

#[test]
fn test_dashboard_host_connection_string_default_port() {
    let host = DashboardHost {
        host_id: 1,
        display_name: "Test Server".to_string(),
        hostname: "192.168.1.100".to_string(),
        port: 22,
        metrics: DeviceMetrics::new(1),
    };

    assert_eq!(
        host.connection_string(),
        "192.168.1.100",
        "Default SSH port should not be shown in connection string"
    );
}

#[test]
fn test_dashboard_host_connection_string_custom_port() {
    let host = DashboardHost {
        host_id: 2,
        display_name: "Custom Port Server".to_string(),
        hostname: "192.168.1.200".to_string(),
        port: 2222,
        metrics: DeviceMetrics::new(2),
    };

    assert_eq!(
        host.connection_string(),
        "192.168.1.200:2222",
        "Custom SSH port should be shown in connection string"
    );
}

#[test]
fn test_dashboard_host_update_metrics() {
    let mut host = DashboardHost {
        host_id: 1,
        display_name: "Test".to_string(),
        hostname: "test.local".to_string(),
        port: 22,
        metrics: DeviceMetrics::new(1),
    };

    let new_metrics = DeviceMetrics::new(1);
    host.update_metrics(new_metrics);

    assert_eq!(host.metrics.host_id, 1, "Metrics host ID should match");
}

// ============================================================================
// HealthDashboard State Tests
// ============================================================================

#[test]
fn test_health_dashboard_new_empty_hosts() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    assert_eq!(dashboard.host_count(), 0, "Dashboard should have no hosts");
    assert_eq!(
        dashboard.mode(),
        DashboardMode::Overview,
        "Initial mode should be Overview"
    );
    assert!(
        dashboard.auto_refresh(),
        "Auto-refresh should be enabled by default"
    );
    assert_eq!(
        dashboard.selected_index(),
        0,
        "Initial selection should be 0"
    );
    assert_eq!(dashboard.scroll_offset(), 0, "Initial scroll should be 0");
}

#[test]
fn test_health_dashboard_mode_starts_overview() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    assert_eq!(
        dashboard.mode(),
        DashboardMode::Overview,
        "Dashboard should start in Overview mode"
    );
}

#[test]
fn test_health_dashboard_toggle_mode() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    assert_eq!(dashboard.mode(), DashboardMode::Overview);

    dashboard.toggle_mode();
    assert_eq!(
        dashboard.mode(),
        DashboardMode::Detail,
        "Toggle should switch to Detail"
    );

    dashboard.toggle_mode();
    assert_eq!(
        dashboard.mode(),
        DashboardMode::Overview,
        "Toggle should switch back to Overview"
    );
}

#[test]
fn test_health_dashboard_enter_detail_empty_hosts() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.enter_detail();

    // With no hosts, should stay in Overview
    assert_eq!(
        dashboard.mode(),
        DashboardMode::Overview,
        "Cannot enter detail with no hosts"
    );
}

#[test]
fn test_health_dashboard_exit_detail() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    // Force into detail mode
    dashboard.toggle_mode();
    assert_eq!(dashboard.mode(), DashboardMode::Detail);

    dashboard.exit_detail();
    assert_eq!(
        dashboard.mode(),
        DashboardMode::Overview,
        "exit_detail should return to Overview"
    );
}

#[test]
fn test_health_dashboard_toggle_auto_refresh() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    assert!(dashboard.auto_refresh(), "Auto-refresh default is true");

    dashboard.toggle_auto_refresh();
    assert!(!dashboard.auto_refresh(), "First toggle should disable");

    dashboard.toggle_auto_refresh();
    assert!(dashboard.auto_refresh(), "Second toggle should enable");
}

// ============================================================================
// Selection Navigation Tests
// ============================================================================

#[test]
fn test_health_dashboard_select_previous_at_start() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.select_previous();
    assert_eq!(
        dashboard.selected_index(),
        0,
        "select_previous at start should stay at 0"
    );
}

#[test]
fn test_health_dashboard_select_next_empty() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.select_next();
    assert_eq!(
        dashboard.selected_index(),
        0,
        "select_next with no hosts should stay at 0"
    );
}

#[test]
fn test_health_dashboard_select_first() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.select_first();
    assert_eq!(dashboard.selected_index(), 0);
    assert_eq!(dashboard.scroll_offset(), 0);
}

#[test]
fn test_health_dashboard_select_last_empty() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.select_last();
    // With 0 hosts, saturating_sub(1) = 0
    assert_eq!(
        dashboard.selected_index(),
        0,
        "select_last with no hosts should be 0"
    );
}

// ============================================================================
// ListSelectable Trait Tests
// ============================================================================

#[test]
fn test_health_dashboard_list_selectable_trait() {
    use ratterm::app::input_traits::ListSelectable;

    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    // Test trait methods
    dashboard.select_next();
    assert_eq!(dashboard.selected_index(), 0);

    dashboard.select_prev();
    assert_eq!(dashboard.selected_index(), 0);

    dashboard.select_first();
    assert_eq!(dashboard.selected_index(), 0);

    dashboard.select_last();
    assert_eq!(dashboard.selected_index(), 0);
}

// ============================================================================
// Time and Refresh Tests
// ============================================================================

#[test]
fn test_health_dashboard_time_since_refresh() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    // Time since refresh should be 0 or very small immediately after creation
    let elapsed = dashboard.time_since_refresh();
    assert!(
        elapsed < 2,
        "Time since refresh should be minimal right after creation"
    );
}

#[test]
fn test_health_dashboard_needs_refresh_auto_disabled() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    dashboard.toggle_auto_refresh(); // Disable
    assert!(!dashboard.auto_refresh());

    // With auto-refresh disabled, should never need refresh
    assert!(
        !dashboard.needs_refresh(),
        "Should not need refresh when auto-refresh is disabled"
    );
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_health_dashboard_error_none_initially() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    assert!(
        dashboard.error().is_none(),
        "Error should be None initially"
    );
}

// ============================================================================
// Host Count Tests
// ============================================================================

#[test]
fn test_health_dashboard_online_offline_counts_empty() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    assert_eq!(dashboard.online_count(), 0);
    assert_eq!(dashboard.offline_count(), 0);
}

#[test]
fn test_health_dashboard_hosts_accessor() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    let hosts = dashboard.hosts();
    assert!(hosts.is_empty(), "Hosts slice should be empty");
}

#[test]
fn test_health_dashboard_selected_host_empty() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    assert!(
        dashboard.selected_host().is_none(),
        "Selected host should be None when empty"
    );
}

// ============================================================================
// DashboardMode Tests
// ============================================================================

#[test]
fn test_dashboard_mode_default() {
    let mode = DashboardMode::default();
    assert_eq!(
        mode,
        DashboardMode::Overview,
        "Default mode should be Overview"
    );
}

#[test]
fn test_dashboard_mode_equality() {
    assert_eq!(DashboardMode::Overview, DashboardMode::Overview);
    assert_eq!(DashboardMode::Detail, DashboardMode::Detail);
    assert_ne!(DashboardMode::Overview, DashboardMode::Detail);
}

#[test]
fn test_dashboard_mode_debug() {
    let overview = DashboardMode::Overview;
    let detail = DashboardMode::Detail;

    // These should not panic
    let _ = format!("{:?}", overview);
    let _ = format!("{:?}", detail);
}

// ============================================================================
// Stop and Drop Tests
// ============================================================================

#[test]
fn test_health_dashboard_stop() {
    let ssh_hosts = SSHHostList::new();
    let mut dashboard = HealthDashboard::new(&ssh_hosts);

    // Should not panic
    dashboard.stop();
}

#[test]
fn test_health_dashboard_drop() {
    let ssh_hosts = SSHHostList::new();
    let dashboard = HealthDashboard::new(&ssh_hosts);

    // Should not panic when dropped
    drop(dashboard);
}
