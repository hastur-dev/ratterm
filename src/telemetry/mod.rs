//! One ingest path for fleet metrics.
//!
//! There were two collectors producing the same `DeviceMetrics` by different
//! transports — the SSH poller that reaches out to hosts, and the push daemon
//! that reports in — toggled independently, each keeping only the latest
//! sample in a `HashMap`. Nothing was persisted, so a restart lost the
//! history and no question about the past could be answered.
//!
//! Both now write through [`Telemetry::ingest`]. It keeps the latest sample in
//! memory for the dashboard's live view, writes every sample to the durable
//! store, evaluates the alert rules, and downsamples in the background.
//!
//! The store is optional. If the database cannot be opened — a read-only home
//! directory, a corrupt file — the application keeps working with the live
//! view alone and says so once, rather than refusing to show metrics at all.

pub mod agent;
pub mod alerts;

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

pub use alerts::AlertSettings;

use crate::ssh::metrics::{DeviceMetrics, MetricStatus};
use crate::store::{
    AlertRecord, AlertRule, MetricKind, MetricSample, MetricStore, MetricSummary, RetentionPolicy,
    StoreError,
};

/// How often downsampling runs while the application is up.
///
/// Downsampling is cheap on a small database and there is no reason to do it
/// more often than an hour; a session shorter than that is handled the next
/// time the application starts.
pub const DOWNSAMPLE_INTERVAL: Duration = Duration::from_secs(3600);

/// How many alert records the dashboard keeps to hand.
const RECENT_ALERT_LIMIT: usize = 32;

/// The eight block heights a sparkline is drawn from, shortest first.
///
/// A gap — a bucket with no sample — is a space rather than the lowest block,
/// so "the host was quiet" and "the host was not there" do not look the same.
const SPARK_LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// The character used where a bucket has no data.
const SPARK_GAP: char = ' ';

/// One host's past, ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct HostHistory {
    /// False when there is no database, so the view can say why it is empty.
    pub durable: bool,
    /// The window the series and the summary cover.
    pub window: Duration,
    /// CPU percentage per bucket, oldest first. Empty means no history.
    pub cpu: Vec<Option<f64>>,
    /// Memory percentage per bucket, oldest first.
    pub memory: Vec<Option<f64>>,
    /// Minimum, maximum and mean over the window.
    pub summary: Option<MetricSummary>,
    /// Unix seconds the host was last seen, if it has stopped reporting.
    pub offline_since: Option<i64>,
}

impl HostHistory {
    /// Returns true when there is nothing to plot.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cpu.iter().all(Option::is_none) && self.memory.iter().all(Option::is_none)
    }

    /// Returns min, max and mean for one metric, as whole percentages.
    #[must_use]
    pub fn stats(&self, metric: MetricKind) -> Option<(f64, f64, f64)> {
        let stats = self.summary.as_ref()?.stats(metric)?;
        Some((stats.min, stats.max, stats.avg))
    }
}

/// Draws a series as a row of block characters.
///
/// Scaled between the smallest and largest value present rather than 0-100:
/// a host sitting between 3% and 7% CPU is a flat line at the bottom on an
/// absolute scale, which hides the shape the sparkline exists to show. A
/// series with no variation renders at the lowest level.
#[must_use]
pub fn sparkline(values: &[Option<f64>]) -> String {
    let present: Vec<f64> = values.iter().filter_map(|v| *v).collect();
    if present.is_empty() {
        return String::new();
    }

    let min = present.iter().copied().fold(f64::INFINITY, f64::min);
    let max = present.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;

    values
        .iter()
        .map(|value| match value {
            None => SPARK_GAP,
            Some(v) => {
                if span <= f64::EPSILON {
                    return SPARK_LEVELS[0];
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = (((v - min) / span) * (SPARK_LEVELS.len() - 1) as f64).round() as usize;
                SPARK_LEVELS[index.min(SPARK_LEVELS.len() - 1)]
            }
        })
        .collect()
}

/// Formats a "last seen" timestamp as an age, for the dashboard.
///
/// Returns `None` when the host is reporting, so the caller can leave the
/// line out rather than printing "offline for 0s" next to a live host.
#[must_use]
pub fn offline_for(offline_since: Option<i64>, now: i64) -> Option<String> {
    let since = offline_since?;
    let seconds = now.saturating_sub(since).max(0);

    Some(match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
        _ => format!("{}d {}h", seconds / 86_400, (seconds % 86_400) / 3600),
    })
}

/// Live and durable fleet metrics.
pub struct Telemetry {
    store: Option<MetricStore>,
    /// SSH host id to the row id the store uses.
    host_rows: HashMap<u32, i64>,
    /// The most recent sample per host, for the dashboard's live view.
    latest: HashMap<u32, DeviceMetrics>,
    /// When each host's last *stored* sample was collected.
    ///
    /// The dashboard polls on every frame and hands back the same cached
    /// sample until the collector produces a new one; without this the store
    /// would see a write per host per frame, almost all of them redundant.
    last_stored: HashMap<u32, Instant>,
    rules: Vec<AlertRule>,
    policy: RetentionPolicy,
    last_downsample: Option<Instant>,
    /// Set once, so a broken database does not warn on every sample.
    warned_about_store: bool,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Telemetry")
            .field("durable", &self.store.is_some())
            .field("hosts", &self.latest.len())
            .field("rules", &self.rules.len())
            .finish()
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::in_memory_only()
    }
}

impl Telemetry {
    /// Creates a live-only instance with no durable store.
    ///
    /// Used by tests, by fixture runs, and as the fallback when the database
    /// cannot be opened.
    #[must_use]
    pub fn in_memory_only() -> Self {
        Self {
            store: None,
            host_rows: HashMap::new(),
            latest: HashMap::new(),
            last_stored: HashMap::new(),
            rules: Vec::new(),
            policy: RetentionPolicy::default(),
            last_downsample: None,
            warned_about_store: false,
        }
    }

    /// Opens the durable store at `path`.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened. Callers that would
    /// rather degrade than fail should use [`Telemetry::open_or_live_only`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let store = MetricStore::open(path)?;
        Ok(Self {
            store: Some(store),
            ..Self::in_memory_only()
        })
    }

    /// Opens the durable store at the default path, degrading to live-only.
    #[must_use]
    pub fn open_or_live_only() -> Self {
        match MetricStore::default_path().and_then(MetricStore::open) {
            Ok(store) => {
                info!("metric history: {:?}", store.path());
                Self {
                    store: Some(store),
                    ..Self::in_memory_only()
                }
            }
            Err(e) => {
                warn!("metric history is unavailable, keeping live values only: {e}");
                Self::in_memory_only()
            }
        }
    }

    /// Creates an instance backed by an in-memory database.
    ///
    /// Behaves like the durable store in every way except that nothing
    /// survives the process, which is what tests want.
    ///
    /// # Errors
    /// Returns an error if the in-memory database cannot be created.
    pub fn ephemeral() -> Result<Self, StoreError> {
        Ok(Self {
            store: Some(MetricStore::open_in_memory()?),
            ..Self::in_memory_only()
        })
    }

    /// Returns true if samples are being written to a database.
    #[must_use]
    pub const fn is_durable(&self) -> bool {
        self.store.is_some()
    }

    /// Replaces the alert rules.
    pub fn set_rules(&mut self, rules: Vec<AlertRule>) {
        self.rules = rules;
    }

    /// Returns the alert rules in force.
    #[must_use]
    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }

    /// Sets the retention policy used by [`Telemetry::maybe_downsample`].
    pub fn set_policy(&mut self, policy: RetentionPolicy) {
        self.policy = policy;
    }

    /// Records a sample from either collector.
    ///
    /// Returns the alerts that started firing, so the caller can put them in
    /// the status bar. A sample that reports an error is kept in the live view
    /// but not written to the store: there is nothing to plot, and writing a
    /// row of nulls would make "offline since" wrong.
    pub fn ingest(&mut self, label: &str, metrics: &DeviceMetrics) -> Vec<String> {
        self.ingest_at(label, metrics, unix_now())
    }

    /// Records a sample with an explicit timestamp.
    ///
    /// The store deliberately never reads the clock; this keeps that property
    /// one level up, so a test can lay out a history without sleeping. Two
    /// samples with the same timestamp are one row: a second is finer than any
    /// collector polls.
    pub fn ingest_at(&mut self, label: &str, metrics: &DeviceMetrics, now: i64) -> Vec<String> {
        let host_id = metrics.host_id;
        self.latest.insert(host_id, metrics.clone());

        if metrics.status != MetricStatus::Online {
            return Vec::new();
        }

        // Same collection as last time: the caller is polling, not reporting.
        if self.last_stored.get(&host_id) == Some(&metrics.timestamp) {
            return Vec::new();
        }
        self.last_stored.insert(host_id, metrics.timestamp);

        let Some(row) = self.row_for(host_id, label, now) else {
            return Vec::new();
        };

        let sample = to_sample(row, now, metrics);
        let rules = self.rules.clone();

        let Some(store) = self.store.as_mut() else {
            return Vec::new();
        };

        match store.ingest(&sample, &rules) {
            Ok(update) => update
                .fired
                .iter()
                .map(|firing| {
                    format!(
                        "{label}: {} is {:.1} ({} {:.1})",
                        firing.rule.metric.as_str(),
                        firing.value,
                        firing.rule.comparison.as_str(),
                        firing.rule.threshold
                    )
                })
                .collect(),
            Err(e) => {
                self.warn_once(&format!("could not record a metric sample: {e}"));
                Vec::new()
            }
        }
    }

    /// Returns the latest live sample for a host.
    #[must_use]
    pub fn latest(&self, host_id: u32) -> Option<&DeviceMetrics> {
        self.latest.get(&host_id)
    }

    /// Returns every live sample.
    #[must_use]
    pub fn all_latest(&self) -> &HashMap<u32, DeviceMetrics> {
        &self.latest
    }

    /// Forgets the live samples, without touching the stored history.
    pub fn clear_live(&mut self) {
        self.latest.clear();
        self.last_stored.clear();
    }

    /// Returns a bucketed series for a chart.
    ///
    /// Returns an empty vector when there is no durable store, which the
    /// dashboard renders as "no history yet".
    #[must_use]
    pub fn series(
        &self,
        host_id: u32,
        metric: MetricKind,
        from_ts: i64,
        to_ts: i64,
        buckets: usize,
    ) -> Vec<Option<f64>> {
        let Some((store, row)) = self.store.as_ref().zip(self.host_rows.get(&host_id)) else {
            return Vec::new();
        };
        store
            .sparkline_series(*row, metric, from_ts, to_ts, buckets)
            .unwrap_or_default()
    }

    /// Returns minimum, maximum and mean over a window.
    #[must_use]
    pub fn summary(&self, host_id: u32, from_ts: i64, to_ts: i64) -> Option<MetricSummary> {
        let store = self.store.as_ref()?;
        let row = self.host_rows.get(&host_id)?;
        store.summary(*row, from_ts, to_ts).ok()
    }

    /// Returns when the host was last seen, if it has stopped reporting.
    #[must_use]
    pub fn offline_since(&self, host_id: u32) -> Option<i64> {
        let store = self.store.as_ref()?;
        let row = self.host_rows.get(&host_id)?;
        store.offline_since(*row).ok().flatten()
    }

    /// Returns the most recent alerts across the fleet.
    #[must_use]
    pub fn recent_alerts(&self) -> Vec<AlertRecord> {
        self.store
            .as_ref()
            .and_then(|store| store.recent_alerts(RECENT_ALERT_LIMIT).ok())
            .unwrap_or_default()
    }

    /// Collects everything the detail view needs about one host's past.
    ///
    /// Assembled here rather than in the widget so the numbers can be tested
    /// without a terminal, and so the widget cannot accidentally hold the store
    /// open across a render.
    #[must_use]
    pub fn host_history(&self, host_id: u32, window: Duration, buckets: usize) -> HostHistory {
        self.host_history_at(host_id, window, buckets, unix_now())
    }

    /// [`Telemetry::host_history`] with the clock supplied.
    #[must_use]
    pub fn host_history_at(
        &self,
        host_id: u32,
        window: Duration,
        buckets: usize,
        now: i64,
    ) -> HostHistory {
        let from = now.saturating_sub(i64::try_from(window.as_secs()).unwrap_or(i64::MAX));

        HostHistory {
            durable: self.is_durable(),
            window,
            cpu: self.series(host_id, MetricKind::CpuPercent, from, now, buckets),
            memory: self.series(host_id, MetricKind::MemUsedPercent, from, now, buckets),
            summary: self.summary(host_id, from, now),
            offline_since: self.offline_since(host_id),
        }
    }

    /// Runs downsampling if enough time has passed.
    ///
    /// Returns how many rows were collapsed and deleted, or `None` if it was
    /// not time yet. Call from the application's tick.
    pub fn maybe_downsample(&mut self) -> Option<(usize, usize)> {
        let due = self
            .last_downsample
            .is_none_or(|last| last.elapsed() >= DOWNSAMPLE_INTERVAL);
        if !due {
            return None;
        }
        self.last_downsample = Some(Instant::now());

        let now = unix_now();
        let policy = self.policy;
        let store = self.store.as_mut()?;

        match store.downsample(now, &policy) {
            Ok(report) => {
                let collapsed =
                    usize::try_from(report.minute_written + report.hour_written).unwrap_or(0);
                let deleted = usize::try_from(report.deleted).unwrap_or(0);
                if collapsed > 0 || deleted > 0 {
                    info!("downsampled {collapsed} rows, deleted {deleted}");
                }
                Some((collapsed, deleted))
            }
            Err(e) => {
                self.warn_once(&format!("could not downsample the metric history: {e}"));
                None
            }
        }
    }

    /// Returns the store row for an SSH host, registering it on first sight.
    fn row_for(&mut self, host_id: u32, label: &str, now: i64) -> Option<i64> {
        if let Some(row) = self.host_rows.get(&host_id) {
            return Some(*row);
        }

        let store = self.store.as_mut()?;
        match store.register_host(label, now) {
            Ok(row) => {
                self.host_rows.insert(host_id, row);
                Some(row)
            }
            Err(e) => {
                self.warn_once(&format!("could not register host {label}: {e}"));
                None
            }
        }
    }

    /// Logs a store problem once per session.
    fn warn_once(&mut self, message: &str) {
        if !self.warned_about_store {
            self.warned_about_store = true;
            warn!("{message}; metric history is degraded for this session");
        }
    }
}

/// Returns the current time as Unix seconds.
#[must_use]
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Converts a collected sample into the store's row shape.
///
/// The collectors report memory in mebibytes and disk in gibibytes because
/// that is what the dashboard shows; the store keeps bytes so a later change
/// of units does not need a migration.
#[must_use]
pub fn to_sample(row: i64, ts: i64, metrics: &DeviceMetrics) -> MetricSample {
    const MIB: i64 = 1024 * 1024;
    const GIB: i64 = 1024 * 1024 * 1024;

    let mut sample = MetricSample::new(row, ts);

    if metrics.cpu_usage_percent > 0.0 {
        sample.cpu_percent = Some(f64::from(metrics.cpu_usage_percent));
    }
    if metrics.load_avg.0 > 0.0 {
        sample.cpu_load1 = Some(f64::from(metrics.load_avg.0));
    }
    if metrics.mem_total_mb > 0 {
        sample.mem_total_bytes = i64::try_from(metrics.mem_total_mb).ok().map(|v| v * MIB);
        sample.mem_used_bytes = i64::try_from(metrics.mem_used_mb).ok().map(|v| v * MIB);
    }
    if metrics.disk_total_gb > 0 {
        sample.disk_total_bytes = i64::try_from(metrics.disk_total_gb).ok().map(|v| v * GIB);
        sample.disk_used_bytes = i64::try_from(metrics.disk_used_gb).ok().map(|v| v * GIB);
    }
    if let Some(gpu) = metrics.gpu.as_ref() {
        sample.gpu_util_percent = Some(f64::from(gpu.usage_percent));
        sample.gpu_mem_used_bytes = i64::try_from(gpu.memory_used_mb).ok().map(|v| v * MIB);
        sample.temperature_c = gpu.temperature_celsius.map(f64::from);
    }

    sample
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ssh::metrics::GpuMetrics;
    use crate::store::{AlertMetric, Comparison};

    fn ok_metrics(host_id: u32) -> DeviceMetrics {
        let mut m = DeviceMetrics::new(host_id);
        m.status = MetricStatus::Online;
        m.cpu_usage_percent = 42.0;
        m.load_avg = (1.5, 1.2, 1.0);
        m.mem_total_mb = 8192;
        m.mem_used_mb = 4096;
        m.disk_total_gb = 500;
        m.disk_used_gb = 250;
        m
    }

    #[test]
    fn a_live_only_instance_keeps_the_latest_sample() {
        let mut telemetry = Telemetry::in_memory_only();
        assert!(!telemetry.is_durable());

        telemetry.ingest("host-a", &ok_metrics(1));
        assert!(telemetry.latest(1).is_some());
        assert_eq!(telemetry.all_latest().len(), 1);
        assert!(telemetry.latest(2).is_none());
    }

    #[test]
    fn a_later_sample_replaces_the_earlier_one_in_the_live_view() {
        let mut telemetry = Telemetry::in_memory_only();
        telemetry.ingest("host-a", &ok_metrics(1));

        let mut second = ok_metrics(1);
        second.cpu_usage_percent = 99.0;
        telemetry.ingest("host-a", &second);

        let latest = telemetry.latest(1).expect("a sample");
        assert!((latest.cpu_usage_percent - 99.0).abs() < f32::EPSILON);
    }

    #[test]
    fn clearing_the_live_view_forgets_every_host() {
        let mut telemetry = Telemetry::in_memory_only();
        telemetry.ingest("host-a", &ok_metrics(1));
        telemetry.clear_live();
        assert!(telemetry.all_latest().is_empty());
    }

    #[test]
    fn a_durable_instance_writes_history() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        assert!(telemetry.is_durable());

        let base = 1_700_000_000;
        for offset in [0, 60, 120] {
            // A fresh sample each time, as a real collection would be.
            let mut metrics = ok_metrics(1);
            metrics.timestamp = Instant::now();
            std::thread::sleep(Duration::from_millis(1));
            telemetry.ingest_at("host-a", &metrics, base + offset);
        }

        let summary = telemetry
            .summary(1, base - 3600, base + 3600)
            .expect("a summary");
        assert_eq!(summary.sample_count, 3, "{summary:?}");
    }

    #[test]
    fn the_same_collection_is_not_stored_twice() {
        // What the dashboard does on every frame: hand back the cached sample.
        let mut telemetry = Telemetry::ephemeral().expect("store");
        let metrics = ok_metrics(1);
        let base = 1_700_000_000;

        telemetry.ingest_at("host-a", &metrics, base);
        telemetry.ingest_at("host-a", &metrics, base + 60);
        telemetry.ingest_at("host-a", &metrics, base + 120);

        let summary = telemetry
            .summary(1, base - 3600, base + 3600)
            .expect("a summary");
        assert_eq!(
            summary.sample_count, 1,
            "polling must not write a row per frame"
        );
    }

    #[test]
    fn two_samples_in_the_same_second_are_one_row() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        let base = 1_700_000_000;
        let mut first = ok_metrics(1);
        first.timestamp = Instant::now();
        telemetry.ingest_at("host-a", &first, base);
        std::thread::sleep(Duration::from_millis(1));
        let mut second = ok_metrics(1);
        second.timestamp = Instant::now();
        telemetry.ingest_at("host-a", &second, base);

        let summary = telemetry
            .summary(1, base - 60, base + 60)
            .expect("a summary");
        assert_eq!(summary.sample_count, 1, "{summary:?}");
    }

    #[test]
    fn a_series_has_the_requested_number_of_buckets() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        telemetry.ingest("host-a", &ok_metrics(1));

        let now = unix_now();
        let series = telemetry.series(1, MetricKind::CpuPercent, now - 600, now + 600, 20);
        assert!(!series.is_empty());
        assert_eq!(series.len(), 20);
        assert!(series.iter().any(Option::is_some), "{series:?}");
    }

    #[test]
    fn a_live_only_instance_has_no_history_to_offer() {
        let mut telemetry = Telemetry::in_memory_only();
        telemetry.ingest("host-a", &ok_metrics(1));

        let now = unix_now();
        assert!(
            telemetry
                .series(1, MetricKind::CpuPercent, now - 60, now, 10)
                .is_empty()
        );
        assert!(telemetry.summary(1, now - 60, now).is_none());
        assert!(telemetry.offline_since(1).is_none());
        assert!(telemetry.recent_alerts().is_empty());
    }

    #[test]
    fn a_failed_sample_is_kept_live_but_not_stored() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        let mut failed = DeviceMetrics::new(7);
        failed.status = MetricStatus::Error;
        failed.error = Some("timed out".to_string());

        telemetry.ingest("host-g", &failed);

        assert!(
            telemetry.latest(7).is_some(),
            "the dashboard still shows it"
        );
        let now = unix_now();
        assert!(
            telemetry.summary(7, now - 3600, now + 3600).is_none(),
            "a row of nulls would make offline-since wrong"
        );
    }

    #[test]
    fn an_alert_fires_once_and_is_reported() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        telemetry.set_rules(vec![AlertRule::new(
            AlertMetric::CpuPercent,
            Comparison::Above,
            10.0,
        )]);

        let fired = telemetry.ingest("host-a", &ok_metrics(1));
        assert_eq!(fired.len(), 1, "{fired:?}");
        assert!(fired[0].contains("host-a"), "{fired:?}");

        let again = telemetry.ingest("host-a", &ok_metrics(1));
        assert!(again.is_empty(), "an alert must not re-fire while active");

        assert!(!telemetry.recent_alerts().is_empty());
    }

    #[test]
    fn no_rules_means_no_alerts() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        assert!(telemetry.rules().is_empty());
        assert!(telemetry.ingest("host-a", &ok_metrics(1)).is_empty());
    }

    #[test]
    fn units_are_converted_to_bytes() {
        let metrics = ok_metrics(1);
        let sample = to_sample(1, 100, &metrics);

        assert_eq!(sample.mem_total_bytes, Some(8192 * 1024 * 1024));
        assert_eq!(sample.mem_used_bytes, Some(4096 * 1024 * 1024));
        assert_eq!(sample.disk_total_bytes, Some(500 * 1024 * 1024 * 1024));
        assert_eq!(sample.cpu_percent, Some(42.0));
        assert_eq!(sample.cpu_load1, Some(1.5));
        assert_eq!(sample.ts, 100);
    }

    #[test]
    fn absent_metrics_stay_absent() {
        let mut metrics = DeviceMetrics::new(1);
        metrics.status = MetricStatus::Online;
        let sample = to_sample(1, 100, &metrics);

        assert_eq!(sample.cpu_percent, None);
        assert_eq!(sample.mem_total_bytes, None);
        assert_eq!(sample.disk_total_bytes, None);
        assert_eq!(sample.gpu_util_percent, None);
        assert_eq!(sample.temperature_c, None);
    }

    #[test]
    fn gpu_metrics_are_carried_across() {
        let mut metrics = ok_metrics(1);
        metrics.gpu = Some(GpuMetrics {
            name: "test".to_string(),
            gpu_type: crate::ssh::metrics::GpuType::Nvidia,
            usage_percent: 77.0,
            memory_used_mb: 2048,
            memory_total_mb: 8192,
            temperature_celsius: Some(61.0),
        });

        let sample = to_sample(1, 100, &metrics);
        assert_eq!(sample.gpu_util_percent, Some(77.0));
        assert_eq!(sample.gpu_mem_used_bytes, Some(2048 * 1024 * 1024));
        assert_eq!(sample.temperature_c, Some(61.0));
    }

    #[test]
    fn downsampling_runs_at_most_once_per_interval() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        telemetry.ingest("host-a", &ok_metrics(1));

        assert!(
            telemetry.maybe_downsample().is_some(),
            "the first call runs"
        );
        assert!(
            telemetry.maybe_downsample().is_none(),
            "the second is too soon"
        );
    }

    #[test]
    fn downsampling_a_live_only_instance_does_nothing() {
        let mut telemetry = Telemetry::in_memory_only();
        assert!(telemetry.maybe_downsample().is_none());
    }

    #[test]
    fn opening_a_directory_as_a_database_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(Telemetry::open(dir.path()).is_err());
    }

    #[test]
    fn debug_output_says_whether_history_is_on() {
        let telemetry = Telemetry::in_memory_only();
        assert!(format!("{telemetry:?}").contains("durable: false"));

        let telemetry = Telemetry::ephemeral().expect("store");
        assert!(format!("{telemetry:?}").contains("durable: true"));
    }

    #[test]
    fn the_clock_helper_returns_a_plausible_time() {
        // Any time after 2020 and before 2100.
        let now = unix_now();
        assert!(now > 1_577_836_800, "{now}");
        assert!(now < 4_102_444_800, "{now}");
    }

    #[test]
    fn a_sparkline_of_nothing_is_empty() {
        assert_eq!(sparkline(&[]), "");
        assert_eq!(sparkline(&[None, None, None]), "");
    }

    #[test]
    fn a_sparkline_uses_the_full_height_range() {
        let line = sparkline(&[Some(0.0), Some(50.0), Some(100.0)]);
        assert_eq!(line.chars().count(), 3);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(
            chars[0], SPARK_LEVELS[0],
            "the lowest value is the shortest"
        );
        assert_eq!(
            chars[2],
            SPARK_LEVELS[SPARK_LEVELS.len() - 1],
            "the highest value is the tallest"
        );
        assert!(chars[1] > chars[0] && chars[1] < chars[2], "{line}");
    }

    #[test]
    fn a_sparkline_is_scaled_to_the_values_present() {
        // Between 3% and 7% on an absolute 0-100 scale is a flat line, which
        // is exactly the shape a sparkline should be showing.
        let line = sparkline(&[Some(3.0), Some(7.0), Some(5.0)]);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars[0], SPARK_LEVELS[0]);
        assert_eq!(chars[1], SPARK_LEVELS[SPARK_LEVELS.len() - 1]);
    }

    #[test]
    fn a_flat_sparkline_does_not_divide_by_zero() {
        let line = sparkline(&[Some(42.0), Some(42.0), Some(42.0)]);
        assert_eq!(line, "▁▁▁");
    }

    #[test]
    fn a_gap_is_blank_rather_than_the_lowest_block() {
        let line = sparkline(&[Some(10.0), None, Some(20.0)]);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars[1], SPARK_GAP, "a missing sample is not a low sample");
        assert_eq!(chars.len(), 3, "the gap still occupies its slot");
    }

    #[test]
    fn a_negative_value_still_plots() {
        // Temperature deltas and load can go below zero on some reporters.
        let line = sparkline(&[Some(-5.0), Some(0.0), Some(5.0)]);
        assert_eq!(line.chars().count(), 3);
        assert_eq!(line.chars().next(), Some(SPARK_LEVELS[0]));
    }

    #[test]
    fn a_reporting_host_has_no_offline_age() {
        assert_eq!(offline_for(None, 1_700_000_000), None);
    }

    #[test]
    fn an_offline_age_is_written_at_a_useful_scale() {
        let now = 1_700_000_000;
        assert_eq!(offline_for(Some(now - 30), now).as_deref(), Some("30s"));
        assert_eq!(offline_for(Some(now - 600), now).as_deref(), Some("10m"));
        assert_eq!(
            offline_for(Some(now - 7_260), now).as_deref(),
            Some("2h 1m")
        );
        assert_eq!(
            offline_for(Some(now - 180_000), now).as_deref(),
            Some("2d 2h")
        );
    }

    #[test]
    fn a_clock_that_went_backwards_does_not_report_a_negative_age() {
        let now = 1_700_000_000;
        assert_eq!(offline_for(Some(now + 500), now).as_deref(), Some("0s"));
    }

    #[test]
    fn a_history_with_no_store_says_so_rather_than_looking_empty() {
        let telemetry = Telemetry::in_memory_only();
        let history = telemetry.host_history(1, Duration::from_secs(3600), 32);
        assert!(!history.durable, "the view must be able to explain itself");
        assert!(history.is_empty());
        assert!(history.summary.is_none());
    }

    #[test]
    fn a_history_carries_the_series_and_the_summary() {
        let mut telemetry = Telemetry::ephemeral().expect("store");
        let base = 1_700_000_000;

        for (offset, cpu) in [(0, 10.0), (60, 50.0), (120, 90.0)] {
            let mut metrics = ok_metrics(1);
            metrics.cpu_usage_percent = cpu;
            metrics.timestamp = Instant::now();
            std::thread::sleep(Duration::from_millis(1));
            telemetry.ingest_at("host-a", &metrics, base + offset);
        }

        let history = telemetry.host_history_at(1, Duration::from_secs(3600), 8, base + 180);
        assert!(history.durable);
        assert!(!history.is_empty(), "{history:?}");

        let (min, max, avg) = history
            .stats(MetricKind::CpuPercent)
            .expect("cpu statistics");
        assert!((min - 10.0).abs() < 0.5, "min was {min}");
        assert!((max - 90.0).abs() < 0.5, "max was {max}");
        assert!((avg - 50.0).abs() < 0.5, "avg was {avg}");

        let line = sparkline(&history.cpu);
        assert!(!line.is_empty(), "a plotted history draws something");
    }

    #[test]
    fn a_history_for_an_unknown_host_is_empty_but_durable() {
        let telemetry = Telemetry::ephemeral().expect("store");
        let history = telemetry.host_history_at(99, Duration::from_secs(3600), 8, 1_700_000_000);
        assert!(history.durable);
        assert!(history.is_empty());
        assert!(history.stats(MetricKind::CpuPercent).is_none());
    }
}
