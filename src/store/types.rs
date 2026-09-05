//! Row structs and the metric vocabulary shared by every part of the store.
//!
//! Timestamps are Unix seconds in UTC everywhere, stored as `i64`. Byte counts
//! are `i64` rather than `u64` because SQLite has no unsigned integer type and
//! a silent wrap at 2^63 is a better failure than a silent reinterpretation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Resolution marker written into `metric_samples.resolution`.
///
/// Downsampling reads this to decide what a row already is, so a row that has
/// been collapsed into a minute average is never collapsed a second time.
pub const RESOLUTION_RAW: i64 = 0;
/// One-minute average rows.
pub const RESOLUTION_MINUTE: i64 = 60;
/// One-hour average rows.
pub const RESOLUTION_HOUR: i64 = 3600;

/// A host the store has seen at least one sample or event from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRow {
    /// Numeric key used by every other table.
    pub host_id: i64,
    /// Human-facing name. `None` for a host that was only ever seen by id,
    /// which happens when the push daemon reports before the host list loads.
    pub label: Option<String>,
    /// First time anything was recorded for this host.
    pub first_seen: i64,
    /// Most recent time anything was recorded for this host.
    pub last_seen: i64,
}

/// One point in the metric time series.
///
/// Every metric is optional because hosts differ: a Raspberry Pi reports no
/// GPU, a container host reports no temperature, and a first poll after boot
/// may have uptime but nothing else yet. Storing `NULL` keeps "not reported"
/// distinct from "reported as zero", which matters for averages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    /// Host this sample belongs to.
    pub host_id: i64,
    /// Unix seconds, UTC.
    pub ts: i64,
    /// [`RESOLUTION_RAW`] for ingested samples; set by downsampling otherwise.
    pub resolution: i64,
    /// One-minute load average.
    pub cpu_load1: Option<f64>,
    /// CPU busy percentage, 0-100.
    pub cpu_percent: Option<f64>,
    /// Memory in use, bytes.
    pub mem_used_bytes: Option<i64>,
    /// Total memory, bytes.
    pub mem_total_bytes: Option<i64>,
    /// Disk in use on the root filesystem, bytes.
    pub disk_used_bytes: Option<i64>,
    /// Total disk on the root filesystem, bytes.
    pub disk_total_bytes: Option<i64>,
    /// GPU utilisation percentage, 0-100.
    pub gpu_util_percent: Option<f64>,
    /// GPU memory in use, bytes.
    pub gpu_mem_used_bytes: Option<i64>,
    /// Hottest reported sensor, degrees Celsius.
    pub temperature_c: Option<f64>,
    /// Seconds since boot.
    pub uptime_secs: Option<i64>,
}

impl MetricSample {
    /// A raw sample for `host_id` at `ts` with no metrics filled in yet.
    ///
    /// Collectors build a sample this way and set only the fields the host
    /// actually reported.
    pub fn new(host_id: i64, ts: i64) -> Self {
        Self {
            host_id,
            ts,
            resolution: RESOLUTION_RAW,
            cpu_load1: None,
            cpu_percent: None,
            mem_used_bytes: None,
            mem_total_bytes: None,
            disk_used_bytes: None,
            disk_total_bytes: None,
            gpu_util_percent: None,
            gpu_mem_used_bytes: None,
            temperature_c: None,
            uptime_secs: None,
        }
    }

    /// Memory used as a percentage of total, when both are known and total is
    /// positive.
    pub fn mem_used_percent(&self) -> Option<f64> {
        percent_of(self.mem_used_bytes, self.mem_total_bytes)
    }

    /// Disk used as a percentage of total, when both are known and total is
    /// positive.
    pub fn disk_used_percent(&self) -> Option<f64> {
        percent_of(self.disk_used_bytes, self.disk_total_bytes)
    }
}

fn percent_of(used: Option<i64>, total: Option<i64>) -> Option<f64> {
    match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some((used as f64 * 100.0) / total as f64),
        _ => None,
    }
}

/// Every metric the store can aggregate or chart.
///
/// The two `*_percent` variants that are not stored columns are derived in SQL
/// from the byte counts, so a caller can chart "memory used %" without the
/// collector having to compute and store it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// One-minute load average.
    CpuLoad1,
    /// CPU busy percentage.
    CpuPercent,
    /// Memory in use, bytes.
    MemUsedBytes,
    /// Total memory, bytes.
    MemTotalBytes,
    /// Memory in use as a percentage of total. Derived.
    MemUsedPercent,
    /// Disk in use, bytes.
    DiskUsedBytes,
    /// Total disk, bytes.
    DiskTotalBytes,
    /// Disk in use as a percentage of total. Derived.
    DiskUsedPercent,
    /// GPU utilisation percentage.
    GpuUtilPercent,
    /// GPU memory in use, bytes.
    GpuMemUsedBytes,
    /// Temperature in degrees Celsius.
    TemperatureC,
    /// Seconds since boot.
    UptimeSecs,
}

impl MetricKind {
    /// Every metric, in a stable order. Summary queries iterate this, so the
    /// order also fixes the column layout of the generated SQL.
    pub const ALL: [MetricKind; 12] = [
        MetricKind::CpuLoad1,
        MetricKind::CpuPercent,
        MetricKind::MemUsedBytes,
        MetricKind::MemTotalBytes,
        MetricKind::MemUsedPercent,
        MetricKind::DiskUsedBytes,
        MetricKind::DiskTotalBytes,
        MetricKind::DiskUsedPercent,
        MetricKind::GpuUtilPercent,
        MetricKind::GpuMemUsedBytes,
        MetricKind::TemperatureC,
        MetricKind::UptimeSecs,
    ];

    /// SQL expression yielding this metric from a `metric_samples` row.
    ///
    /// Returned from a closed enum rather than built from caller input, so
    /// splicing it into a statement cannot inject SQL.
    pub fn sql_expr(self) -> &'static str {
        match self {
            MetricKind::CpuLoad1 => "cpu_load1",
            MetricKind::CpuPercent => "cpu_percent",
            MetricKind::MemUsedBytes => "mem_used_bytes",
            MetricKind::MemTotalBytes => "mem_total_bytes",
            MetricKind::MemUsedPercent => {
                "CASE WHEN mem_total_bytes > 0 \
                 THEN (mem_used_bytes * 100.0) / mem_total_bytes END"
            }
            MetricKind::DiskUsedBytes => "disk_used_bytes",
            MetricKind::DiskTotalBytes => "disk_total_bytes",
            MetricKind::DiskUsedPercent => {
                "CASE WHEN disk_total_bytes > 0 \
                 THEN (disk_used_bytes * 100.0) / disk_total_bytes END"
            }
            MetricKind::GpuUtilPercent => "gpu_util_percent",
            MetricKind::GpuMemUsedBytes => "gpu_mem_used_bytes",
            MetricKind::TemperatureC => "temperature_c",
            MetricKind::UptimeSecs => "uptime_secs",
        }
    }

    /// Stable snake_case name, used in alert rule keys and log lines.
    pub fn as_str(self) -> &'static str {
        match self {
            MetricKind::CpuLoad1 => "cpu_load1",
            MetricKind::CpuPercent => "cpu_percent",
            MetricKind::MemUsedBytes => "mem_used_bytes",
            MetricKind::MemTotalBytes => "mem_total_bytes",
            MetricKind::MemUsedPercent => "mem_used_percent",
            MetricKind::DiskUsedBytes => "disk_used_bytes",
            MetricKind::DiskTotalBytes => "disk_total_bytes",
            MetricKind::DiskUsedPercent => "disk_used_percent",
            MetricKind::GpuUtilPercent => "gpu_util_percent",
            MetricKind::GpuMemUsedBytes => "gpu_mem_used_bytes",
            MetricKind::TemperatureC => "temperature_c",
            MetricKind::UptimeSecs => "uptime_secs",
        }
    }

    /// Reads this metric out of an in-memory sample, including the derived
    /// percentages. Used by alert evaluation, which runs before the sample is
    /// written and so cannot go through SQL.
    pub fn value_of(self, sample: &MetricSample) -> Option<f64> {
        match self {
            MetricKind::CpuLoad1 => sample.cpu_load1,
            MetricKind::CpuPercent => sample.cpu_percent,
            MetricKind::MemUsedBytes => sample.mem_used_bytes.map(|v| v as f64),
            MetricKind::MemTotalBytes => sample.mem_total_bytes.map(|v| v as f64),
            MetricKind::MemUsedPercent => sample.mem_used_percent(),
            MetricKind::DiskUsedBytes => sample.disk_used_bytes.map(|v| v as f64),
            MetricKind::DiskTotalBytes => sample.disk_total_bytes.map(|v| v as f64),
            MetricKind::DiskUsedPercent => sample.disk_used_percent(),
            MetricKind::GpuUtilPercent => sample.gpu_util_percent,
            MetricKind::GpuMemUsedBytes => sample.gpu_mem_used_bytes.map(|v| v as f64),
            MetricKind::TemperatureC => sample.temperature_c,
            MetricKind::UptimeSecs => sample.uptime_secs.map(|v| v as f64),
        }
    }
}

/// Minimum, maximum and mean of one metric over a window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetricStats {
    /// Smallest non-null value in the window.
    pub min: f64,
    /// Largest non-null value in the window.
    pub max: f64,
    /// Mean of the non-null values.
    pub avg: f64,
    /// How many rows carried a value for this metric.
    pub count: u64,
}

/// Aggregate view of one host over a time window.
///
/// A metric with no non-null rows in the window is absent from `metrics`
/// rather than present with zeroes, so a dashboard can tell "the host does not
/// report a GPU" from "the GPU was idle".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    /// Rows in the window, whatever their resolution.
    pub sample_count: u64,
    /// Per-metric statistics, for metrics that had at least one value.
    pub metrics: BTreeMap<MetricKind, MetricStats>,
}

impl MetricSummary {
    /// Statistics for one metric, if the window contained any.
    pub fn stats(&self, metric: MetricKind) -> Option<&MetricStats> {
        self.metrics.get(&metric)
    }
}

/// A Docker lifecycle event observed on a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerEvent {
    /// Row id, `None` before the row is written.
    pub event_id: Option<i64>,
    /// Host the container runs on.
    pub host_id: i64,
    /// Full or short container id, as reported.
    pub container_id: String,
    /// Container name, when the runtime gave one.
    pub container_name: Option<String>,
    /// Unix seconds, UTC.
    pub ts: i64,
    /// Event name, for example `start`, `die`, `oom`.
    pub event: String,
    /// Free-form extra context, for example an exit code.
    pub detail: Option<String>,
}

/// A Kubernetes event, keyed by cluster context rather than host because a
/// cluster is not tied to one machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct K8sEvent {
    /// Row id, `None` before the row is written.
    pub event_id: Option<i64>,
    /// Kubeconfig context name.
    pub context: String,
    /// Namespace of the involved object.
    pub namespace: String,
    /// Kind of the involved object, for example `Pod`.
    pub kind: String,
    /// Name of the involved object.
    pub name: String,
    /// Unix seconds, UTC.
    pub ts: i64,
    /// Event reason, for example `BackOff`.
    pub reason: Option<String>,
    /// Human-readable message.
    pub message: Option<String>,
}

/// One run of the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Row id.
    pub id: i64,
    /// When the session started, Unix seconds.
    pub started: i64,
    /// When it ended, or `None` while it is still running or if it crashed.
    pub ended: Option<i64>,
    /// Host ids touched during the session.
    pub hosts_touched: Vec<i64>,
}

/// A Docker log file on disk, recorded so a multi-day search can skip files
/// whose date is out of range instead of decompressing all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogIndexEntry {
    /// Row id, `None` before the row is written.
    pub entry_id: Option<i64>,
    /// Host the log came from.
    pub host_id: i64,
    /// Container the log belongs to.
    pub container_id: String,
    /// `YYYY-MM-DD` in UTC. Stored as text so lexicographic comparison is also
    /// chronological comparison.
    pub date: String,
    /// Path to the file, absolute on the machine that holds it.
    pub path: String,
    /// Lines in the file.
    pub line_count: i64,
    /// Size of the file on disk, bytes.
    pub byte_size: i64,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_sample_is_raw_and_empty() {
        let sample = MetricSample::new(3, 1_700_000_000);
        assert_eq!(sample.host_id, 3);
        assert_eq!(sample.ts, 1_700_000_000);
        assert_eq!(sample.resolution, RESOLUTION_RAW);
        assert!(sample.cpu_load1.is_none());
        assert!(sample.uptime_secs.is_none());
    }

    #[test]
    fn derived_percentages_need_both_halves() {
        let mut sample = MetricSample::new(1, 0);
        assert_eq!(sample.mem_used_percent(), None);
        sample.mem_used_bytes = Some(512);
        assert_eq!(sample.mem_used_percent(), None);
        sample.mem_total_bytes = Some(2048);
        assert_eq!(sample.mem_used_percent(), Some(25.0));
    }

    #[test]
    fn zero_total_does_not_divide_by_zero() {
        let mut sample = MetricSample::new(1, 0);
        sample.disk_used_bytes = Some(10);
        sample.disk_total_bytes = Some(0);
        assert_eq!(sample.disk_used_percent(), None);
    }

    #[test]
    fn all_metrics_have_distinct_names_and_exprs() {
        let mut names: Vec<&str> = MetricKind::ALL.iter().map(|m| m.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "metric names must be unique");
        assert_eq!(count, MetricKind::ALL.len());
    }

    #[test]
    fn value_of_matches_the_sample_fields() {
        let mut sample = MetricSample::new(1, 0);
        sample.cpu_percent = Some(42.5);
        sample.mem_used_bytes = Some(100);
        sample.mem_total_bytes = Some(400);
        assert_eq!(MetricKind::CpuPercent.value_of(&sample), Some(42.5));
        assert_eq!(MetricKind::MemUsedBytes.value_of(&sample), Some(100.0));
        assert_eq!(MetricKind::MemUsedPercent.value_of(&sample), Some(25.0));
        assert_eq!(MetricKind::TemperatureC.value_of(&sample), None);
    }

    #[test]
    fn summary_stats_lookup_returns_none_for_absent_metric() {
        let mut summary = MetricSummary::default();
        summary.metrics.insert(
            MetricKind::CpuPercent,
            MetricStats {
                min: 1.0,
                max: 2.0,
                avg: 1.5,
                count: 2,
            },
        );
        assert!(summary.stats(MetricKind::CpuPercent).is_some());
        assert!(summary.stats(MetricKind::TemperatureC).is_none());
    }
}
