//! Durable metric and event store.
//!
//! Fleet metrics used to live in a `HashMap` that held the latest sample per
//! host and nothing else, so a restart lost the history and no question about
//! the past could be answered. This module replaces it with a SQLite database
//! at `~/.ratterm/ratterm.db`.
//!
//! [`MetricStore`] is the single entry point. Both collectors write through
//! it — the SSH poller that reaches out to hosts and the push daemon that
//! reports in — so alert state and retention behave the same whichever path a
//! sample arrived by.
//!
//! Layout:
//!
//! - [`schema`] — table definitions, connection pragmas, schema versioning
//! - [`metrics`] — sample ingest and row-level queries
//! - [`query`] — window summaries and bucketed series for charts
//! - [`events`] — container, Kubernetes, session and log-index rows
//! - [`retention`] — downsampling and deletion of old rows
//! - [`alerts`] — threshold rules and their evaluation, with no database
//! - [`alert_log`] — opening, clearing and reading stored alerts
//!
//! All timestamps are Unix seconds in UTC. Nothing in this module reads the
//! clock: the caller passes the current time, which keeps retention and alert
//! behaviour testable and stops a wrong system clock from deleting live data
//! on its own.
//!
//! ```no_run
//! use ratterm::store::{AlertMetric, AlertRule, Comparison, MetricSample, MetricStore};
//!
//! # fn main() -> Result<(), ratterm::store::StoreError> {
//! let mut store = MetricStore::open(MetricStore::default_path()?)?;
//! let host = store.register_host("cthulhu-computer", 1_700_000_000)?;
//!
//! let mut sample = MetricSample::new(host, 1_700_000_000);
//! sample.cpu_percent = Some(93.0);
//!
//! let rules = [AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 90.0)];
//! let update = store.ingest(&sample, &rules)?;
//! assert_eq!(update.fired.len(), 1);
//! # Ok(())
//! # }
//! ```

pub mod alert_log;
pub mod alerts;
pub mod error;
pub mod events;
pub mod metrics;
pub mod query;
pub mod retention;
pub mod schema;
pub mod types;

#[cfg(test)]
mod facade_tests;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use alert_log::{AlertRecord, AlertUpdate};
pub use alerts::{AlertFiring, AlertMetric, AlertRule, Comparison, evaluate};
pub use error::StoreError;
pub use query::MAX_BUCKETS;
pub use retention::{Cutoffs, DownsampleReport, RetentionPolicy};
pub use schema::SCHEMA_VERSION;
pub use types::{
    ContainerEvent, HostRow, K8sEvent, LogIndexEntry, MetricKind, MetricSample, MetricStats,
    MetricSummary, RESOLUTION_HOUR, RESOLUTION_MINUTE, RESOLUTION_RAW, SessionRecord,
};

/// Directory under the user's home that holds ratterm state.
const STATE_DIR: &str = ".ratterm";
/// File name of the database inside [`STATE_DIR`].
const DB_FILE: &str = "ratterm.db";

/// A connection to the metric and event database.
///
/// One process holds one of these. Reads take `&self`; writes take `&mut self`
/// because they run in a transaction, which SQLite scopes to the connection.
pub struct MetricStore {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for MetricStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The connection has no Debug impl and nothing useful to print.
        f.debug_struct("MetricStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl MetricStore {
    /// `~/.ratterm/ratterm.db`.
    pub fn default_path() -> Result<PathBuf, StoreError> {
        let home = dirs::home_dir().ok_or(StoreError::NoHomeDirectory)?;
        Ok(home.join(STATE_DIR).join(DB_FILE))
    }

    /// Opens the database at `path`, creating the file, its directory and the
    /// schema if they do not exist.
    ///
    /// Reopening a database of the current schema version leaves every row in
    /// place; the schema statements are all `IF NOT EXISTS`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::Directory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let conn = Connection::open(&path)?;
        // Reading a pragma touches page one, so a file that is not a database
        // fails here rather than on the first query a caller makes.
        schema::configure(&conn)?;
        schema::ensure_schema(&conn)?;
        tracing::debug!(path = %path.display(), "metric store opened");
        Ok(Self {
            conn,
            path: Some(path),
        })
    }

    /// Opens the database at [`MetricStore::default_path`].
    pub fn open_default() -> Result<Self, StoreError> {
        Self::open(Self::default_path()?)
    }

    /// Opens a private in-memory database. For tests, and for a run that must
    /// not write to disk.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        schema::configure(&conn)?;
        schema::ensure_schema(&conn)?;
        Ok(Self { conn, path: None })
    }

    /// Where the database lives, or `None` for an in-memory store.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Schema version recorded in this database.
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        schema::schema_version(&self.conn)
    }

    // -- hosts ------------------------------------------------------------

    /// Returns the numeric id for a host name, allocating one on first sight.
    pub fn register_host(&mut self, label: &str, now_ts: i64) -> Result<i64, StoreError> {
        metrics::register_host(&self.conn, label, now_ts)
    }

    /// Names a host that was first seen by id.
    pub fn set_host_label(&mut self, host_id: i64, label: &str) -> Result<(), StoreError> {
        metrics::set_host_label(&self.conn, host_id, label)
    }

    /// Every host the store knows about.
    pub fn hosts(&self) -> Result<Vec<HostRow>, StoreError> {
        metrics::hosts(&self.conn)
    }

    // -- ingest -----------------------------------------------------------

    /// Writes a sample and updates alert state in one transaction.
    ///
    /// This is the entry point for both collectors. Pass an empty rule slice
    /// to skip alert evaluation.
    pub fn ingest(
        &mut self,
        sample: &MetricSample,
        rules: &[AlertRule],
    ) -> Result<AlertUpdate, StoreError> {
        let tx = self.conn.transaction()?;
        metrics::insert_sample(&tx, sample)?;
        let update = alert_log::apply(&tx, sample, rules)?;
        tx.commit()?;
        Ok(update)
    }

    /// Writes one sample without evaluating alerts.
    pub fn record_sample(&mut self, sample: &MetricSample) -> Result<(), StoreError> {
        metrics::insert_sample(&self.conn, sample)
    }

    /// Writes many samples in one transaction and returns how many were
    /// written.
    ///
    /// Backfilling from a daemon's spool file goes through here; one
    /// transaction turns a few hundred round trips to disk into one.
    pub fn record_samples(&mut self, samples: &[MetricSample]) -> Result<usize, StoreError> {
        let tx = self.conn.transaction()?;
        let written = metrics::insert_samples(&tx, samples)?;
        tx.commit()?;
        Ok(written)
    }

    /// Records a Docker lifecycle event and returns its row id.
    pub fn record_container_event(&mut self, event: &ContainerEvent) -> Result<i64, StoreError> {
        events::record_container_event(&self.conn, event)
    }

    /// Records a Kubernetes event and returns its row id.
    pub fn record_k8s_event(&mut self, event: &K8sEvent) -> Result<i64, StoreError> {
        events::record_k8s_event(&self.conn, event)
    }

    /// Records a log file, replacing an earlier record of the same file.
    pub fn index_log_file(&mut self, entry: &LogIndexEntry) -> Result<i64, StoreError> {
        events::index_log_file(&self.conn, entry)
    }

    // -- queries ----------------------------------------------------------

    /// Samples for one host in `[from_ts, to_ts]`, both ends inclusive.
    pub fn samples_between(
        &self,
        host_id: i64,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<MetricSample>, StoreError> {
        metrics::samples_between(&self.conn, host_id, from_ts, to_ts)
    }

    /// Most recent sample for a host.
    pub fn latest_sample(&self, host_id: i64) -> Result<Option<MetricSample>, StoreError> {
        metrics::latest_sample(&self.conn, host_id)
    }

    /// Timestamp of the last sample from a host, or `None` if it never
    /// reported.
    pub fn offline_since(&self, host_id: i64) -> Result<Option<i64>, StoreError> {
        metrics::offline_since(&self.conn, host_id)
    }

    /// Minimum, maximum and mean of every metric over a window.
    pub fn summary(
        &self,
        host_id: i64,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<MetricSummary, StoreError> {
        query::summary(&self.conn, host_id, from_ts, to_ts)
    }

    /// A fixed-length bucketed series for a ratatui `Sparkline`.
    pub fn sparkline_series(
        &self,
        host_id: i64,
        metric: MetricKind,
        from_ts: i64,
        to_ts: i64,
        buckets: usize,
    ) -> Result<Vec<Option<f64>>, StoreError> {
        query::sparkline_series(&self.conn, host_id, metric, from_ts, to_ts, buckets)
    }

    /// Container events for one host in a window, oldest first.
    pub fn container_events_between(
        &self,
        host_id: i64,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<ContainerEvent>, StoreError> {
        events::container_events_between(&self.conn, host_id, from_ts, to_ts)
    }

    /// Kubernetes events for one context in a window, oldest first.
    pub fn k8s_events_between(
        &self,
        context: &str,
        namespace: Option<&str>,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<K8sEvent>, StoreError> {
        events::k8s_events_between(&self.conn, context, namespace, from_ts, to_ts)
    }

    /// Log files for a container between two `YYYY-MM-DD` dates.
    pub fn log_files_between(
        &self,
        host_id: i64,
        container_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Vec<LogIndexEntry>, StoreError> {
        events::log_files_between(&self.conn, host_id, container_id, from_date, to_date)
    }

    // -- sessions ---------------------------------------------------------

    /// Opens a session row and returns its id.
    pub fn start_session(&mut self, started: i64) -> Result<i64, StoreError> {
        events::start_session(&self.conn, started)
    }

    /// Closes a session and records the hosts it touched.
    pub fn end_session(
        &mut self,
        id: i64,
        ended: i64,
        hosts_touched: &[i64],
    ) -> Result<(), StoreError> {
        events::end_session(&self.conn, id, ended, hosts_touched)
    }

    /// The most recent sessions, newest first.
    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<SessionRecord>, StoreError> {
        events::recent_sessions(&self.conn, limit)
    }

    // -- alerts -----------------------------------------------------------

    /// Alerts still firing for one host.
    pub fn active_alerts(&self, host_id: i64) -> Result<Vec<AlertRecord>, StoreError> {
        alert_log::active_alerts(&self.conn, host_id)
    }

    /// The most recently opened alerts across every host.
    pub fn recent_alerts(&self, limit: usize) -> Result<Vec<AlertRecord>, StoreError> {
        alert_log::recent_alerts(&self.conn, limit)
    }

    // -- retention --------------------------------------------------------

    /// Applies a retention policy as of `now_ts`.
    pub fn downsample(
        &mut self,
        now_ts: i64,
        policy: &RetentionPolicy,
    ) -> Result<DownsampleReport, StoreError> {
        retention::downsample(&mut self.conn, now_ts, policy)
    }
}
