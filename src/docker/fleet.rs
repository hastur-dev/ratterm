//! Several Docker hosts at once.
//!
//! The Docker manager used to have one active host: switching machines threw
//! the previous host's containers away and there was no way to see the fleet.
//! This keeps every host's data side by side, each tagged with the host it came
//! from, with its own connection state. A host that is unreachable records why
//! and is skipped; it never stops the others from refreshing.

use std::collections::BTreeMap;
use std::sync::Arc;

use tracing::warn;

use super::client::{DockerClient, HostSnapshot, probe};
use super::compose::ComposeGrouping;
use super::error::DockerError;
use super::fleet_rows::{FleetCounts, FleetRow, FleetSort, filter_rows, sort_rows};
use super::host::DockerHost;
use super::transport::choose_transport;

pub use super::fleet_host::{FleetHost, HostConnection};

/// Every Docker host being watched.
pub struct DockerFleet {
    hosts: BTreeMap<Option<u32>, FleetHost>,
    sort: FleetSort,
    filter: String,
}

impl std::fmt::Debug for DockerFleet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerFleet")
            .field("hosts", &self.hosts.len())
            .field("sort", &self.sort)
            .field("filter", &self.filter)
            .finish()
    }
}

impl Default for DockerFleet {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerFleet {
    /// An empty fleet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hosts: BTreeMap::new(),
            sort: FleetSort::default(),
            filter: String::new(),
        }
    }

    /// Adds a host, or renames one already tracked.
    ///
    /// Re-tracking an existing host keeps its connection and its data, so a
    /// host list refresh does not drop the fleet on the floor.
    pub fn track(&mut self, host: &DockerHost, label: &str) {
        let key = host.fleet_key();
        match self.hosts.get_mut(&key) {
            Some(existing) => existing.label = label.to_string(),
            None => {
                self.hosts.insert(key, FleetHost::idle(key, label));
            }
        }
    }

    /// Stops watching a host and drops its connection.
    pub fn forget(&mut self, key: Option<u32>) -> bool {
        self.hosts.remove(&key).is_some()
    }

    /// Stops watching every host.
    pub fn clear(&mut self) {
        self.hosts.clear();
    }

    /// The hosts, local first then by SSH host id.
    pub fn hosts(&self) -> impl Iterator<Item = &FleetHost> {
        self.hosts.values()
    }

    /// The fleet keys, in the same order.
    pub fn keys(&self) -> Vec<Option<u32>> {
        self.hosts.keys().copied().collect()
    }

    /// One host.
    #[must_use]
    pub fn host(&self, key: Option<u32>) -> Option<&FleetHost> {
        self.hosts.get(&key)
    }

    /// One host's client, if it is connected.
    #[must_use]
    pub fn client(&self, key: Option<u32>) -> Option<Arc<DockerClient>> {
        self.hosts.get(&key).and_then(FleetHost::client)
    }

    /// How many hosts are tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    /// True when no host is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Marks a host as being connected to.
    pub fn mark_connecting(&mut self, key: Option<u32>) {
        if let Some(host) = self.hosts.get_mut(&key) {
            host.connection = HostConnection::Connecting;
        }
    }

    /// Records a failed connection without touching any other host.
    pub fn mark_failed(&mut self, key: Option<u32>, reason: String, now: i64) {
        if let Some(host) = self.hosts.get_mut(&key) {
            host.client = None;
            host.connection = HostConnection::Failed { reason, at: now };
        }
    }

    /// Connects to one host, replacing any client it already had.
    ///
    /// Returns the error as well as recording it, so a caller can put it in
    /// the status bar. Every other host is left exactly as it was.
    ///
    /// # Errors
    /// Returns the reason the daemon could not be reached.
    pub fn connect(&mut self, key: Option<u32>, now: i64) -> Result<(), DockerError> {
        if !self.hosts.contains_key(&key) {
            return Err(DockerError::UnknownHost(key.unwrap_or(0)));
        }

        self.mark_connecting(key);

        let docker_host = DockerHost::from_fleet_key(key);
        let choice = choose_transport(&probe(&docker_host));
        let rationale = choice.rationale();

        match DockerClient::connect_with(&choice) {
            Ok(client) => {
                let transport = client.transport().to_string();
                if let Some(host) = self.hosts.get_mut(&key) {
                    host.client = Some(Arc::new(client));
                    host.connection = HostConnection::Connected {
                        transport,
                        rationale,
                        since: now,
                    };
                }
                Ok(())
            }
            Err(e) => {
                self.mark_failed(key, e.to_string(), now);
                Err(e)
            }
        }
    }

    /// Refreshes one host's containers, images, volumes and networks.
    ///
    /// Connects first if the host has no client yet.
    ///
    /// # Errors
    /// Returns the reason the refresh failed.
    pub fn refresh(&mut self, key: Option<u32>, now: i64) -> Result<(), DockerError> {
        if self.client(key).is_none() {
            self.connect(key, now)?;
        }

        let Some(client) = self.client(key) else {
            return Err(DockerError::UnknownHost(key.unwrap_or(0)));
        };

        match client.snapshot_blocking() {
            Ok(snapshot) => {
                if let Some(host) = self.hosts.get_mut(&key) {
                    host.snapshot = snapshot;
                    host.last_refresh = Some(now);
                }
                Ok(())
            }
            Err(e) => {
                self.mark_failed(key, e.to_string(), now);
                Err(e)
            }
        }
    }

    /// Records a snapshot obtained somewhere else, marking the host connected.
    ///
    /// [`DockerFleet::refresh`] blocks until the daemon answers, which is too
    /// long to spend on the render thread once there are several hosts. A
    /// caller that refreshed on a worker thread hands the result back here.
    /// Returns false if the host is not tracked.
    pub fn record_snapshot(
        &mut self,
        key: Option<u32>,
        transport: String,
        rationale: &'static str,
        snapshot: HostSnapshot,
        now: i64,
    ) -> bool {
        let Some(host) = self.hosts.get_mut(&key) else {
            return false;
        };
        host.connection = HostConnection::Connected {
            transport,
            rationale,
            since: now,
        };
        host.snapshot = snapshot;
        host.last_refresh = Some(now);
        true
    }

    /// Refreshes every host, reporting the ones that failed.
    ///
    /// One unreachable host does not stop the rest: each is attempted and its
    /// own failure recorded against it.
    pub fn refresh_all(&mut self, now: i64) -> Vec<(Option<u32>, String)> {
        let mut failures = Vec::new();
        for key in self.keys() {
            if let Err(e) = self.refresh(key, now) {
                warn!("Docker host {key:?} could not be refreshed: {e}");
                failures.push((key, e.to_string()));
            }
        }
        failures
    }

    /// The order the fleet list is in.
    #[must_use]
    pub const fn sort(&self) -> FleetSort {
        self.sort
    }

    /// Sets the order.
    pub fn set_sort(&mut self, sort: FleetSort) {
        self.sort = sort;
    }

    /// Moves to the next order, for a key that cycles.
    pub fn cycle_sort(&mut self) -> FleetSort {
        self.sort = self.sort.next();
        self.sort
    }

    /// The filter query in force.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Sets the filter query.
    pub fn set_filter(&mut self, query: impl Into<String>) {
        self.filter = query.into();
    }

    /// Every container on every connected host, sorted and filtered.
    #[must_use]
    pub fn rows(&self) -> Vec<FleetRow> {
        let mut rows: Vec<FleetRow> = self
            .hosts
            .values()
            .filter(|host| host.connection.is_connected())
            .flat_map(|host| FleetRow::for_host(host.key, &host.label, &host.snapshot.containers))
            .collect();

        sort_rows(&mut rows, self.sort);
        filter_rows(&rows, &self.filter)
    }

    /// Every container on every connected host, ignoring the filter.
    #[must_use]
    pub fn all_rows(&self) -> Vec<FleetRow> {
        let mut rows: Vec<FleetRow> = self
            .hosts
            .values()
            .filter(|host| host.connection.is_connected())
            .flat_map(|host| FleetRow::for_host(host.key, &host.label, &host.snapshot.containers))
            .collect();
        sort_rows(&mut rows, self.sort);
        rows
    }

    /// The header counts.
    #[must_use]
    pub fn counts(&self) -> FleetCounts {
        let mut counts = FleetCounts {
            hosts: self.hosts.len(),
            ..FleetCounts::default()
        };

        for host in self.hosts.values() {
            if host.connection.is_failed() {
                counts.failed += 1;
            }
            if host.connection.is_connected() {
                counts.connected += 1;
                counts.containers += host.snapshot.containers.len();
                counts.running += host.snapshot.running_count();
            }
        }

        counts
    }

    /// One host's Compose projects.
    #[must_use]
    pub fn compose(&self, key: Option<u32>) -> Option<ComposeGrouping> {
        self.hosts.get(&key).map(FleetHost::compose)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::container::DockerContainer;
    use super::super::model::ContainerDetail;
    use super::*;
    use std::collections::HashMap;

    fn detail(name: &str, status: &str) -> ContainerDetail {
        ContainerDetail {
            container: DockerContainer::new(
                format!("id-{name}"),
                name.to_string(),
                "img".to_string(),
                status.to_string(),
            ),
            labels: HashMap::new(),
            state: status.to_lowercase(),
            command: String::new(),
        }
    }

    /// Puts a host into the connected state with data, without a daemon.
    fn connect_with_data(
        fleet: &mut DockerFleet,
        key: Option<u32>,
        containers: Vec<ContainerDetail>,
    ) {
        let recorded = fleet.record_snapshot(
            key,
            "unix socket /var/run/docker.sock".to_string(),
            "the local daemon socket is present",
            HostSnapshot {
                containers,
                ..Default::default()
            },
            100,
        );
        assert!(recorded, "the host must be tracked first");
    }

    #[test]
    fn recording_a_snapshot_for_an_untracked_host_is_refused() {
        let mut fleet = DockerFleet::new();
        assert!(!fleet.record_snapshot(
            Some(9),
            "unix socket".to_string(),
            "test",
            HostSnapshot::default(),
            1
        ));
    }

    #[test]
    fn a_new_fleet_is_empty() {
        let fleet = DockerFleet::new();
        assert!(fleet.is_empty());
        assert_eq!(fleet.len(), 0);
        assert!(fleet.rows().is_empty());
        assert_eq!(fleet.counts(), FleetCounts::default());
        assert!(fleet.client(None).is_none());
    }

    #[test]
    fn hosts_are_tracked_local_first() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::remote(5), "rock5c");
        fleet.track(&DockerHost::Local, "Local");
        fleet.track(&DockerHost::remote(2), "cthulhu");

        assert_eq!(fleet.keys(), vec![None, Some(2), Some(5)]);
        assert_eq!(fleet.len(), 3);
        assert_eq!(
            fleet.host(Some(2)).map(|h| h.label.as_str()),
            Some("cthulhu")
        );
    }

    #[test]
    fn re_tracking_a_host_renames_it_and_keeps_its_data() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::Local, "Local");
        connect_with_data(&mut fleet, None, vec![detail("a", "Up")]);

        fleet.track(&DockerHost::Local, "This machine");
        let host = fleet.host(None).expect("still tracked");
        assert_eq!(host.label, "This machine");
        assert_eq!(host.snapshot.containers.len(), 1);
        assert!(host.connection.is_connected());
    }

    #[test]
    fn forgetting_a_host_removes_it_and_reports_whether_it_was_there() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::remote(1), "a");
        assert!(fleet.forget(Some(1)));
        assert!(!fleet.forget(Some(1)));
        assert!(fleet.is_empty());
    }

    #[test]
    fn clearing_removes_every_host() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::Local, "Local");
        fleet.track(&DockerHost::remote(1), "a");
        fleet.clear();
        assert!(fleet.is_empty());
    }

    #[test]
    fn a_tracked_host_starts_idle_and_contributes_no_rows() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::remote(1), "rock5c");
        let host = fleet.host(Some(1)).expect("tracked");
        assert_eq!(host.connection, HostConnection::Idle);
        assert!(host.connection.detail().is_none());
        assert!(fleet.rows().is_empty(), "an idle host shows nothing");
        assert_eq!(fleet.counts().connected, 0);
        assert_eq!(fleet.counts().hosts, 1);
    }

    #[test]
    fn connecting_to_a_host_that_is_not_tracked_is_refused() {
        let mut fleet = DockerFleet::new();
        assert!(matches!(
            fleet.connect(Some(3), 100),
            Err(DockerError::UnknownHost(3))
        ));
        assert!(matches!(
            fleet.refresh(Some(3), 100),
            Err(DockerError::UnknownHost(3))
        ));
    }

    #[test]
    fn marking_a_host_connecting_only_touches_that_host() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::Local, "Local");
        fleet.track(&DockerHost::remote(1), "rock5c");
        fleet.mark_connecting(Some(1));
        assert_eq!(
            fleet.host(Some(1)).map(|h| h.connection.clone()),
            Some(HostConnection::Connecting)
        );
        assert_eq!(
            fleet.host(None).map(|h| h.connection.clone()),
            Some(HostConnection::Idle)
        );
        // An untracked key is a no-op rather than a panic.
        fleet.mark_connecting(Some(77));
        fleet.mark_failed(Some(77), "gone".to_string(), 1);
    }

    #[test]
    fn debug_output_summarises_the_fleet() {
        let mut fleet = DockerFleet::new();
        fleet.track(&DockerHost::Local, "Local");
        let rendered = format!("{fleet:?}");
        assert!(rendered.contains("hosts: 1"), "{rendered}");
    }
}
