//! Test utilities for the daemon system.
//!
//! Provides standalone functions to test daemon components without the TUI.

use std::time::Duration;

use tracing::{error, info};

use super::DaemonManager;
use super::receiver::DEFAULT_RECEIVER_PORT;
use super::types::DaemonMetrics;

/// Result of a daemon system test.
#[derive(Debug, Clone)]
pub struct DaemonTestResult {
    /// Whether the receiver started successfully.
    pub receiver_started: bool,
    /// Error message if receiver failed to start.
    pub receiver_error: Option<String>,
    /// Whether a test metric was received.
    pub metric_received: bool,
    /// Port the receiver is listening on.
    pub port: u16,
}

impl DaemonTestResult {
    /// Returns true if all tests passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.receiver_started && self.metric_received
    }

    /// Returns a summary string.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_success() {
            format!("Daemon system OK (port {})", self.port)
        } else if !self.receiver_started {
            format!(
                "Receiver failed: {}",
                self.receiver_error.as_deref().unwrap_or("unknown")
            )
        } else {
            "Test metric not received".to_string()
        }
    }
}

/// Tests the daemon receiver by starting it and sending a test metric.
///
/// This is a standalone test that doesn't require the TUI or health dashboard.
///
/// # Returns
/// A `DaemonTestResult` with the test outcome.
pub fn test_daemon_receiver() -> DaemonTestResult {
    info!("=== DAEMON RECEIVER TEST ===");

    let mut result = DaemonTestResult {
        receiver_started: false,
        receiver_error: None,
        metric_received: false,
        port: DEFAULT_RECEIVER_PORT,
    };

    // Step 1: Start the daemon manager
    info!("Step 1: Starting daemon manager...");
    let mut manager = DaemonManager::new();

    match manager.start() {
        Ok(()) => {
            info!(
                "Daemon manager started successfully on port {}",
                DEFAULT_RECEIVER_PORT
            );
            result.receiver_started = true;
        }
        Err(e) => {
            let err_msg = format!("{}", e);
            error!("Failed to start daemon manager: {}", err_msg);
            result.receiver_error = Some(err_msg);
            return result;
        }
    }

    // Step 2: Send a test metric via HTTP
    info!("Step 2: Sending test metric...");
    let test_metric = create_test_metric(999);

    match send_test_metric(&test_metric) {
        Ok(()) => {
            info!("Test metric sent successfully");
        }
        Err(e) => {
            error!("Failed to send test metric: {}", e);
            result.receiver_error = Some(format!("HTTP POST failed: {}", e));
            manager.stop();
            return result;
        }
    }

    // Step 3: Wait and verify metric was received
    info!("Step 3: Verifying metric received...");
    std::thread::sleep(Duration::from_millis(500));

    if let Some(metrics) = manager.get_metrics(999) {
        info!(
            "Test metric received! CPU cores: {}, Mem total: {} MB",
            metrics.cpu_cores, metrics.mem_total_mb
        );
        result.metric_received = true;
    } else {
        error!("Test metric not found in cache");
    }

    // Cleanup
    info!("Cleanup: Stopping daemon manager...");
    manager.stop();

    info!("=== TEST COMPLETE: {} ===", result.summary());
    result
}

/// Creates a test metric payload.
fn create_test_metric(host_id: u32) -> DaemonMetrics {
    DaemonMetrics {
        host_id: host_id.to_string(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        cpu: super::types::DaemonCpuMetrics {
            load: vec![1.0, 0.8, 0.6],
            cores: 8,
        },
        mem: super::types::DaemonMemMetrics {
            total: 16384,
            avail: 8192,
            swap_total: 4096,
            swap_used: 512,
        },
        disk: super::types::DaemonDiskMetrics {
            total: 500,
            used: 250,
        },
        gpu: None,
    }
}

/// Sends a test metric to the local receiver via HTTP POST.
fn send_test_metric(metric: &DaemonMetrics) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/metrics", DEFAULT_RECEIVER_PORT);
    let json = serde_json::to_string(metric).map_err(|e| format!("JSON error: {}", e))?;

    // Use reqwest blocking client
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(json)
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("HTTP {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

/// Checks if port 19999 is available.
#[must_use]
pub fn is_port_available() -> bool {
    use std::net::TcpListener;

    TcpListener::bind(("127.0.0.1", DEFAULT_RECEIVER_PORT)).is_ok()
}

/// Returns diagnostic info about the daemon system.
#[must_use]
pub fn get_diagnostics() -> String {
    let mut lines = Vec::new();

    lines.push("=== Daemon System Diagnostics ===".to_string());
    lines.push(format!("Receiver port: {}", DEFAULT_RECEIVER_PORT));
    lines.push(format!(
        "Port available: {}",
        if is_port_available() {
            "Yes"
        } else {
            "No (in use)"
        }
    ));

    // Check if running as admin on Windows
    #[cfg(windows)]
    {
        lines.push("Platform: Windows".to_string());
        // Note: Checking admin status requires additional dependencies
    }

    #[cfg(not(windows))]
    {
        lines.push(format!("Platform: Unix"));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_metric() {
        let metric = create_test_metric(42);
        assert_eq!(metric.host_id, "42");
        assert_eq!(metric.cpu.cores, 8);
        assert_eq!(metric.mem.total, 16384);
    }

    #[test]
    fn test_daemon_test_result_summary() {
        let success = DaemonTestResult {
            receiver_started: true,
            receiver_error: None,
            metric_received: true,
            port: 19999,
        };
        assert!(success.is_success());
        assert!(success.summary().contains("OK"));

        let failure = DaemonTestResult {
            receiver_started: false,
            receiver_error: Some("Access denied".to_string()),
            metric_received: false,
            port: 19999,
        };
        assert!(!failure.is_success());
        assert!(failure.summary().contains("Access denied"));
    }

    #[test]
    fn test_is_port_available() {
        // Just verify the function doesn't panic
        let _ = is_port_available();
    }

    #[test]
    fn test_get_diagnostics() {
        let diag = get_diagnostics();
        assert!(diag.contains("Daemon System Diagnostics"));
        assert!(diag.contains("19999"));
    }
}
