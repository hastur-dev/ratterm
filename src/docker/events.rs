//! One ingest path for container lifecycle events.
//!
//! The Docker event stream is the only record of what happened to a container
//! while nobody was watching. Every event, from every host, goes through
//! [`DockerEvents::ingest`]: it keeps a bounded ring in memory so the fleet
//! view always has something to show, and writes the same event to the durable
//! store so "what happened to this container overnight" survives a restart.
//!
//! The store is optional, the same way `crate::telemetry` treats it. If the
//! database cannot be opened the events still appear live, and the reason is
//! logged once rather than on every event.

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use tracing::{info, warn};

use crate::store::{ContainerEvent, MetricStore, StoreError};

pub use super::event_model::{
    FleetEvent, TRACKED_ACTIONS, event_from_message, is_tracked, split_action,
};

/// How many events are kept in memory, across all hosts.
///
/// Enough to fill several screens of the fleet view; the durable store holds
/// the rest.
pub const LIVE_EVENT_CAPACITY: usize = 512;

/// Live and durable container events.
pub struct DockerEvents {
    store: Option<MetricStore>,
    /// Fleet key to the store's host row id.
    host_rows: HashMap<Option<u32>, i64>,
    live: VecDeque<FleetEvent>,
    /// Set once, so a broken database does not warn on every event.
    warned_about_store: bool,
}

impl std::fmt::Debug for DockerEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerEvents")
            .field("durable", &self.store.is_some())
            .field("live", &self.live.len())
            .field("hosts", &self.host_rows.len())
            .finish()
    }
}

impl Default for DockerEvents {
    fn default() -> Self {
        Self::in_memory_only()
    }
}

impl DockerEvents {
    /// Creates a live-only instance with no durable store.
    #[must_use]
    pub fn in_memory_only() -> Self {
        Self {
            store: None,
            host_rows: HashMap::new(),
            live: VecDeque::with_capacity(LIVE_EVENT_CAPACITY),
            warned_about_store: false,
        }
    }

    /// Opens the durable store at `path`.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            store: Some(MetricStore::open(path)?),
            ..Self::in_memory_only()
        })
    }

    /// Opens the durable store at the default path, degrading to live-only.
    #[must_use]
    pub fn open_or_live_only() -> Self {
        match MetricStore::default_path().and_then(MetricStore::open) {
            Ok(store) => {
                info!("container event history: {:?}", store.path());
                Self {
                    store: Some(store),
                    ..Self::in_memory_only()
                }
            }
            Err(e) => {
                warn!("container event history is unavailable, keeping live events only: {e}");
                Self::in_memory_only()
            }
        }
    }

    /// Creates an instance backed by an in-memory database, for tests.
    ///
    /// # Errors
    /// Returns an error if the in-memory database cannot be created.
    pub fn ephemeral() -> Result<Self, StoreError> {
        Ok(Self {
            store: Some(MetricStore::open_in_memory()?),
            ..Self::in_memory_only()
        })
    }

    /// Returns true if events are being written to a database.
    #[must_use]
    pub const fn is_durable(&self) -> bool {
        self.store.is_some()
    }

    /// Records one event.
    ///
    /// Returns true if it reached the durable store. A false return is normal
    /// when there is no store; the event is still in the live ring either way.
    pub fn ingest(&mut self, event: FleetEvent) -> bool {
        if self.live.len() == LIVE_EVENT_CAPACITY {
            self.live.pop_front();
        }
        let stored = self.write_through(&event);
        self.live.push_back(event);
        stored
    }

    /// Writes one event to the store, if there is one.
    fn write_through(&mut self, event: &FleetEvent) -> bool {
        if self.store.is_none() {
            return false;
        }

        let Some(row) = self.row_for(event.host_key, &event.host_label) else {
            return false;
        };

        let record = event.to_container_event(row);
        let Some(store) = self.store.as_mut() else {
            return false;
        };

        match store.record_container_event(&record) {
            Ok(_) => true,
            Err(e) => {
                self.warn_once(&format!("could not record a container event: {e}"));
                false
            }
        }
    }

    /// Resolves the store row id for a fleet key, registering it if needed.
    fn row_for(&mut self, host_key: Option<u32>, label: &str) -> Option<i64> {
        if let Some(row) = self.host_rows.get(&host_key) {
            return Some(*row);
        }

        let now = unix_now();
        let store = self.store.as_mut()?;
        match store.register_host(label, now) {
            Ok(row) => {
                self.host_rows.insert(host_key, row);
                Some(row)
            }
            Err(e) => {
                self.warn_once(&format!("could not register host {label:?}: {e}"));
                None
            }
        }
    }

    /// Logs a store problem once per instance.
    fn warn_once(&mut self, message: &str) {
        if !self.warned_about_store {
            self.warned_about_store = true;
            warn!("{message}");
        }
    }

    /// The most recent events, newest last.
    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<&FleetEvent> {
        let skip = self.live.len().saturating_sub(limit);
        self.live.iter().skip(skip).collect()
    }

    /// Every live event for one container, oldest first.
    #[must_use]
    pub fn for_container(&self, container_id: &str) -> Vec<&FleetEvent> {
        self.live
            .iter()
            .filter(|e| e.container_id == container_id)
            .collect()
    }

    /// How many events are held live.
    #[must_use]
    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Forgets the live ring, without touching the stored history.
    pub fn clear_live(&mut self) {
        self.live.clear();
    }

    /// Stored events for one host in `[from_ts, to_ts]`, oldest first.
    ///
    /// Returns an empty vector when there is no store, which the view renders
    /// as "no history yet".
    #[must_use]
    pub fn history(&self, host_key: Option<u32>, from_ts: i64, to_ts: i64) -> Vec<ContainerEvent> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let Some(row) = self.host_rows.get(&host_key) else {
            return Vec::new();
        };
        store
            .container_events_between(*row, from_ts, to_ts)
            .unwrap_or_else(|e| {
                warn!("could not read container events: {e}");
                Vec::new()
            })
    }
}

/// Seconds since the Unix epoch.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn event(action: &str, id: &str) -> FleetEvent {
        FleetEvent {
            host_key: Some(1),
            host_label: "rock5c".to_string(),
            container_id: id.to_string(),
            container_name: Some("web".to_string()),
            ts: 1_700_000_000,
            action: action.to_string(),
            detail: None,
        }
    }

    #[test]
    fn a_live_only_recorder_keeps_events_without_a_store() {
        let mut events = DockerEvents::in_memory_only();
        assert!(!events.is_durable());
        assert!(!events.ingest(event("start", "abc")));
        assert_eq!(events.live_count(), 1);
        assert_eq!(events.recent(10).len(), 1);
        assert!(events.history(Some(1), 0, i64::MAX).is_empty());
    }

    #[test]
    fn a_durable_recorder_writes_through_and_reads_back() {
        let mut events = DockerEvents::ephemeral().expect("an in-memory store");
        assert!(events.is_durable());
        assert!(events.ingest(event("start", "abc")));
        assert!(events.ingest(event("die", "abc")));

        let history = events.history(Some(1), 0, i64::MAX);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].event, "start");
        assert_eq!(history[1].event, "die");
        assert_eq!(history[0].container_name.as_deref(), Some("web"));
    }

    #[test]
    fn two_hosts_get_two_rows_and_do_not_mix() {
        let mut events = DockerEvents::ephemeral().expect("an in-memory store");
        let mut local = event("start", "local-1");
        local.host_key = None;
        local.host_label = "Local".to_string();
        assert!(events.ingest(local));
        assert!(events.ingest(event("start", "remote-1")));

        assert_eq!(events.history(None, 0, i64::MAX).len(), 1);
        assert_eq!(events.history(Some(1), 0, i64::MAX).len(), 1);
        assert_eq!(events.history(Some(99), 0, i64::MAX).len(), 0);
    }

    #[test]
    fn the_live_ring_is_bounded_and_drops_the_oldest() {
        let mut events = DockerEvents::in_memory_only();
        for i in 0..(LIVE_EVENT_CAPACITY + 10) {
            events.ingest(event("start", &format!("c{i}")));
        }
        assert_eq!(events.live_count(), LIVE_EVENT_CAPACITY);
        let recent = events.recent(1);
        assert_eq!(
            recent[0].container_id,
            format!("c{}", LIVE_EVENT_CAPACITY + 9)
        );
    }

    #[test]
    fn events_can_be_looked_up_by_container() {
        let mut events = DockerEvents::in_memory_only();
        events.ingest(event("create", "abc"));
        events.ingest(event("start", "def"));
        events.ingest(event("die", "abc"));

        let for_abc = events.for_container("abc");
        assert_eq!(for_abc.len(), 2);
        assert_eq!(for_abc[0].action, "create");
        assert_eq!(for_abc[1].action, "die");
        assert!(events.for_container("missing").is_empty());
    }

    #[test]
    fn clearing_the_live_ring_leaves_the_stored_history() {
        let mut events = DockerEvents::ephemeral().expect("an in-memory store");
        events.ingest(event("start", "abc"));
        events.clear_live();
        assert_eq!(events.live_count(), 0);
        assert_eq!(events.history(Some(1), 0, i64::MAX).len(), 1);
    }

    #[test]
    fn a_window_that_excludes_the_event_returns_nothing() {
        let mut events = DockerEvents::ephemeral().expect("an in-memory store");
        events.ingest(event("start", "abc"));
        assert!(events.history(Some(1), 0, 1_000).is_empty());
    }

    #[test]
    fn opening_a_store_at_a_bad_path_is_an_error_the_caller_can_degrade_from() {
        let dir = std::env::temp_dir();
        // A directory is not a database file.
        assert!(DockerEvents::open(&dir).is_err());
    }

    #[test]
    fn debug_output_says_whether_events_are_durable() {
        let events = DockerEvents::in_memory_only();
        assert!(format!("{events:?}").contains("durable: false"));
    }
}
