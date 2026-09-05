//! Kubernetes operations for the App.
//!
//! Every method here is a thin call into `src/k8s` — the API layer — followed
//! by putting the result into [`K8sManager`] and saying what happened. The
//! decisions live on either side of this file, not in it: which cluster to
//! reach is in `src/k8s/endpoint.rs`, and what the rows say is in
//! `src/ui/k8s_manager/rows.rs`.

use tracing::{info, warn};

use crate::k8s::{
    ClusterEndpoint, K8sClient, K8sError, MAX_REPLICAS, load_contexts, load_contexts_from,
};
use crate::ui::k8s_manager::{ConnectedCluster, K8sManager, K8sView, ResourceKind};

use super::App;

/// How many replicas a single scale step changes by.
const SCALE_STEP: i64 = 1;

impl App {
    /// Opens the Kubernetes screens.
    ///
    /// A machine with no kubeconfig still gets a screen, saying so: an action
    /// that appears to do nothing is worse than one that explains itself.
    pub fn open_k8s_manager(&mut self) {
        let loaded = match self.k8s_fixture_kubeconfig() {
            // A fixture run reads the fixture's kubeconfig, or nothing. Reading
            // the user's real one would make a scripted run's output depend on
            // whose machine it ran on.
            Some(path) => load_contexts_from(&[path]),
            None if self.is_fixture_mode() => {
                self.k8s_manager = Some(K8sManager::unavailable(
                    "this fixture directory provides no kubeconfig".to_string(),
                ));
                self.set_status("Kubernetes");
                return;
            }
            None => load_contexts(),
        };

        let manager = match loaded {
            Ok(set) => {
                info!("kubernetes: {} context(s)", set.len());
                K8sManager::from_context_set(&set)
            }
            Err(e) => {
                warn!("kubernetes: {e}");
                K8sManager::unavailable(e.to_string())
            }
        };

        self.k8s_manager = Some(manager);
        self.set_status("Kubernetes");
    }

    /// The kubeconfig a fixture directory supplies, if it has one.
    fn k8s_fixture_kubeconfig(&self) -> Option<std::path::PathBuf> {
        let path = self.fixture_dir()?.join("kubeconfig");
        path.is_file().then_some(path)
    }

    /// Closes the Kubernetes screens, dropping any connection they held.
    pub fn close_k8s_manager(&mut self) {
        // Dropping the manager drops the client, which drops the SSH forward.
        self.k8s_manager = None;
        self.set_status("Kubernetes closed");
    }

    /// True while the Kubernetes screens are open.
    #[must_use]
    pub const fn is_k8s_manager_open(&self) -> bool {
        self.k8s_manager.is_some()
    }

    /// The Kubernetes manager, if it is open.
    #[must_use]
    pub const fn k8s_manager(&self) -> Option<&K8sManager> {
        self.k8s_manager.as_ref()
    }

    /// The Kubernetes manager, mutably.
    pub const fn k8s_manager_mut(&mut self) -> Option<&mut K8sManager> {
        self.k8s_manager.as_mut()
    }

    /// Connects to the selected context and lists its resources.
    pub fn k8s_connect_selected(&mut self) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        let Some(context) = manager.contexts().selected_item() else {
            manager.set_error("No context selected".to_string());
            return;
        };

        let name = context.name.clone();
        if !context.cluster_defined {
            manager.set_error(format!(
                "context '{name}' names a cluster the kubeconfig does not define; fix the clusters entry"
            ));
            return;
        }

        manager.set_status(format!("Connecting to {name}..."));
        let endpoint = ClusterEndpoint::direct(&name);

        match K8sClient::connect(&endpoint) {
            Ok(client) => {
                // The namespace list is a first request, so it is also the
                // first thing that can report the cluster is unreachable.
                let namespaces = client.list_namespaces().unwrap_or_default();
                manager.set_connected(ConnectedCluster {
                    client,
                    context: name.clone(),
                    namespaces,
                });
                self.k8s_refresh();
            }
            Err(e) => {
                warn!("kubernetes connect failed: {e}");
                manager.set_error(e.to_string());
            }
        }
    }

    /// Reloads the resource list currently showing.
    pub fn k8s_refresh(&mut self) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        let Some(cluster) = manager.connected() else {
            manager.set_error("Not connected to a cluster".to_string());
            return;
        };

        let namespace = manager.effective_namespace().map(str::to_string);
        let kind = manager.kind();
        let client = &cluster.client;
        let scope = namespace.as_deref();

        // Each arm keeps the borrow short: the result is collected first, then
        // handed to the manager, so nothing holds a reference across the call.
        let outcome = match kind {
            ResourceKind::Pods => client.list_pods(scope).map(Listing::Pods),
            ResourceKind::Deployments => client.list_deployments(scope).map(Listing::Deployments),
            ResourceKind::Services => client.list_services(scope).map(Listing::Services),
            ResourceKind::Nodes => client.list_nodes().map(Listing::Nodes),
            ResourceKind::Events => client.list_events(scope).map(Listing::Events),
        };

        match outcome {
            Ok(listing) => {
                let count = listing.len();
                listing.apply(manager);
                manager.clear_error();
                manager.set_status(format!("{count} {}", kind.label().to_lowercase()));
            }
            Err(e) => {
                warn!("kubernetes list failed: {e}");
                let gone = is_fatal(&e);
                manager.set_error(e.to_string());
                if gone {
                    // The cluster is not there any more. Keeping the client
                    // would leave the screen showing a stale listing under a
                    // cluster name that no longer answers.
                    manager.disconnect();
                    manager.set_error(format!("Disconnected: {e}"));
                }
            }
        }
    }

    /// Shows the next resource kind and lists it.
    pub fn k8s_next_kind(&mut self) {
        if let Some(manager) = self.k8s_manager.as_mut() {
            manager.next_kind();
        }
        self.k8s_refresh();
    }

    /// Shows the previous resource kind and lists it.
    pub fn k8s_previous_kind(&mut self) {
        if let Some(manager) = self.k8s_manager.as_mut() {
            manager.previous_kind();
        }
        self.k8s_refresh();
    }

    /// Scales the selected deployment by one replica.
    ///
    /// One at a time rather than to a typed number: a scale key that reads a
    /// number needs a prompt, and a prompt over a cluster action is the kind
    /// of thing that gets confirmed by accident.
    pub fn k8s_scale_selected(&mut self, up: bool) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        let Some(deployment) = manager.selected_deployment() else {
            manager.set_error("Select a deployment to scale".to_string());
            return;
        };

        let name = deployment.name.clone();
        let namespace = deployment.namespace.clone();
        let current = i64::from(deployment.desired);
        let target = if up {
            (current + SCALE_STEP).min(MAX_REPLICAS)
        } else {
            (current - SCALE_STEP).max(0)
        };

        if target == current {
            manager.set_status(format!("{name} is already at {current}"));
            return;
        }

        let Some(cluster) = manager.connected() else {
            return;
        };

        match cluster.client.scale_deployment(&namespace, &name, target) {
            Ok(_) => {
                manager.clear_error();
                manager.set_status(format!("Scaled {name} to {target}"));
                self.k8s_refresh();
            }
            Err(e) => manager.set_error(e.to_string()),
        }
    }

    /// Restarts the selected deployment's rollout.
    pub fn k8s_restart_selected(&mut self) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        let Some(deployment) = manager.selected_deployment() else {
            manager.set_error("Select a deployment to restart".to_string());
            return;
        };

        let name = deployment.name.clone();
        let namespace = deployment.namespace.clone();
        let Some(cluster) = manager.connected() else {
            return;
        };

        match cluster.client.restart_rollout(&namespace, &name) {
            Ok(_) => {
                manager.clear_error();
                manager.set_status(format!("Restarted {name}"));
                self.k8s_refresh();
            }
            Err(e) => manager.set_error(e.to_string()),
        }
    }

    /// Deletes the selected pod.
    ///
    /// Deleting a pod is how a rollout is nudged, not a destructive act: the
    /// controller replaces it. Deleting a deployment, which is destructive, is
    /// deliberately not offered here.
    pub fn k8s_delete_selected_pod(&mut self) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        let Some(pod) = manager.selected_pod() else {
            manager.set_error("Select a pod to delete".to_string());
            return;
        };

        let name = pod.name.clone();
        let namespace = pod.namespace.clone();
        let Some(cluster) = manager.connected() else {
            return;
        };

        match cluster.client.delete_pod(&namespace, &name, None) {
            Ok(()) => {
                manager.clear_error();
                manager.set_status(format!("Deleted {name}"));
                self.k8s_refresh();
            }
            Err(e) => manager.set_error(e.to_string()),
        }
    }

    /// Returns to the context list, keeping the connection.
    pub fn k8s_show_contexts(&mut self) {
        if let Some(manager) = self.k8s_manager.as_mut() {
            manager.show_contexts();
        }
    }

    /// Returns to the resource list.
    pub fn k8s_show_resources(&mut self) {
        let Some(manager) = self.k8s_manager.as_mut() else {
            return;
        };
        if !manager.show_resources() {
            manager.set_error("Connect to a context first".to_string());
        }
    }

    /// Reports whether the manager is on the context screen.
    #[must_use]
    pub fn k8s_is_choosing_context(&self) -> bool {
        self.k8s_manager
            .as_ref()
            .is_some_and(|manager| manager.view() == K8sView::Contexts)
    }
}

/// One listing, so the borrow of the client ends before the manager is written.
enum Listing {
    /// Pods.
    Pods(Vec<crate::k8s::PodView>),
    /// Deployments.
    Deployments(Vec<crate::k8s::DeploymentView>),
    /// Services.
    Services(Vec<crate::k8s::ServiceView>),
    /// Nodes.
    Nodes(Vec<crate::k8s::NodeView>),
    /// Events.
    Events(Vec<crate::k8s::EventView>),
}

impl Listing {
    /// How many were listed.
    const fn len(&self) -> usize {
        match self {
            Self::Pods(items) => items.len(),
            Self::Deployments(items) => items.len(),
            Self::Services(items) => items.len(),
            Self::Nodes(items) => items.len(),
            Self::Events(items) => items.len(),
        }
    }

    /// Puts the listing into the manager.
    fn apply(self, manager: &mut K8sManager) {
        match self {
            Self::Pods(items) => manager.set_pods(items),
            Self::Deployments(items) => manager.set_deployments(items),
            Self::Services(items) => manager.set_services(items),
            Self::Nodes(items) => manager.set_nodes(items),
            Self::Events(items) => manager.set_events(items),
        }
    }
}

/// True when the error means the cluster could not be reached at all.
///
/// Used to decide whether to keep the connection: a listing that failed
/// because of a permission is worth staying connected for, one that failed
/// because the API server is gone is not.
#[must_use]
pub const fn is_fatal(error: &K8sError) -> bool {
    matches!(
        error,
        K8sError::Unreachable { .. } | K8sError::SshTunnel { .. } | K8sError::NoKubeconfig
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_reports_its_own_length() {
        assert_eq!(Listing::Pods(Vec::new()).len(), 0);
        assert_eq!(Listing::Nodes(Vec::new()).len(), 0);
    }

    #[test]
    fn applying_a_listing_fills_the_matching_panel() {
        let mut manager = K8sManager::new(Vec::new());
        Listing::Pods(Vec::new()).apply(&mut manager);
        assert!(manager.pods().is_open());
        assert!(!manager.nodes().is_open(), "only the matching one");
    }

    #[test]
    fn an_unreachable_cluster_is_a_fatal_error_and_a_missing_pod_is_not() {
        assert!(is_fatal(&K8sError::NoKubeconfig));
        assert!(is_fatal(&K8sError::Unreachable {
            context: "dev".to_string(),
            reason: "connection refused".to_string(),
        }));
        assert!(!is_fatal(&K8sError::UnknownContext("dev".to_string())));
    }
}
