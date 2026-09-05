//! Everything the Docker screens need, in one value.
//!
//! The fleet, the event recorder, the event feed and the per-host event
//! subscriptions all have to move together: a host that connects needs a
//! subscription, a host that fails needs its subscription dropped, and every
//! event has to reach the same recorder however many hosts are connected.
//! Bundling them means the application holds one field rather than four, and
//! that the lifecycle is written and tested once, here, rather than in the
//! event loop.

use std::collections::HashMap;

use tracing::warn;

use crate::hosts::HostRegistry;

use super::error::DockerError;
use super::event_stream::{EventFeed, EventSubscription, subscribe};
use super::events::DockerEvents;
use super::fleet::DockerFleet;
use super::fleet_rows::{FleetCounts, FleetRow, FleetSort};
use super::host::DockerHost;

pub use super::session_actions::ContainerAction;

/// How many events are taken off the feed in one pump.
///
/// A restarting host can emit hundreds in a second; taking them all would
/// stall a redraw, and the rest are still there next frame.
pub const EVENTS_PER_PUMP: usize = 64;

/// The Docker fleet, its events, and the subscriptions feeding them.
pub struct DockerFleetState {
    /// Per-host connections and data.
    pub fleet: DockerFleet,
    /// Live and durable container events.
    pub events: DockerEvents,
    feed: EventFeed,
    subscriptions: HashMap<Option<u32>, EventSubscription>,
}

impl std::fmt::Debug for DockerFleetState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerFleetState")
            .field("fleet", &self.fleet)
            .field("events", &self.events)
            .field("subscriptions", &self.subscriptions.len())
            .finish()
    }
}

impl Default for DockerFleetState {
    fn default() -> Self {
        Self::live_only()
    }
}

impl DockerFleetState {
    /// A state that keeps events in memory only.
    #[must_use]
    pub fn live_only() -> Self {
        Self {
            fleet: DockerFleet::new(),
            events: DockerEvents::in_memory_only(),
            feed: EventFeed::new(),
            subscriptions: HashMap::new(),
        }
    }

    /// A state that records events to the durable store when it can open one.
    #[must_use]
    pub fn with_history() -> Self {
        Self {
            events: DockerEvents::open_or_live_only(),
            ..Self::live_only()
        }
    }

    /// Tracks the local daemon and every SSH host that has Docker.
    ///
    /// A host whose capabilities have not been probed yet is included: not
    /// knowing is not the same as knowing it has no Docker, and the connection
    /// attempt is what settles it. Hosts that have gone from the registry are
    /// dropped, along with their subscriptions.
    pub fn sync_hosts(&mut self, registry: &HostRegistry) {
        self.fleet.track(&DockerHost::Local, "Local");

        let mut wanted: Vec<Option<u32>> = vec![None];
        for id in registry.hosts_with(|caps| {
            caps.docker || caps.podman || caps.detected_at == std::time::SystemTime::UNIX_EPOCH
        }) {
            let label = registry.label(id).unwrap_or_else(|| format!("host {id}"));
            self.fleet.track(&DockerHost::remote(id), &label);
            wanted.push(Some(id));
        }

        for key in self.fleet.keys() {
            if !wanted.contains(&key) {
                self.fleet.forget(key);
                self.subscriptions.remove(&key);
            }
        }
    }

    /// Refreshes every host, then re-syncs the event subscriptions.
    ///
    /// Returns the hosts that failed and why. One unreachable host does not
    /// stop the others.
    pub fn refresh_all(&mut self, now: i64) -> Vec<(Option<u32>, String)> {
        let failures = self.fleet.refresh_all(now);
        self.sync_subscriptions();
        failures
    }

    /// Refreshes one host and re-syncs its subscription.
    ///
    /// # Errors
    /// Returns the reason the refresh failed.
    pub fn refresh(&mut self, key: Option<u32>, now: i64) -> Result<(), DockerError> {
        let result = self.fleet.refresh(key, now);
        self.sync_subscriptions();
        result
    }

    /// Starts a subscription for every connected host that lacks one, and
    /// drops the ones whose host is gone or whose stream has ended.
    pub fn sync_subscriptions(&mut self) {
        self.subscriptions
            .retain(|key, subscription| match self.fleet.host(*key) {
                Some(host) => host.connection.is_connected() && subscription.is_running(),
                None => false,
            });

        for key in self.fleet.keys() {
            if self.subscriptions.contains_key(&key) {
                continue;
            }
            let Some(host) = self.fleet.host(key) else {
                continue;
            };
            if !host.connection.is_connected() {
                continue;
            }
            let Some(client) = host.client() else {
                continue;
            };
            let label = host.label.clone();

            match subscribe(client, key, &label, self.feed.sender()) {
                Ok(subscription) => {
                    self.subscriptions.insert(key, subscription);
                }
                Err(e) => warn!("could not watch {label} for container events: {e}"),
            }
        }
    }

    /// Moves whatever the subscriptions produced into the recorder.
    ///
    /// Call once per frame. Returns how many events were taken.
    pub fn pump_events(&mut self) -> usize {
        let drained = self.feed.drain(EVENTS_PER_PUMP);
        let count = drained.len();
        for event in drained {
            self.events.ingest(event);
        }
        count
    }

    /// How many hosts are being watched for events.
    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Stops every subscription.
    pub fn stop_watching(&mut self) {
        self.subscriptions.clear();
    }

    /// The fleet list, sorted and filtered.
    #[must_use]
    pub fn rows(&self) -> Vec<FleetRow> {
        self.fleet.rows()
    }

    /// The header counts.
    #[must_use]
    pub fn counts(&self) -> FleetCounts {
        self.fleet.counts()
    }

    /// Moves to the next sort order.
    pub fn cycle_sort(&mut self) -> FleetSort {
        self.fleet.cycle_sort()
    }

    /// Sets the fleet filter.
    pub fn set_filter(&mut self, query: impl Into<String>) {
        self.fleet.set_filter(query);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::docker::events::FleetEvent;

    fn event(id: &str) -> FleetEvent {
        FleetEvent {
            host_key: None,
            host_label: "Local".to_string(),
            container_id: id.to_string(),
            container_name: None,
            ts: 1_700_000_000,
            action: "start".to_string(),
            detail: None,
        }
    }

    #[test]
    fn a_new_state_watches_nothing_and_has_no_events() {
        let state = DockerFleetState::live_only();
        assert!(state.fleet.is_empty());
        assert_eq!(state.subscription_count(), 0);
        assert_eq!(state.events.live_count(), 0);
        assert!(state.rows().is_empty());
        assert!(!state.events.is_durable());
    }

    #[test]
    fn syncing_an_empty_registry_still_tracks_the_local_daemon() {
        let mut state = DockerFleetState::live_only();
        state.sync_hosts(&HostRegistry::new());
        assert_eq!(state.fleet.keys(), vec![None]);
        assert_eq!(state.counts().hosts, 1);
    }

    #[test]
    fn syncing_twice_does_not_duplicate_hosts() {
        let mut state = DockerFleetState::live_only();
        let registry = HostRegistry::new();
        state.sync_hosts(&registry);
        state.sync_hosts(&registry);
        assert_eq!(state.fleet.len(), 1);
    }

    #[test]
    fn a_host_that_leaves_the_registry_is_forgotten_with_its_subscription() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::remote(42), "gone-tomorrow");
        assert_eq!(state.fleet.len(), 1);

        state.sync_hosts(&HostRegistry::new());
        assert_eq!(
            state.fleet.keys(),
            vec![None],
            "only the local daemon remains"
        );
        assert_eq!(state.subscription_count(), 0);
    }

    #[test]
    fn pumping_an_empty_feed_takes_nothing() {
        let mut state = DockerFleetState::live_only();
        assert_eq!(state.pump_events(), 0);
        assert_eq!(state.events.live_count(), 0);
    }

    #[test]
    fn pumped_events_reach_the_recorder() {
        let mut state = DockerFleetState::live_only();
        let sender = state.feed.sender();
        sender.send(event("a")).expect("the feed is alive");
        sender.send(event("b")).expect("the feed is alive");

        assert_eq!(state.pump_events(), 2);
        assert_eq!(state.events.live_count(), 2);
        assert_eq!(state.pump_events(), 0, "a pump consumes what it takes");
    }

    #[test]
    fn a_pump_is_capped_so_a_burst_cannot_stall_a_frame() {
        let mut state = DockerFleetState::live_only();
        let sender = state.feed.sender();
        for i in 0..(EVENTS_PER_PUMP + 5) {
            sender
                .send(event(&i.to_string()))
                .expect("the feed is alive");
        }
        assert_eq!(state.pump_events(), EVENTS_PER_PUMP);
        assert_eq!(state.pump_events(), 5);
    }

    #[test]
    fn syncing_subscriptions_with_no_connected_host_starts_none() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::Local, "Local");
        state.sync_subscriptions();
        assert_eq!(state.subscription_count(), 0);
    }

    #[test]
    fn refreshing_an_unreachable_fleet_reports_it_and_starts_no_subscription() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::remote(994_421), "ghost");
        let failures = state.refresh_all(100);
        assert_eq!(failures.len(), 1);
        assert_eq!(state.subscription_count(), 0);
        assert_eq!(state.counts().failed, 1);
    }

    #[test]
    fn refreshing_one_unreachable_host_returns_its_reason() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::remote(994_422), "ghost");
        let error = state.refresh(Some(994_422), 100).expect_err("unreachable");
        assert!(matches!(error, DockerError::UnknownHost(_)));
    }

    #[test]
    fn the_filter_and_sort_pass_through_to_the_fleet() {
        let mut state = DockerFleetState::live_only();
        state.set_filter("web");
        assert_eq!(state.fleet.filter(), "web");
        assert_eq!(state.cycle_sort(), FleetSort::Name);
    }

    #[test]
    fn stopping_watching_drops_every_subscription() {
        let mut state = DockerFleetState::live_only();
        state.stop_watching();
        assert_eq!(state.subscription_count(), 0);
    }

    #[test]
    fn debug_output_summarises_the_state() {
        let state = DockerFleetState::live_only();
        let rendered = format!("{state:?}");
        assert!(rendered.contains("subscriptions: 0"), "{rendered}");
    }
}
