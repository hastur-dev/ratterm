//! Pod logs, delivered into the Docker log buffer and storage.
//!
//! Pod lines become the same `LogEntry` Docker lines become, and are indexed
//! through the same `LogStorage`, so search, saved patterns, retention and the
//! log viewer work on Kubernetes logs without a second implementation.
//!
//! # How following is implemented
//!
//! `kube` exposes a following log stream as a `futures_io::AsyncBufRead`.
//! Reading one line at a time from it needs the `futures-io` traits in scope,
//! and this crate does not depend on `futures`; adding a dependency for it was
//! out of scope for this change. Following is therefore done by asking the API
//! server for the last few seconds of output once a second, with server-side
//! timestamps switched on, and dropping lines already delivered. The entries
//! that reach the buffer are the same ones a stream would deliver, with up to
//! [`POLL_INTERVAL`] of extra latency. Replacing the poll with a real stream is
//! a change confined to [`follow_loop`].

mod line;
mod options;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, LogParams};

use crate::docker_logs::log_buffer::LogBuffer;
use crate::docker_logs::log_storage::LogStorage;
use crate::docker_logs::types::LogEntry;

use super::client::K8sClient;
use super::{K8sError, Result, block_on, runtime};

pub use line::{
    FollowCursor, MAX_LOG_LINE_BYTES, TRUNCATION_MARKER, log_display_name, log_entry_from_line,
    log_source_id, split_timestamp, truncate_line,
};
pub use options::PodLogOptions;

/// How long the follower waits between polls.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Extra seconds of history each poll asks for, so a line written just before
/// the previous request was answered is not missed. The overlap is removed by
/// the cursor.
const POLL_OVERLAP_SECONDS: i64 = 2;

/// Upper bound on how many polls one follower makes before it stops.
///
/// At [`POLL_INTERVAL`] this is about eleven days, which is far longer than a
/// log view stays open, and it keeps the loop bounded.
const MAX_POLLS: u64 = 1_000_000;

/// A running pod log follower.
///
/// Dropping this stops it.
pub struct PodLogStream {
    buffer: Arc<Mutex<LogBuffer>>,
    target: String,
    stop: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for PodLogStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodLogStream")
            .field("target", &self.target)
            .field("running", &self.is_running())
            .finish()
    }
}

impl PodLogStream {
    /// Returns the buffer lines are pushed into.
    #[must_use]
    pub fn buffer(&self) -> Arc<Mutex<LogBuffer>> {
        self.buffer.clone()
    }

    /// Returns what is being followed, for the status bar.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// True while the follower is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::SeqCst) && self.task.as_ref().is_none_or(|t| !t.is_finished())
    }

    /// Returns the most recent failure, if the follower has had one.
    ///
    /// A poll that fails does not stop the follower; the cluster may simply
    /// have been restarting.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        match self.last_error.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Stops the follower.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for PodLogStream {
    fn drop(&mut self) {
        self.stop();
    }
}

impl K8sClient {
    /// Reads a pod's logs once and returns them as entries.
    ///
    /// # Errors
    /// Whatever the API server reported. A pod with more than one container
    /// and no `container` option is reported by the API server as a bad
    /// request.
    pub fn pod_logs(
        &self,
        namespace: &str,
        pod: &str,
        options: &PodLogOptions,
    ) -> Result<Vec<LogEntry>> {
        let text = self.fetch_logs(namespace, pod, &options.to_params())?;
        let now = Utc::now();
        Ok(text
            .lines()
            .map(|line| {
                log_entry_from_line(line, namespace, pod, options.container.as_deref(), now)
            })
            .collect())
    }

    /// Follows a pod's logs into `buffer`, and into `storage` when one is
    /// given.
    ///
    /// The first poll delivers the history the options ask for; later polls
    /// deliver what is new. Returns immediately; the work happens on the
    /// Kubernetes runtime.
    ///
    /// # Errors
    /// [`K8sError::Runtime`] if the follower cannot be started.
    pub fn follow_pod_logs(
        &self,
        namespace: &str,
        pod: &str,
        options: &PodLogOptions,
        buffer: Arc<Mutex<LogBuffer>>,
        storage: Option<Arc<LogStorage>>,
    ) -> Result<PodLogStream> {
        let api: Api<Pod> = Api::namespaced(self.inner().clone(), namespace);
        let stop = Arc::new(AtomicBool::new(false));
        let last_error = Arc::new(Mutex::new(None));
        let target = log_display_name(namespace, pod, options.container.as_deref());

        let task_context = FollowContext {
            api,
            namespace: namespace.to_string(),
            pod: pod.to_string(),
            options: options.clone(),
            buffer: buffer.clone(),
            storage,
            stop: stop.clone(),
            last_error: last_error.clone(),
        };

        let task = runtime()?.spawn(async move { follow_loop(task_context).await });

        Ok(PodLogStream {
            buffer,
            target,
            stop,
            task: Some(task),
            last_error,
        })
    }

    /// Runs one log request and returns the raw body.
    fn fetch_logs(&self, namespace: &str, pod: &str, params: &LogParams) -> Result<String> {
        let api: Api<Pod> = Api::namespaced(self.inner().clone(), namespace);
        let context = self.context().to_string();
        let action = format!("read logs of pod {namespace}/{pod}");
        let name = pod.to_string();
        let params = params.clone();

        block_on(async move { api.logs(&name, &params).await })?
            .map_err(|e| K8sError::from_kube(&e, &context, &action))
    }
}

/// Everything the follower task owns.
struct FollowContext {
    api: Api<Pod>,
    namespace: String,
    pod: String,
    options: PodLogOptions,
    buffer: Arc<Mutex<LogBuffer>>,
    storage: Option<Arc<LogStorage>>,
    stop: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
}

/// Polls for new log lines until the follower is stopped.
///
/// Bounded by [`MAX_POLLS`] as well as the stop flag, so it cannot run forever
/// on network input.
async fn follow_loop(context: FollowContext) {
    let FollowContext {
        api,
        namespace,
        pod,
        options,
        buffer,
        storage,
        stop,
        last_error,
    } = context;

    let mut cursor = FollowCursor::new();
    let mut params = options.to_params();
    let source_id = log_source_id(&namespace, &pod, options.container.as_deref());

    for poll in 0..MAX_POLLS {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        if poll > 0 {
            // Later polls ask only for the recent window; the first one
            // carries whatever history the caller asked for.
            params.tail_lines = None;
            params.since_seconds = Some(poll_window_seconds());
        }

        match api.logs(&pod, &params).await {
            Ok(text) => {
                set_error(&last_error, None);
                deliver(
                    &text,
                    &Target {
                        namespace: &namespace,
                        pod: &pod,
                        container: options.container.as_deref(),
                        source_id: &source_id,
                    },
                    &mut cursor,
                    &buffer,
                    storage.as_deref(),
                );
            }
            Err(e) => set_error(&last_error, Some(e.to_string())),
        }

        if !options.follow {
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// How many seconds of history each follow-up poll asks for.
fn poll_window_seconds() -> i64 {
    i64::try_from(POLL_INTERVAL.as_secs()).unwrap_or(1) + POLL_OVERLAP_SECONDS
}

/// Which pod a batch of lines came from.
struct Target<'a> {
    namespace: &'a str,
    pod: &'a str,
    container: Option<&'a str>,
    source_id: &'a str,
}

/// Pushes the new lines of one response into the buffer and the storage.
fn deliver(
    text: &str,
    target: &Target<'_>,
    cursor: &mut FollowCursor,
    buffer: &Arc<Mutex<LogBuffer>>,
    storage: Option<&LogStorage>,
) {
    let now = Utc::now();
    for line in text.lines() {
        let (timestamp, message) = split_timestamp(line);
        if !cursor.accept(timestamp, message) {
            continue;
        }

        let entry = log_entry_from_line(line, target.namespace, target.pod, target.container, now);
        if let Some(storage) = storage
            && let Err(e) = storage.append(target.source_id, &entry)
        {
            tracing::warn!("a pod log line could not be stored: {e}");
        }
        match buffer.lock() {
            Ok(mut guard) => guard.push(entry),
            Err(poisoned) => poisoned.into_inner().push(entry),
        }
    }
}

/// Records the follower's most recent failure, recovering from a poisoned
/// lock rather than leaving the follower permanently broken.
fn set_error(slot: &Arc<Mutex<Option<String>>>, message: Option<String>) {
    match slot.lock() {
        Ok(mut guard) => *guard = message,
        Err(poisoned) => *poisoned.into_inner() = message,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn target<'a>() -> Target<'a> {
        Target {
            namespace: "web",
            pod: "api-0",
            container: None,
            source_id: "k8s_web_api-0",
        }
    }

    #[test]
    fn the_poll_window_covers_the_interval_plus_the_overlap() {
        assert_eq!(poll_window_seconds(), 3);
    }

    #[test]
    fn delivered_lines_reach_the_buffer_once_each() {
        let buffer = Arc::new(Mutex::new(LogBuffer::new(100)));
        let mut cursor = FollowCursor::new();
        let batch = "2026-09-05T12:00:00Z one\n2026-09-05T12:00:01Z two\n";

        deliver(batch, &target(), &mut cursor, &buffer, None);
        // The overlapping window of the next poll repeats both lines.
        deliver(batch, &target(), &mut cursor, &buffer, None);

        let guard = buffer.lock().expect("lock");
        assert_eq!(guard.len(), 2);
        assert_eq!(guard.all_entries()[0].message, "one");
        assert_eq!(guard.all_entries()[1].message, "two");
    }

    #[test]
    fn a_later_batch_with_new_lines_appends_only_those() {
        let buffer = Arc::new(Mutex::new(LogBuffer::new(100)));
        let mut cursor = FollowCursor::new();

        deliver(
            "2026-09-05T12:00:00Z one\n",
            &target(),
            &mut cursor,
            &buffer,
            None,
        );
        deliver(
            "2026-09-05T12:00:00Z one\n2026-09-05T12:00:02Z three\n",
            &target(),
            &mut cursor,
            &buffer,
            None,
        );

        let guard = buffer.lock().expect("lock");
        assert_eq!(guard.len(), 2);
        assert_eq!(guard.all_entries()[1].message, "three");
    }

    #[test]
    fn an_empty_response_delivers_nothing() {
        let buffer = Arc::new(Mutex::new(LogBuffer::new(100)));
        let mut cursor = FollowCursor::new();
        deliver("", &target(), &mut cursor, &buffer, None);
        assert!(buffer.lock().expect("lock").is_empty());
    }

    #[test]
    fn delivered_lines_are_also_written_to_storage() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LogStorage::with_path(dir.path().to_path_buf(), true, 24);
        let buffer = Arc::new(Mutex::new(LogBuffer::new(100)));
        let mut cursor = FollowCursor::new();

        deliver(
            "2026-09-05T12:00:00Z stored line\n",
            &target(),
            &mut cursor,
            &buffer,
            Some(&storage),
        );

        let history = storage.read_history("k8s_web_api-0", 10).expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "stored line");
    }

    #[test]
    fn an_error_slot_records_and_clears() {
        let slot = Arc::new(Mutex::new(None));
        set_error(&slot, Some("boom".to_string()));
        assert_eq!(slot.lock().expect("lock").as_deref(), Some("boom"));
        set_error(&slot, None);
        assert!(slot.lock().expect("lock").is_none());
    }
}
