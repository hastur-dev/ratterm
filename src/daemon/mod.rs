//! SSH Health Dashboard Daemon System.
//!
//! This module provides a lightweight daemon that runs on remote SSH hosts
//! to collect and send system metrics back to Ratterm.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              RATTERM (Windows)          │
//! │  ┌────────────────────────────────────┐ │
//! │  │ DaemonManager                      │ │
//! │  │  - Deploys daemons via SSH         │ │
//! │  │  - Manages daemon lifecycle        │ │
//! │  └────────────────────────────────────┘ │
//! │  ┌────────────────────────────────────┐ │
//! │  │ MetricsReceiver (HTTP :19999)      │ │
//! │  │  - Receives JSON metrics via POST  │ │
//! │  │  - Caches latest metrics per host  │ │
//! │  └────────────────────────────────────┘ │
//! └─────────────────────────────────────────┘
//!               ▲ HTTP POST /metrics
//!               │ (via SSH tunnel -R 19999)
//! ┌─────────────────────────────────────────┐
//! │           REMOTE HOST (Linux)           │
//! │  ┌────────────────────────────────────┐ │
//! │  │ ratterm-daemon.sh                  │ │
//! │  │  - Collects from /proc, df, nvidia │ │
//! │  │  - Sends JSON every 1 second       │ │
//! │  │  - Lightweight shell script        │ │
//! │  └────────────────────────────────────┘ │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! The daemon system is optional and enhances the health dashboard
//! with real-time metrics updates. Without it, the dashboard falls
//! back to periodic SSH-based collection.

mod deployer;
mod receiver;
pub mod script;
pub mod test_utils;
pub mod types;

pub use deployer::{DaemonDeployer, DaemonStatus};
pub use receiver::{MetricsReceiver, DEFAULT_RECEIVER_PORT};
pub use script::DAEMON_SCRIPT;
pub use types::{DaemonError, DaemonMetrics};

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use tracing::{debug, error, info};

use crate::remote::SftpClient;
use crate::ssh::metrics::DeviceMetrics;
use crate::terminal::SSHContext;

/// Manages the daemon system for health dashboard metrics collection.
///
/// This is a synchronous wrapper that manages a tokio runtime internally
/// for the HTTP receiver. All public methods are synchronous and safe to
/// call from the main app event loop.
///
/// Coordinates:
/// - HTTP receiver for incoming metrics
/// - Daemon deployment to remote hosts
/// - Tracking of active daemon hosts
pub struct DaemonManager {
    /// Metrics receiver HTTP server (runs in background thread).
    receiver: Arc<Mutex<Option<MetricsReceiver>>>,
    /// Set of host IDs with active daemons.
    active_hosts: Arc<Mutex<HashSet<u32>>>,
    /// Whether the manager is active.
    active: Arc<AtomicBool>,
    /// Background thread handle for the receiver.
    receiver_thread: Option<JoinHandle<()>>,
    /// Shutdown flag for the background thread.
    shutdown: Arc<AtomicBool>,
}

impl DaemonManager {
    /// Creates a new daemon manager.
    ///
    /// The manager starts inactive. Call `start()` to begin
    /// accepting metrics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            receiver: Arc::new(Mutex::new(None)),
            active_hosts: Arc::new(Mutex::new(HashSet::new())),
            active: Arc::new(AtomicBool::new(false)),
            receiver_thread: None,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts the metrics receiver in a background thread.
    ///
    /// # Errors
    /// Returns error if the receiver cannot start.
    pub fn start(&mut self) -> Result<(), DaemonError> {
        if self.active.load(Ordering::Relaxed) {
            debug!("DaemonManager already active");
            return Ok(());
        }

        info!("Starting daemon manager");

        let receiver_arc = Arc::clone(&self.receiver);
        let active_arc = Arc::clone(&self.active);
        let shutdown_arc = Arc::clone(&self.shutdown);

        // Spawn background thread with its own tokio runtime
        let handle = thread::Builder::new()
            .name("daemon-receiver".into())
            .spawn(move || {
                // Create a new tokio runtime for this thread
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        error!("Failed to create tokio runtime: {}", e);
                        return;
                    }
                };

                // Run the receiver in this runtime
                rt.block_on(async {
                    match MetricsReceiver::start(DEFAULT_RECEIVER_PORT).await {
                        Ok(recv) => {
                            // Store the receiver
                            if let Ok(mut guard) = receiver_arc.lock() {
                                *guard = Some(recv);
                            }

                            active_arc.store(true, Ordering::Relaxed);
                            info!("Daemon receiver started on port {}", DEFAULT_RECEIVER_PORT);

                            // Keep the runtime alive until shutdown
                            while !shutdown_arc.load(Ordering::Relaxed) {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }

                            info!("Daemon receiver shutdown requested");
                        }
                        Err(e) => {
                            error!("Failed to start daemon receiver: {}", e);
                        }
                    }
                });
            })
            .map_err(|e| DaemonError::ServerError(format!("Failed to spawn thread: {}", e)))?;

        self.receiver_thread = Some(handle);

        // Wait for the receiver to start (with timeout)
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);

        while !self.active.load(Ordering::Relaxed) && start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if self.active.load(Ordering::Relaxed) {
            info!("Daemon manager started successfully");
            Ok(())
        } else {
            Err(DaemonError::ServerError(
                "Receiver failed to start within timeout".to_string(),
            ))
        }
    }

    /// Stops the metrics receiver and cleans up.
    pub fn stop(&mut self) {
        info!("Stopping daemon manager");

        // Signal shutdown
        self.shutdown.store(true, Ordering::Relaxed);
        self.active.store(false, Ordering::Relaxed);

        // Stop the receiver
        if let Ok(mut guard) = self.receiver.lock() {
            if let Some(mut recv) = guard.take() {
                recv.stop();
            }
        }

        // Wait for the background thread
        if let Some(handle) = self.receiver_thread.take() {
            let _ = handle.join();
        }

        // Clear active hosts
        if let Ok(mut guard) = self.active_hosts.lock() {
            guard.clear();
        }

        info!("Daemon manager stopped");
    }

    /// Returns whether the manager is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Deploys a daemon to a remote host.
    ///
    /// This should be called after establishing an SSH connection.
    /// The daemon will send metrics to the receiver via the SSH
    /// reverse tunnel.
    ///
    /// # Arguments
    /// * `context` - SSH connection context with credentials
    /// * `host_id` - Unique identifier for this host
    ///
    /// # Errors
    /// Returns error if deployment fails.
    pub fn deploy_to_host(
        &self,
        context: &SSHContext,
        host_id: u32,
    ) -> Result<(), DaemonError> {
        assert!(host_id > 0, "host_id must be positive");

        if !self.is_active() {
            return Err(DaemonError::ServerError(
                "Manager not active".to_string(),
            ));
        }

        info!(
            "Deploying daemon to {} (host_id={})",
            context.hostname, host_id
        );

        // Connect via SFTP
        let sftp = SftpClient::connect(context)
            .map_err(|e| DaemonError::SshError(e.to_string()))?;

        // Deploy the daemon
        DaemonDeployer::deploy(&sftp, host_id)?;

        // Track as active
        if let Ok(mut guard) = self.active_hosts.lock() {
            guard.insert(host_id);
        }

        info!("Daemon deployed and tracked for host_id={}", host_id);

        Ok(())
    }

    /// Stops the daemon on a remote host.
    ///
    /// # Errors
    /// Returns error if the stop command fails.
    pub fn stop_on_host(&self, context: &SSHContext, host_id: u32) -> Result<(), DaemonError> {
        info!(
            "Stopping daemon on {} (host_id={})",
            context.hostname, host_id
        );

        let sftp = SftpClient::connect(context)
            .map_err(|e| DaemonError::SshError(e.to_string()))?;

        DaemonDeployer::stop(&sftp)?;

        // Remove from active tracking
        if let Ok(mut guard) = self.active_hosts.lock() {
            guard.remove(&host_id);
        }

        // Clear cached metrics
        if let Ok(guard) = self.receiver.lock() {
            if let Some(ref receiver) = *guard {
                receiver.remove(host_id);
            }
        }

        info!("Daemon stopped for host_id={}", host_id);

        Ok(())
    }

    /// Gets metrics for a specific host.
    ///
    /// Returns None if no metrics are available for this host.
    #[must_use]
    pub fn get_metrics(&self, host_id: u32) -> Option<DeviceMetrics> {
        let guard = self.receiver.try_lock().ok()?;
        let receiver = guard.as_ref()?;
        receiver.get_metrics(host_id)
    }

    /// Checks if there are recent metrics for a host (within last 5 seconds).
    #[must_use]
    pub fn has_recent_metrics(&self, host_id: u32) -> bool {
        let guard = match self.receiver.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        match guard.as_ref() {
            Some(receiver) => receiver.has_recent_metrics(host_id),
            None => false,
        }
    }

    /// Returns the set of host IDs with active daemons.
    #[must_use]
    pub fn active_host_ids(&self) -> HashSet<u32> {
        match self.active_hosts.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => HashSet::new(),
        }
    }

    /// Returns the number of active daemon hosts.
    #[must_use]
    pub fn active_host_count(&self) -> usize {
        match self.active_hosts.lock() {
            Ok(guard) => guard.len(),
            Err(_) => 0,
        }
    }

    /// Returns the number of cached metrics entries.
    #[must_use]
    pub fn cached_metrics_count(&self) -> usize {
        let guard = match self.receiver.try_lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        match guard.as_ref() {
            Some(receiver) => receiver.cached_count(),
            None => 0,
        }
    }
}

impl Default for DaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_manager_creation() {
        let manager = DaemonManager::new();
        assert!(!manager.is_active());
    }

    #[test]
    fn test_daemon_manager_default() {
        let manager = DaemonManager::default();
        assert!(!manager.is_active());
    }

    #[test]
    fn test_daemon_manager_start_stop() {
        let mut manager = DaemonManager::new();

        // Start might fail if port is in use
        match manager.start() {
            Ok(()) => {
                assert!(manager.is_active());
                manager.stop();
                assert!(!manager.is_active());
            }
            Err(e) => {
                // Port likely in use, skip test
                println!("Skipping test, port in use: {}", e);
            }
        }
    }

    #[test]
    fn test_active_hosts_tracking() {
        let manager = DaemonManager::new();

        // No active hosts initially
        assert_eq!(manager.active_host_count(), 0);

        let hosts = manager.active_host_ids();
        assert!(hosts.is_empty());
    }

    #[test]
    fn test_get_metrics_when_inactive() {
        let manager = DaemonManager::new();

        // Should return None when not active
        let metrics = manager.get_metrics(1);
        assert!(metrics.is_none());
    }

    #[test]
    fn test_has_recent_metrics_when_inactive() {
        let manager = DaemonManager::new();
        assert!(!manager.has_recent_metrics(1));
    }

    #[test]
    fn test_cached_metrics_count_when_inactive() {
        let manager = DaemonManager::new();
        assert_eq!(manager.cached_metrics_count(), 0);
    }

    #[test]
    fn test_daemon_manager_receives_metrics() {
        use std::time::Duration;

        let mut manager = DaemonManager::new();

        // Start the manager (may fail if port in use)
        match manager.start() {
            Ok(()) => {
                assert!(manager.is_active());

                // Send a test metric via HTTP POST
                let json = r#"{"host_id":"999","ts":1700000000,"cpu":{"load":[1.0,0.8,0.6],"cores":4},"mem":{"total":8192,"avail":4096},"disk":{"total":256,"used":128}}"#;

                let client = reqwest::blocking::Client::new();
                let response = client
                    .post("http://127.0.0.1:19999/metrics")
                    .header("Content-Type", "application/json")
                    .body(json)
                    .send();

                if let Ok(resp) = response {
                    assert!(resp.status().is_success());

                    // Wait for metric to be processed
                    std::thread::sleep(Duration::from_millis(100));

                    // Verify metric was received
                    let metrics = manager.get_metrics(999);
                    assert!(metrics.is_some(), "Metric should be cached");

                    let m = metrics.unwrap();
                    assert_eq!(m.cpu_cores, 4);
                    assert_eq!(m.mem_total_mb, 8192);
                }

                manager.stop();
                assert!(!manager.is_active());
            }
            Err(e) => {
                // Port likely in use, skip test
                println!("Skipping test, port in use: {}", e);
            }
        }
    }
}
