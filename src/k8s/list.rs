//! Listing and watching cluster resources.
//!
//! Every list returns owned views in a stable order, so a refresh that changes
//! nothing leaves the selected row where it was. The comparators live with the
//! views in [`super::resources`] and are tested there.
//!
//! Watching runs on the Kubernetes runtime and posts changes to a plain
//! [`std::sync::mpsc`] channel. The UI thread drains it without blocking, so a
//! dashboard updates from the watch instead of re-listing on a timer.

use std::cmp::Ordering;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use k8s_openapi::NamespaceResourceScope;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod, Service};
use kube::api::{Api, ListParams};
use kube::runtime::watcher::{self, watcher};
use serde::de::DeserializeOwned;
use tokio_stream::StreamExt;

use super::client::K8sClient;
use super::resources::{
    DeploymentView, EventView, NodeView, PodView, ServiceView, compare_deployments, compare_events,
    compare_nodes, compare_pods, compare_services,
};
use super::{K8sError, Result, block_on, runtime};

/// Upper bound on how many events one watch will deliver before it stops.
///
/// The loop is over network input, so it needs a bound as well as the stop
/// flag. At this size a watch would have to run for weeks on a busy cluster to
/// reach it, and the caller can start another.
const MAX_WATCH_EVENTS: u64 = 5_000_000;

/// How long a watch waits after an error before polling again.
///
/// `kube`'s watcher recovers on the next poll; without a pause a cluster that
/// keeps rejecting the watch would be polled in a tight loop.
const WATCH_ERROR_BACKOFF: Duration = Duration::from_secs(2);

/// How many updates [`WatchHandle::drain`] will take in one call by default.
const DEFAULT_DRAIN_LIMIT: usize = 512;

/// One change reported by a watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchUpdate<T> {
    /// The watch (re)started; everything delivered until [`WatchUpdate::InitDone`]
    /// is the current state, not a change.
    Restarted,
    /// An object was added or changed.
    Applied(T),
    /// An object was removed.
    Deleted(T),
    /// The initial listing is complete.
    InitDone,
    /// The watch failed. It recovers on its own; this is for the status bar.
    Failed(String),
}

/// A running watch.
///
/// Dropping this stops the watch.
pub struct WatchHandle<T> {
    rx: Receiver<WatchUpdate<T>>,
    stop: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
    description: String,
}

impl<T> Debug for WatchHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchHandle")
            .field("watching", &self.description)
            .field("running", &self.is_running())
            .finish()
    }
}

impl<T> WatchHandle<T> {
    /// Returns the next update, or `None` when there is nothing waiting.
    ///
    /// Never blocks, so this is safe to call from the render loop.
    pub fn try_recv(&self) -> Option<WatchUpdate<T>> {
        self.rx.try_recv().ok()
    }

    /// Takes up to `limit` waiting updates.
    ///
    /// Bounded so a burst of cluster activity cannot make one frame take
    /// arbitrarily long.
    pub fn drain(&self, limit: usize) -> Vec<WatchUpdate<T>> {
        let mut updates = Vec::new();
        for _ in 0..limit {
            match self.try_recv() {
                Some(update) => updates.push(update),
                None => break,
            }
        }
        updates
    }

    /// Takes up to [`DEFAULT_DRAIN_LIMIT`] waiting updates.
    pub fn drain_available(&self) -> Vec<WatchUpdate<T>> {
        self.drain(DEFAULT_DRAIN_LIMIT)
    }

    /// True while the watch is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.stop.load(AtomicOrdering::SeqCst)
            && self.task.as_ref().is_none_or(|t| !t.is_finished())
    }

    /// Returns what this watch is watching, for the status bar.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Stops the watch.
    pub fn stop(&mut self) {
        self.stop.store(true, AtomicOrdering::SeqCst);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }

    /// Builds a handle around an existing channel, with no task behind it.
    #[cfg(test)]
    fn detached(rx: Receiver<WatchUpdate<T>>, stop: Arc<AtomicBool>, description: &str) -> Self {
        Self {
            rx,
            stop,
            task: None,
            description: description.to_string(),
        }
    }
}

impl<T> Drop for WatchHandle<T> {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Describes a namespace selection for an error message.
///
/// `None` means every namespace, which is what the API calls a cluster-wide
/// list.
#[must_use]
pub fn describe_scope(namespace: Option<&str>) -> String {
    match namespace {
        Some(ns) if !ns.is_empty() => format!("namespace {ns}"),
        _ => "all namespaces".to_string(),
    }
}

/// Builds an `Api` for a namespace, or for the whole cluster when there is
/// none.
fn scoped_api<K>(client: &kube::Client, namespace: Option<&str>) -> Api<K>
where
    K: kube::Resource<Scope = NamespaceResourceScope, DynamicType = ()>,
{
    match namespace {
        Some(ns) if !ns.is_empty() => Api::namespaced(client.clone(), ns),
        _ => Api::all(client.clone()),
    }
}

impl K8sClient {
    /// Lists every namespace name, sorted.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_namespaces(&self) -> Result<Vec<String>> {
        let api: Api<Namespace> = Api::all(self.inner().clone());
        let items = self.fetch(api, "list namespaces")?;
        let mut names: Vec<String> = items
            .into_iter()
            .filter_map(|ns| ns.metadata.name)
            .collect();
        names.sort();
        Ok(names)
    }

    /// Lists pods in a namespace, or in every namespace when `namespace` is
    /// `None`.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_pods(&self, namespace: Option<&str>) -> Result<Vec<PodView>> {
        let api: Api<Pod> = scoped_api(self.inner(), namespace);
        let action = format!("list pods in {}", describe_scope(namespace));
        let items = self.fetch(api, &action)?;
        Ok(convert_sorted(&items, PodView::from_api, compare_pods))
    }

    /// Lists deployments in a namespace, or in every namespace.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_deployments(&self, namespace: Option<&str>) -> Result<Vec<DeploymentView>> {
        let api: Api<Deployment> = scoped_api(self.inner(), namespace);
        let action = format!("list deployments in {}", describe_scope(namespace));
        let items = self.fetch(api, &action)?;
        Ok(convert_sorted(
            &items,
            DeploymentView::from_api,
            compare_deployments,
        ))
    }

    /// Lists services in a namespace, or in every namespace.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_services(&self, namespace: Option<&str>) -> Result<Vec<ServiceView>> {
        let api: Api<Service> = scoped_api(self.inner(), namespace);
        let action = format!("list services in {}", describe_scope(namespace));
        let items = self.fetch(api, &action)?;
        Ok(convert_sorted(
            &items,
            ServiceView::from_api,
            compare_services,
        ))
    }

    /// Lists the cluster's nodes.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_nodes(&self) -> Result<Vec<NodeView>> {
        let api: Api<Node> = Api::all(self.inner().clone());
        let items = self.fetch(api, "list nodes")?;
        Ok(convert_sorted(&items, NodeView::from_api, compare_nodes))
    }

    /// Lists events in a namespace, or in every namespace, newest first.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn list_events(&self, namespace: Option<&str>) -> Result<Vec<EventView>> {
        let api: Api<Event> = scoped_api(self.inner(), namespace);
        let action = format!("list events in {}", describe_scope(namespace));
        let items = self.fetch(api, &action)?;
        Ok(convert_sorted(&items, EventView::from_api, compare_events))
    }

    /// Watches pods and posts changes to a channel.
    ///
    /// # Errors
    /// [`K8sError::Runtime`] if the watch task cannot be started.
    pub fn watch_pods(&self, namespace: Option<&str>) -> Result<WatchHandle<PodView>> {
        let api: Api<Pod> = scoped_api(self.inner(), namespace);
        let description = format!("pods in {}", describe_scope(namespace));
        spawn_watch(api, PodView::from_api, description)
    }

    /// Watches events and posts changes to a channel.
    ///
    /// # Errors
    /// [`K8sError::Runtime`] if the watch task cannot be started.
    pub fn watch_events(&self, namespace: Option<&str>) -> Result<WatchHandle<EventView>> {
        let api: Api<Event> = scoped_api(self.inner(), namespace);
        let description = format!("events in {}", describe_scope(namespace));
        spawn_watch(api, EventView::from_api, description)
    }

    /// Runs one list call and returns the raw items.
    fn fetch<K>(&self, api: Api<K>, action: &str) -> Result<Vec<K>>
    where
        K: Clone + DeserializeOwned + Debug,
    {
        let context = self.context().to_string();
        let list = block_on(async move { api.list(&ListParams::default()).await })?
            .map_err(|e| K8sError::from_kube(&e, &context, action))?;
        Ok(list.items)
    }
}

/// Converts a list of API objects into views and sorts them.
fn convert_sorted<K, V>(
    items: &[K],
    convert: impl Fn(&K) -> V,
    compare: impl Fn(&V, &V) -> Ordering,
) -> Vec<V> {
    let mut views: Vec<V> = items.iter().map(convert).collect();
    views.sort_by(compare);
    views
}

/// Starts a watch task and returns its handle.
fn spawn_watch<K, V>(
    api: Api<K>,
    convert: fn(&K) -> V,
    description: String,
) -> Result<WatchHandle<V>>
where
    K: kube::Resource + Clone + DeserializeOwned + Debug + Send + 'static,
    K::DynamicType: Default,
    V: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let task_stop = stop.clone();

    let task = runtime()?.spawn(async move {
        watch_loop(api, convert, tx, task_stop).await;
    });

    Ok(WatchHandle {
        rx,
        stop,
        task: Some(task),
        description,
    })
}

/// Drives one watch until it is stopped, the receiver goes away, or the event
/// cap is reached.
async fn watch_loop<K, V>(
    api: Api<K>,
    convert: fn(&K) -> V,
    tx: Sender<WatchUpdate<V>>,
    stop: Arc<AtomicBool>,
) where
    K: kube::Resource + Clone + DeserializeOwned + Debug + Send + 'static,
    K::DynamicType: Default,
{
    let mut stream = Box::pin(watcher(api, watcher::Config::default()));

    for _ in 0..MAX_WATCH_EVENTS {
        if stop.load(AtomicOrdering::SeqCst) {
            break;
        }

        let Some(next) = stream.next().await else {
            break;
        };

        let update = match next {
            Ok(watcher::Event::Init) => WatchUpdate::Restarted,
            Ok(watcher::Event::InitApply(object) | watcher::Event::Apply(object)) => {
                WatchUpdate::Applied(convert(&object))
            }
            Ok(watcher::Event::Delete(object)) => WatchUpdate::Deleted(convert(&object)),
            Ok(watcher::Event::InitDone) => WatchUpdate::InitDone,
            Err(e) => {
                let failure = WatchUpdate::Failed(e.to_string());
                if tx.send(failure).is_err() {
                    break;
                }
                tokio::time::sleep(WATCH_ERROR_BACKOFF).await;
                continue;
            }
        };

        if tx.send(update).is_err() {
            // The handle was dropped; nothing is reading any more.
            break;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn handle() -> (Sender<WatchUpdate<u32>>, WatchHandle<u32>, Arc<AtomicBool>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let handle = WatchHandle::detached(rx, stop.clone(), "pods in namespace web");
        (tx, handle, stop)
    }

    #[test]
    fn a_scope_with_a_namespace_names_it() {
        assert_eq!(describe_scope(Some("web")), "namespace web");
    }

    #[test]
    fn an_absent_or_empty_namespace_means_every_namespace() {
        assert_eq!(describe_scope(None), "all namespaces");
        assert_eq!(describe_scope(Some("")), "all namespaces");
    }

    #[test]
    fn an_empty_watch_yields_nothing_without_blocking() {
        let (_tx, handle, _stop) = handle();
        assert!(handle.try_recv().is_none());
        assert!(handle.drain(10).is_empty());
    }

    #[test]
    fn updates_arrive_in_order() {
        let (tx, handle, _stop) = handle();
        tx.send(WatchUpdate::Restarted).expect("send");
        tx.send(WatchUpdate::Applied(1)).expect("send");
        tx.send(WatchUpdate::Deleted(2)).expect("send");
        tx.send(WatchUpdate::InitDone).expect("send");

        assert_eq!(handle.try_recv(), Some(WatchUpdate::Restarted));
        assert_eq!(
            handle.drain(10),
            vec![
                WatchUpdate::Applied(1),
                WatchUpdate::Deleted(2),
                WatchUpdate::InitDone
            ]
        );
        assert!(handle.try_recv().is_none());
    }

    #[test]
    fn draining_stops_at_the_limit() {
        let (tx, handle, _stop) = handle();
        for value in 0..10 {
            tx.send(WatchUpdate::Applied(value)).expect("send");
        }

        let first = handle.drain(4);
        assert_eq!(first.len(), 4);
        assert_eq!(handle.drain_available().len(), 6);
    }

    #[test]
    fn a_closed_sender_reads_as_nothing_waiting_rather_than_an_error() {
        let (tx, handle, _stop) = handle();
        tx.send(WatchUpdate::Applied(1)).expect("send");
        drop(tx);

        assert_eq!(handle.try_recv(), Some(WatchUpdate::Applied(1)));
        assert!(handle.try_recv().is_none());
        assert!(handle.drain(10).is_empty());
    }

    #[test]
    fn a_detached_handle_reports_running_until_stopped() {
        let (_tx, mut handle, stop) = handle();
        assert!(handle.is_running());
        assert_eq!(handle.description(), "pods in namespace web");
        assert!(format!("{handle:?}").contains("pods in namespace web"));

        handle.stop();
        assert!(!handle.is_running());
        assert!(stop.load(AtomicOrdering::SeqCst));
        // Stopping twice is harmless.
        handle.stop();
    }

    #[test]
    fn dropping_a_handle_sets_the_stop_flag() {
        let (_tx, handle, stop) = handle();
        drop(handle);
        assert!(stop.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn a_failure_update_carries_its_message() {
        let (tx, handle, _stop) = handle();
        tx.send(WatchUpdate::Failed("410 Gone".to_string()))
            .expect("send");
        match handle.try_recv() {
            Some(WatchUpdate::Failed(message)) => assert_eq!(message, "410 Gone"),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn conversion_and_sorting_run_together() {
        let items = vec![3_u32, 1, 2];
        let sorted = convert_sorted(&items, |v| *v * 10, |a: &u32, b: &u32| a.cmp(b));
        assert_eq!(sorted, vec![10, 20, 30]);
    }

    #[test]
    fn converting_an_empty_list_yields_an_empty_list() {
        let items: Vec<u32> = Vec::new();
        let sorted = convert_sorted(&items, |v| *v, |a: &u32, b: &u32| a.cmp(b));
        assert!(sorted.is_empty());
    }
}
