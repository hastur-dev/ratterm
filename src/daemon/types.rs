//! Types for the daemon metrics collection system.
//!
//! Defines the JSON format for daemon→receiver communication
//! and error types for daemon operations.

use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::ssh::metrics::{DeviceMetrics, GpuMetrics, GpuType, MetricStatus};

/// Metrics payload sent by the daemon to the receiver.
///
/// This is the JSON format that the shell script produces and
/// the receiver parses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonMetrics {
    /// Host identifier (matches SSHHost.id).
    pub host_id: String,
    /// Unix timestamp when metrics were collected.
    pub ts: u64,
    /// CPU metrics.
    pub cpu: DaemonCpuMetrics,
    /// Memory metrics.
    pub mem: DaemonMemMetrics,
    /// Disk metrics.
    pub disk: DaemonDiskMetrics,
    /// GPU metrics (optional).
    #[serde(default)]
    pub gpu: Option<DaemonGpuMetrics>,
}

/// CPU metrics from daemon.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonCpuMetrics {
    /// Load averages [1m, 5m, 15m].
    pub load: Vec<f32>,
    /// Number of CPU cores.
    pub cores: u16,
}

/// Memory metrics from daemon (in MB).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonMemMetrics {
    /// Total memory in MB.
    pub total: u64,
    /// Available memory in MB.
    pub avail: u64,
    /// Swap total in MB (optional).
    #[serde(default)]
    pub swap_total: u64,
    /// Swap used in MB (optional).
    #[serde(default)]
    pub swap_used: u64,
}

/// Disk metrics from daemon.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonDiskMetrics {
    /// Total disk space in GB.
    pub total: u64,
    /// Used disk space in GB.
    pub used: u64,
}

/// GPU metrics from daemon (optional).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonGpuMetrics {
    /// GPU type: "nvidia", "amd", or "none".
    #[serde(default)]
    pub gpu_type: String,
    /// GPU model name.
    #[serde(default)]
    pub name: String,
    /// GPU utilization percentage (0-100).
    #[serde(default)]
    pub usage: f32,
    /// GPU memory used in MB.
    #[serde(default)]
    pub mem_used: u64,
    /// GPU memory total in MB.
    #[serde(default)]
    pub mem_total: u64,
    /// GPU temperature in Celsius.
    #[serde(default)]
    pub temp: Option<f32>,
}

impl DaemonMetrics {
    /// Converts daemon metrics to the internal DeviceMetrics format.
    #[must_use]
    pub fn to_device_metrics(&self) -> DeviceMetrics {
        let host_id = self.host_id.parse().unwrap_or(0);
        let mut metrics = DeviceMetrics::new(host_id);

        // CPU metrics
        metrics.cpu_cores = self.cpu.cores;
        if self.cpu.load.len() >= 3 {
            metrics.load_avg = (self.cpu.load[0], self.cpu.load[1], self.cpu.load[2]);
        } else if !self.cpu.load.is_empty() {
            metrics.load_avg = (
                self.cpu.load.first().copied().unwrap_or(0.0),
                self.cpu.load.get(1).copied().unwrap_or(0.0),
                self.cpu.load.get(2).copied().unwrap_or(0.0),
            );
        }

        // Calculate CPU usage from load average
        if metrics.cpu_cores > 0 {
            let usage = (metrics.load_avg.0 / metrics.cpu_cores as f32) * 100.0;
            metrics.cpu_usage_percent = usage.clamp(0.0, 100.0);
        }

        // Memory metrics
        metrics.mem_total_mb = self.mem.total;
        metrics.mem_available_mb = self.mem.avail;
        metrics.mem_used_mb = self.mem.total.saturating_sub(self.mem.avail);
        metrics.swap_total_mb = self.mem.swap_total;
        metrics.swap_used_mb = self.mem.swap_used;

        // Disk metrics
        metrics.disk_total_gb = self.disk.total;
        metrics.disk_used_gb = self.disk.used;

        // GPU metrics
        if let Some(ref gpu) = self.gpu {
            let gpu_type = match gpu.gpu_type.to_lowercase().as_str() {
                "nvidia" => GpuType::Nvidia,
                "amd" => GpuType::Amd,
                "videocore" => GpuType::Nvidia, // Treat VideoCore as Nvidia for display purposes
                _ => GpuType::None,
            };

            // Include GPU even if type is "videocore" (Raspberry Pi)
            let include_gpu =
                gpu_type != GpuType::None || gpu.gpu_type.to_lowercase() == "videocore";

            if include_gpu {
                let display_type = if gpu.gpu_type.to_lowercase() == "videocore" {
                    GpuType::Nvidia // Use Nvidia type for display, name will show "VideoCore"
                } else {
                    gpu_type
                };
                let mut gpu_metrics = GpuMetrics::new(display_type, gpu.name.clone());
                gpu_metrics.usage_percent = gpu.usage;
                gpu_metrics.memory_used_mb = gpu.mem_used;
                gpu_metrics.memory_total_mb = gpu.mem_total;
                gpu_metrics.temperature_celsius = gpu.temp;
                metrics.gpu = Some(gpu_metrics);
            }
        }

        metrics.status = MetricStatus::Online;
        metrics.timestamp = Instant::now();

        metrics
    }

    /// Creates a sample metrics payload for testing.
    #[cfg(test)]
    #[must_use]
    pub fn sample() -> Self {
        Self {
            host_id: "1".to_string(),
            ts: 1700000000,
            cpu: DaemonCpuMetrics {
                load: vec![0.5, 0.6, 0.7],
                cores: 8,
            },
            mem: DaemonMemMetrics {
                total: 16384,
                avail: 8192,
                swap_total: 4096,
                swap_used: 512,
            },
            disk: DaemonDiskMetrics {
                total: 500,
                used: 250,
            },
            gpu: Some(DaemonGpuMetrics {
                gpu_type: "nvidia".to_string(),
                name: "RTX 3060".to_string(),
                usage: 45.0,
                mem_used: 4096,
                mem_total: 12288,
                temp: Some(55.0),
            }),
        }
    }
}

/// Error type for daemon operations.
#[derive(Debug, Clone)]
pub enum DaemonError {
    /// Failed to deploy daemon to remote host.
    DeployFailed(String),
    /// Failed to stop daemon on remote host.
    StopFailed(String),
    /// Failed to check daemon status.
    StatusCheckFailed(String),
    /// Daemon is not running.
    NotRunning,
    /// Invalid metrics payload.
    InvalidMetrics(String),
    /// HTTP server error.
    ServerError(String),
    /// SSH connection error.
    SshError(String),
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeployFailed(msg) => write!(f, "Deploy failed: {}", msg),
            Self::StopFailed(msg) => write!(f, "Stop failed: {}", msg),
            Self::StatusCheckFailed(msg) => write!(f, "Status check failed: {}", msg),
            Self::NotRunning => write!(f, "Daemon not running"),
            Self::InvalidMetrics(msg) => write!(f, "Invalid metrics: {}", msg),
            Self::ServerError(msg) => write!(f, "Server error: {}", msg),
            Self::SshError(msg) => write!(f, "SSH error: {}", msg),
        }
    }
}

impl std::error::Error for DaemonError {}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_metrics_json_roundtrip() {
        let metrics = DaemonMetrics::sample();
        let json = serde_json::to_string(&metrics).expect("Failed to serialize");
        let parsed: DaemonMetrics = serde_json::from_str(&json).expect("Failed to parse");

        assert_eq!(parsed.host_id, "1");
        assert_eq!(parsed.cpu.cores, 8);
        assert_eq!(parsed.mem.total, 16384);
        assert_eq!(parsed.disk.total, 500);
        assert!(parsed.gpu.is_some());
    }

    #[test]
    fn test_daemon_metrics_to_device_metrics() {
        let daemon_metrics = DaemonMetrics::sample();
        let device_metrics = daemon_metrics.to_device_metrics();

        assert_eq!(device_metrics.host_id, 1);
        assert_eq!(device_metrics.cpu_cores, 8);
        assert!((device_metrics.load_avg.0 - 0.5).abs() < 0.01);
        assert_eq!(device_metrics.mem_total_mb, 16384);
        assert_eq!(device_metrics.disk_total_gb, 500);
        assert!(device_metrics.gpu.is_some());

        let gpu = device_metrics.gpu.unwrap();
        assert_eq!(gpu.gpu_type, GpuType::Nvidia);
        assert!((gpu.usage_percent - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_daemon_metrics_minimal_json() {
        let json = r#"{"host_id":"2","ts":1700000000,"cpu":{"load":[1.0],"cores":4},"mem":{"total":8192,"avail":4096},"disk":{"total":256,"used":128}}"#;
        let metrics: DaemonMetrics = serde_json::from_str(json).expect("Failed to parse");

        assert_eq!(metrics.host_id, "2");
        assert_eq!(metrics.cpu.cores, 4);
        assert!(metrics.gpu.is_none());
    }

    #[test]
    fn test_daemon_error_display() {
        let err = DaemonError::DeployFailed("connection timeout".to_string());
        assert!(err.to_string().contains("connection timeout"));

        let err = DaemonError::NotRunning;
        assert!(err.to_string().contains("not running"));
    }

    #[test]
    fn test_cpu_usage_calculation() {
        let metrics = DaemonMetrics {
            host_id: "1".to_string(),
            ts: 0,
            cpu: DaemonCpuMetrics {
                load: vec![4.0, 3.0, 2.0],
                cores: 4,
            },
            mem: DaemonMemMetrics::default(),
            disk: DaemonDiskMetrics::default(),
            gpu: None,
        };

        let device = metrics.to_device_metrics();
        // load 4.0 on 4 cores = 100% CPU
        assert!((device.cpu_usage_percent - 100.0).abs() < 0.01);
    }
}
