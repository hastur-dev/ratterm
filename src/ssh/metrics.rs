//! Device metrics data structures for SSH health monitoring.
//!
//! Provides types for representing CPU, memory, disk, and GPU metrics
//! collected from remote SSH hosts.

use std::time::Instant;

/// Maximum number of hosts to monitor simultaneously.
pub const MAX_MONITORED_HOSTS: usize = 50;

/// Default refresh interval in milliseconds.
pub const DEFAULT_REFRESH_INTERVAL_MS: u64 = 1000;

/// SSH command timeout in seconds.
pub const SSH_COMMAND_TIMEOUT_SECS: u64 = 5;

/// Maximum concurrent SSH connections for metric collection.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 5;

/// Status of metric collection for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricStatus {
    /// Status not yet determined.
    #[default]
    Unknown,
    /// Currently collecting metrics.
    Collecting,
    /// Device is online and responding.
    Online,
    /// Device is offline or unreachable.
    Offline,
    /// Error occurred during collection.
    Error,
}

impl MetricStatus {
    /// Returns true if the device is reachable.
    #[must_use]
    pub fn is_online(&self) -> bool {
        matches!(self, Self::Online)
    }

    /// Returns true if actively collecting.
    #[must_use]
    pub fn is_collecting(&self) -> bool {
        matches!(self, Self::Collecting)
    }

    /// Returns a display string for this status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Collecting => "COLLECTING",
            Self::Online => "ONLINE",
            Self::Offline => "OFFLINE",
            Self::Error => "ERROR",
        }
    }
}

/// Type of GPU detected on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuType {
    /// No GPU or GPU not detected.
    #[default]
    None,
    /// NVIDIA GPU (detected via nvidia-smi).
    Nvidia,
    /// AMD GPU (detected via rocm-smi).
    Amd,
}

impl GpuType {
    /// Returns a display string for this GPU type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
        }
    }
}

/// GPU metrics collected from a device.
#[derive(Debug, Clone, Default)]
pub struct GpuMetrics {
    /// Type of GPU.
    pub gpu_type: GpuType,
    /// GPU model name.
    pub name: String,
    /// GPU utilization percentage (0-100).
    pub usage_percent: f32,
    /// GPU memory used in MB.
    pub memory_used_mb: u64,
    /// Total GPU memory in MB.
    pub memory_total_mb: u64,
    /// GPU temperature in Celsius (if available).
    pub temperature_celsius: Option<f32>,
}

impl GpuMetrics {
    /// Creates new GPU metrics.
    #[must_use]
    pub fn new(gpu_type: GpuType, name: String) -> Self {
        assert!(
            !name.is_empty() || gpu_type == GpuType::None,
            "GPU name required for detected GPU"
        );
        Self {
            gpu_type,
            name,
            usage_percent: 0.0,
            memory_used_mb: 0,
            memory_total_mb: 0,
            temperature_celsius: None,
        }
    }

    /// Creates metrics for no GPU.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Returns the memory usage percentage.
    #[must_use]
    pub fn memory_percent(&self) -> f32 {
        if self.memory_total_mb == 0 {
            return 0.0;
        }
        let percent = (self.memory_used_mb as f32 / self.memory_total_mb as f32) * 100.0;
        assert!(percent >= 0.0, "Memory percent cannot be negative");
        percent.clamp(0.0, 100.0)
    }

    /// Returns true if GPU was detected.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.gpu_type != GpuType::None
    }
}

/// Complete device metrics collected from an SSH host.
#[derive(Debug, Clone)]
pub struct DeviceMetrics {
    /// Host ID from SSHHostList.
    pub host_id: u32,
    /// Timestamp when metrics were collected.
    pub timestamp: Instant,
    /// Current collection status.
    pub status: MetricStatus,

    // CPU metrics
    /// CPU usage percentage (0-100).
    pub cpu_usage_percent: f32,
    /// Number of CPU cores.
    pub cpu_cores: u16,
    /// Load average (1 minute, 5 minutes, 15 minutes).
    pub load_avg: (f32, f32, f32),

    // Memory metrics
    /// Total memory in MB.
    pub mem_total_mb: u64,
    /// Used memory in MB.
    pub mem_used_mb: u64,
    /// Available memory in MB.
    pub mem_available_mb: u64,
    /// Total swap in MB.
    pub swap_total_mb: u64,
    /// Used swap in MB.
    pub swap_used_mb: u64,

    // Disk metrics
    /// Total disk space in GB (root filesystem).
    pub disk_total_gb: u64,
    /// Used disk space in GB.
    pub disk_used_gb: u64,

    // GPU metrics
    /// GPU metrics (None if no GPU detected).
    pub gpu: Option<GpuMetrics>,

    /// Error message if collection failed.
    pub error: Option<String>,
}

impl DeviceMetrics {
    /// Creates new device metrics with unknown status.
    #[must_use]
    pub fn new(host_id: u32) -> Self {
        Self {
            host_id,
            timestamp: Instant::now(),
            status: MetricStatus::Unknown,
            cpu_usage_percent: 0.0,
            cpu_cores: 0,
            load_avg: (0.0, 0.0, 0.0),
            mem_total_mb: 0,
            mem_used_mb: 0,
            mem_available_mb: 0,
            swap_total_mb: 0,
            swap_used_mb: 0,
            disk_total_gb: 0,
            disk_used_gb: 0,
            gpu: None,
            error: None,
        }
    }

    /// Creates metrics indicating the device is offline.
    #[must_use]
    pub fn offline(host_id: u32) -> Self {
        let mut metrics = Self::new(host_id);
        metrics.status = MetricStatus::Offline;
        metrics
    }

    /// Creates metrics indicating an error occurred.
    #[must_use]
    pub fn with_error(host_id: u32, error: String) -> Self {
        assert!(!error.is_empty(), "Error message cannot be empty");
        let mut metrics = Self::new(host_id);
        metrics.status = MetricStatus::Error;
        metrics.error = Some(error);
        metrics
    }

    /// Creates metrics indicating collection is in progress.
    #[must_use]
    pub fn collecting(host_id: u32) -> Self {
        let mut metrics = Self::new(host_id);
        metrics.status = MetricStatus::Collecting;
        metrics
    }

    /// Returns the memory usage percentage.
    #[must_use]
    pub fn memory_percent(&self) -> f32 {
        if self.mem_total_mb == 0 {
            return 0.0;
        }
        let percent = (self.mem_used_mb as f32 / self.mem_total_mb as f32) * 100.0;
        percent.clamp(0.0, 100.0)
    }

    /// Returns the disk usage percentage.
    #[must_use]
    pub fn disk_percent(&self) -> f32 {
        if self.disk_total_gb == 0 {
            return 0.0;
        }
        let percent = (self.disk_used_gb as f32 / self.disk_total_gb as f32) * 100.0;
        percent.clamp(0.0, 100.0)
    }

    /// Returns the swap usage percentage.
    #[must_use]
    pub fn swap_percent(&self) -> f32 {
        if self.swap_total_mb == 0 {
            return 0.0;
        }
        let percent = (self.swap_used_mb as f32 / self.swap_total_mb as f32) * 100.0;
        percent.clamp(0.0, 100.0)
    }

    /// Returns the age of these metrics in seconds.
    #[must_use]
    pub fn age_secs(&self) -> u64 {
        self.timestamp.elapsed().as_secs()
    }

    /// Returns true if metrics are stale (older than 5 seconds).
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.age_secs() > 5
    }

    /// Updates the timestamp to now.
    pub fn refresh_timestamp(&mut self) {
        self.timestamp = Instant::now();
    }

    /// Sets the status to online and updates timestamp.
    pub fn mark_online(&mut self) {
        self.status = MetricStatus::Online;
        self.error = None;
        self.refresh_timestamp();
    }

    /// Returns true if this device has GPU metrics.
    #[must_use]
    pub fn has_gpu(&self) -> bool {
        self.gpu.as_ref().is_some_and(|g| g.is_available())
    }
}

impl Default for DeviceMetrics {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_status_display() {
        assert_eq!(MetricStatus::Online.as_str(), "ONLINE");
        assert_eq!(MetricStatus::Offline.as_str(), "OFFLINE");
        assert!(MetricStatus::Online.is_online());
        assert!(!MetricStatus::Offline.is_online());
    }

    #[test]
    fn test_device_metrics_percentages() {
        let mut metrics = DeviceMetrics::new(1);
        metrics.mem_total_mb = 1000;
        metrics.mem_used_mb = 500;
        assert!((metrics.memory_percent() - 50.0).abs() < 0.01);

        metrics.disk_total_gb = 100;
        metrics.disk_used_gb = 75;
        assert!((metrics.disk_percent() - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_device_metrics_zero_total() {
        let metrics = DeviceMetrics::new(1);
        assert_eq!(metrics.memory_percent(), 0.0);
        assert_eq!(metrics.disk_percent(), 0.0);
        assert_eq!(metrics.swap_percent(), 0.0);
    }

    #[test]
    fn test_gpu_metrics() {
        let gpu = GpuMetrics::new(GpuType::Nvidia, "RTX 3060".to_string());
        assert!(gpu.is_available());
        assert_eq!(gpu.gpu_type, GpuType::Nvidia);

        let no_gpu = GpuMetrics::none();
        assert!(!no_gpu.is_available());
    }

    #[test]
    fn test_device_metrics_offline() {
        let metrics = DeviceMetrics::offline(5);
        assert_eq!(metrics.host_id, 5);
        assert_eq!(metrics.status, MetricStatus::Offline);
    }

    #[test]
    fn test_device_metrics_with_error() {
        let metrics = DeviceMetrics::with_error(3, "Connection refused".to_string());
        assert_eq!(metrics.status, MetricStatus::Error);
        assert_eq!(metrics.error.as_deref(), Some("Connection refused"));
    }
}
