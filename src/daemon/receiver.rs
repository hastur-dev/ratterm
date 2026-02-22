//! HTTP server for receiving daemon metrics.
//!
//! Runs a lightweight HTTP server on port 19999 that accepts
//! POST /metrics requests from remote daemons via SSH reverse tunnel.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use super::types::{DaemonError, DaemonMetrics};
use crate::ssh::metrics::DeviceMetrics;

/// Default port for the metrics receiver.
pub const DEFAULT_RECEIVER_PORT: u16 = 19999;

/// Maximum number of cached metrics entries.
const MAX_CACHED_ENTRIES: usize = 100;

/// Shared state for the receiver.
#[derive(Clone)]
struct ReceiverState {
    /// Cached metrics by host_id.
    metrics: Arc<Mutex<HashMap<String, DaemonMetrics>>>,
}

impl ReceiverState {
    fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// HTTP server that receives metrics from remote daemons.
pub struct MetricsReceiver {
    /// Cached metrics by host_id (string to handle JSON parsing).
    metrics: Arc<Mutex<HashMap<String, DaemonMetrics>>>,
    /// Shutdown signal sender.
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Port the server is running on.
    port: u16,
}

impl MetricsReceiver {
    /// Creates and starts a new metrics receiver on the specified port.
    ///
    /// # Errors
    /// Returns error if the server cannot bind to the port.
    pub async fn start(port: u16) -> Result<Self, DaemonError> {
        assert!(port > 0, "Port must be positive");

        let state = ReceiverState::new();
        let metrics = Arc::clone(&state.metrics);

        let app = Router::new()
            .route("/metrics", post(handle_metrics))
            .route("/health", get(handle_health))
            .with_state(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], port));

        let listener = TcpListener::bind(addr).await.map_err(|e| {
            DaemonError::ServerError(format!("Failed to bind port {}: {}", port, e))
        })?;

        info!("Metrics receiver listening on http://{}", addr);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Spawn the server task
        tokio::spawn(async move {
            let server = axum::serve(listener, app);

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        error!("Metrics receiver error: {}", e);
                    }
                }
                _ = shutdown_rx => {
                    info!("Metrics receiver shutting down");
                }
            }
        });

        Ok(Self {
            metrics,
            shutdown_tx: Some(shutdown_tx),
            port,
        })
    }

    /// Gets metrics for a specific host by ID (non-blocking).
    ///
    /// Returns `None` if the lock is contended or no metrics exist.
    #[must_use]
    pub fn get_metrics(&self, host_id: u32) -> Option<DeviceMetrics> {
        let guard = self.metrics.try_lock().ok()?;
        let host_id_str = host_id.to_string();
        guard.get(&host_id_str).map(|m| m.to_device_metrics())
    }

    /// Gets raw daemon metrics for a specific host (non-blocking).
    #[must_use]
    pub fn get_raw_metrics(&self, host_id: &str) -> Option<DaemonMetrics> {
        let guard = self.metrics.try_lock().ok()?;
        guard.get(host_id).cloned()
    }

    /// Gets all cached metrics (non-blocking).
    #[must_use]
    pub fn get_all_metrics(&self) -> HashMap<u32, DeviceMetrics> {
        let guard = match self.metrics.try_lock() {
            Ok(g) => g,
            Err(_) => return HashMap::new(),
        };

        guard
            .iter()
            .filter_map(|(id, m)| {
                let host_id = id.parse().ok()?;
                Some((host_id, m.to_device_metrics()))
            })
            .collect()
    }

    /// Returns the number of cached metrics entries (non-blocking).
    #[must_use]
    pub fn cached_count(&self) -> usize {
        self.metrics.try_lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Returns the port the receiver is running on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Clears all cached metrics.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.metrics.lock() {
            guard.clear();
        }
    }

    /// Removes metrics for a specific host.
    pub fn remove(&self, host_id: u32) {
        if let Ok(mut guard) = self.metrics.lock() {
            guard.remove(&host_id.to_string());
        }
    }

    /// Checks if there are recent metrics for a host (within last 5 seconds).
    #[must_use]
    pub fn has_recent_metrics(&self, host_id: u32) -> bool {
        if let Some(metrics) = self.get_metrics(host_id) {
            return !metrics.is_stale();
        }
        false
    }

    /// Stops the receiver.
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            info!("Sent shutdown signal to metrics receiver");
        }
    }
}

impl Drop for MetricsReceiver {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Handler for POST /metrics
async fn handle_metrics(
    State(state): State<ReceiverState>,
    body: String,
) -> (StatusCode, &'static str) {
    debug!("Received metrics POST, body length: {}", body.len());

    // Parse the JSON payload
    let metrics: DaemonMetrics = match serde_json::from_str(&body) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to parse metrics JSON: {}", e);
            debug!("Invalid JSON body: {}", body);
            return (StatusCode::BAD_REQUEST, "Invalid JSON");
        }
    };

    debug!(
        "Parsed metrics for host_id={}, cores={}, mem_total={}",
        metrics.host_id, metrics.cpu.cores, metrics.mem.total
    );

    // Store in cache
    if let Ok(mut guard) = state.metrics.lock() {
        // Prevent unbounded growth
        if guard.len() >= MAX_CACHED_ENTRIES && !guard.contains_key(&metrics.host_id) {
            // Remove oldest entry (simple approach: just remove first)
            if let Some(key) = guard.keys().next().cloned() {
                guard.remove(&key);
            }
        }

        guard.insert(metrics.host_id.clone(), metrics);
    } else {
        error!("Failed to acquire metrics lock");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Lock error");
    }

    (StatusCode::OK, "OK")
}

/// Handler for GET /health
async fn handle_health() -> &'static str {
    "OK"
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_receiver_state_creation() {
        let state = ReceiverState::new();
        let guard = state.metrics.lock().unwrap();
        assert!(guard.is_empty());
    }

    #[tokio::test]
    async fn test_receiver_start_and_stop() {
        // Use a random high port for testing
        let port = 19998;
        let receiver = MetricsReceiver::start(port).await;

        // If port is already in use, skip this test
        let receiver = match receiver {
            Ok(r) => r,
            Err(_) => return, // Port in use, skip test
        };

        assert_eq!(receiver.cached_count(), 0);
        assert_eq!(receiver.port(), port);
    }

    #[test]
    fn test_daemon_metrics_conversion() {
        let daemon_metrics = DaemonMetrics {
            host_id: "42".to_string(),
            ts: 1700000000,
            cpu: super::super::types::DaemonCpuMetrics {
                load: vec![1.5, 1.2, 1.0],
                cores: 8,
            },
            mem: super::super::types::DaemonMemMetrics {
                total: 32768,
                avail: 16384,
                swap_total: 8192,
                swap_used: 1024,
            },
            disk: super::super::types::DaemonDiskMetrics {
                total: 1000,
                used: 500,
            },
            gpu: None,
        };

        let device_metrics = daemon_metrics.to_device_metrics();
        assert_eq!(device_metrics.host_id, 42);
        assert_eq!(device_metrics.cpu_cores, 8);
        assert_eq!(device_metrics.mem_total_mb, 32768);
        assert_eq!(device_metrics.disk_total_gb, 1000);
    }
}
