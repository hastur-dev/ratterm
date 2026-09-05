//! Subscribing to one host's Docker event stream.
//!
//! The daemon's `/events` endpoint is a long-lived stream, so it cannot be
//! polled from the render loop. A subscription runs on the Docker runtime and
//! pushes converted events down a `std::sync::mpsc` channel; the UI drains the
//! channel each frame and hands what it finds to [`super::events::DockerEvents::ingest`],
//! the same way the background image pull already works.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use bollard::system::EventsOptions;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

use super::client::DockerClient;
use super::error::DockerError;
use super::events::{FleetEvent, event_from_message};
use super::runtime::runtime;

/// A running subscription to one host's event stream.
///
/// Dropping this asks the stream task to stop at its next message.
pub struct EventSubscription {
    host_key: Option<u32>,
    host_label: String,
    stop: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl std::fmt::Debug for EventSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSubscription")
            .field("host", &self.host_label)
            .field("running", &self.is_running())
            .finish()
    }
}

impl EventSubscription {
    /// The fleet key this subscription belongs to.
    #[must_use]
    pub const fn host_key(&self) -> Option<u32> {
        self.host_key
    }

    /// The host's display name.
    #[must_use]
    pub fn host_label(&self) -> &str {
        &self.host_label
    }

    /// True while the stream task is still alive.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.handle.is_finished()
    }

    /// Asks the stream task to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The receiving half of a fleet-wide event feed.
///
/// One of these is shared by every host: the sender is cloned per
/// subscription, so the UI drains a single channel however many hosts are
/// connected.
#[derive(Debug)]
pub struct EventFeed {
    sender: Sender<FleetEvent>,
    receiver: Receiver<FleetEvent>,
}

impl Default for EventFeed {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFeed {
    /// Creates an empty feed.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }

    /// A sender for one more subscription.
    #[must_use]
    pub fn sender(&self) -> Sender<FleetEvent> {
        self.sender.clone()
    }

    /// Takes everything that has arrived since the last call.
    ///
    /// Never blocks, so it is safe to call once per frame. Stops at `limit`
    /// so a burst cannot stall a redraw.
    #[must_use]
    pub fn drain(&self, limit: usize) -> Vec<FleetEvent> {
        let mut drained = Vec::new();
        while drained.len() < limit {
            match self.receiver.try_recv() {
                Ok(event) => drained.push(event),
                // The feed owns a sender of its own, so `Disconnected` cannot
                // happen while the feed is alive; treat it as "nothing more".
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        drained
    }
}

/// Starts streaming one host's container events into `sender`.
///
/// # Errors
/// Returns [`DockerError::Runtime`] if the Docker runtime cannot be started.
pub fn subscribe(
    client: Arc<DockerClient>,
    host_key: Option<u32>,
    host_label: &str,
    sender: Sender<FleetEvent>,
) -> Result<EventSubscription, DockerError> {
    let runtime = runtime()?;
    let stop = Arc::new(AtomicBool::new(false));

    let task_stop = stop.clone();
    let task_label = host_label.to_string();
    let stream_label = task_label.clone();

    let handle = runtime.spawn(async move {
        let options = EventsOptions::<String> {
            filters: std::collections::HashMap::from([(
                "type".to_string(),
                vec!["container".to_string()],
            )]),
            ..Default::default()
        };

        let mut stream = client.inner().events(Some(options));

        while let Some(message) = stream.next().await {
            if task_stop.load(Ordering::SeqCst) {
                break;
            }

            match message {
                Ok(message) => {
                    if let Some(event) = event_from_message(&message, host_key, &stream_label)
                        && sender.send(event).is_err()
                    {
                        // Nobody is draining any more; the UI has gone.
                        break;
                    }
                }
                Err(e) => {
                    warn!("the Docker event stream for {stream_label} ended: {e}");
                    break;
                }
            }
        }

        debug!("event subscription for {stream_label} stopped");
    });

    Ok(EventSubscription {
        host_key,
        host_label: task_label,
        stop,
        handle,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn event(id: &str) -> FleetEvent {
        FleetEvent {
            host_key: Some(1),
            host_label: "rock5c".to_string(),
            container_id: id.to_string(),
            container_name: None,
            ts: 1_700_000_000,
            action: "start".to_string(),
            detail: None,
        }
    }

    #[test]
    fn an_empty_feed_drains_to_nothing() {
        let feed = EventFeed::new();
        assert!(feed.drain(10).is_empty());
    }

    #[test]
    fn a_feed_hands_back_what_was_sent_in_order() {
        let feed = EventFeed::new();
        let sender = feed.sender();
        sender.send(event("a")).expect("the feed is alive");
        sender.send(event("b")).expect("the feed is alive");

        let drained = feed.drain(10);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].container_id, "a");
        assert_eq!(drained[1].container_id, "b");
        assert!(feed.drain(10).is_empty(), "a drain consumes what it takes");
    }

    #[test]
    fn draining_stops_at_the_limit_and_leaves_the_rest() {
        let feed = EventFeed::new();
        let sender = feed.sender();
        for i in 0..5 {
            sender
                .send(event(&i.to_string()))
                .expect("the feed is alive");
        }
        assert_eq!(feed.drain(2).len(), 2);
        assert_eq!(feed.drain(10).len(), 3);
    }

    #[test]
    fn several_hosts_can_share_one_feed() {
        let feed = EventFeed::new();
        let first = feed.sender();
        let second = feed.sender();
        first.send(event("from-first")).expect("alive");
        second.send(event("from-second")).expect("alive");
        assert_eq!(feed.drain(10).len(), 2);
    }

    #[test]
    fn a_sender_that_outlives_its_host_still_works_because_the_feed_holds_one() {
        let feed = EventFeed::new();
        {
            let sender = feed.sender();
            sender.send(event("a")).expect("alive");
        }
        assert_eq!(feed.drain(10).len(), 1);
        // Dropping every clone must not turn later drains into errors.
        assert!(feed.drain(10).is_empty());
    }

    #[test]
    fn a_subscription_against_a_dead_endpoint_stops_on_its_own() {
        use super::super::transport::TransportChoice;

        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client =
            Arc::new(DockerClient::connect_with(&choice).expect("http clients build lazily"));
        let feed = EventFeed::new();
        let subscription =
            subscribe(client, Some(3), "dead-host", feed.sender()).expect("the runtime starts");

        assert_eq!(subscription.host_key(), Some(3));
        assert_eq!(subscription.host_label(), "dead-host");

        // Nothing listens on port 1, so the stream errors and the task ends.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while subscription.is_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!subscription.is_running(), "the task must not run forever");
        assert!(feed.drain(10).is_empty(), "a dead host produces no events");
        assert!(format!("{subscription:?}").contains("dead-host"));
    }

    #[test]
    fn stopping_a_subscription_is_idempotent() {
        use super::super::transport::TransportChoice;

        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client =
            Arc::new(DockerClient::connect_with(&choice).expect("http clients build lazily"));
        let feed = EventFeed::new();
        let subscription =
            subscribe(client, None, "local", feed.sender()).expect("the runtime starts");
        subscription.stop();
        subscription.stop();
    }
}
