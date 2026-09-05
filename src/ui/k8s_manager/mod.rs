//! State for the Kubernetes screens.
//!
//! `src/k8s` is the API layer: it owns every dependency on `kube` and hands
//! back plain owned structs. This is the state those structs are held in while
//! the user navigates them — which context is selected, which kind of resource
//! is showing, what is typed into the filter — and nothing here talks to a
//! cluster.
//!
//! That split is what makes the screens testable: every function below is a
//! function of its inputs, so the behaviour can be checked without an API
//! server, and the widget can be rendered from a state built by hand.

mod connection;
mod kinds;
mod rows;
mod widget;

pub use connection::ConnectedCluster;
pub use kinds::{K8sView, ResourceKind};
pub use rows::{ResourceRow, filter_rows, row_matches};
pub use widget::K8sManagerWidget;

use crate::app::panel::ListPanel;
use crate::k8s::{
    ContextSet, DeploymentView, EventView, KubeContext, NodeView, PodView, ServiceView,
};

/// The Kubernetes manager's state.
#[derive(Debug, Default)]
pub struct K8sManager {
    /// Which screen is showing.
    view: K8sView,
    /// Contexts read from the kubeconfig.
    contexts: ListPanel<KubeContext>,
    /// The connected cluster, if any.
    connected: Option<ConnectedCluster>,
    /// Which resource kind the resource view lists.
    kind: ResourceKind,
    /// Namespace filter; `None` means every namespace.
    namespace: Option<String>,
    /// Pods, as last listed.
    pods: ListPanel<PodView>,
    /// Deployments, as last listed.
    deployments: ListPanel<DeploymentView>,
    /// Services, as last listed.
    services: ListPanel<ServiceView>,
    /// Nodes, as last listed.
    nodes: ListPanel<NodeView>,
    /// Events, as last listed.
    events: ListPanel<EventView>,
    /// What is typed into the filter.
    filter: String,
    /// True while the filter is being typed into.
    filtering: bool,
    /// The last error, shown until something replaces it.
    error: Option<String>,
    /// A one-line message about what just happened.
    status: String,
}

impl K8sManager {
    /// A manager showing `contexts`.
    ///
    /// Takes the contexts rather than a [`ContextSet`] so the state does not
    /// depend on the kubeconfig type: this layer only ever displays them, and
    /// a test can build a list by hand.
    #[must_use]
    pub fn new(contexts: Vec<KubeContext>) -> Self {
        // Start on the kubeconfig's current context: it is what `kubectl`
        // would use, and landing anywhere else invites connecting to the
        // wrong cluster.
        let current = contexts.iter().position(|c| c.is_current);

        let mut panel = ListPanel::closed();
        panel.open(contexts);

        let mut manager = Self {
            contexts: panel,
            ..Self::default()
        };
        if let Some(index) = current {
            manager.contexts.select(index);
        }
        manager
    }

    /// A manager showing the contexts a kubeconfig defines.
    #[must_use]
    pub fn from_context_set(set: &ContextSet) -> Self {
        Self::new(set.contexts().to_vec())
    }

    /// A manager with no kubeconfig, showing why.
    #[must_use]
    pub fn unavailable(reason: String) -> Self {
        let mut manager = Self::default();
        manager.contexts.open(Vec::new());
        manager.error = Some(reason);
        manager
    }

    /// Which screen is showing.
    #[must_use]
    pub const fn view(&self) -> K8sView {
        self.view
    }

    /// Shows the context list.
    pub fn show_contexts(&mut self) {
        self.view = K8sView::Contexts;
        self.stop_filtering();
    }

    /// Shows the resource list.
    ///
    /// Refuses while nothing is connected: an empty resource screen with no
    /// cluster behind it looks like a cluster with nothing in it.
    pub fn show_resources(&mut self) -> bool {
        if self.connected.is_none() {
            return false;
        }
        self.view = K8sView::Resources;
        true
    }

    /// The contexts panel.
    #[must_use]
    pub const fn contexts(&self) -> &ListPanel<KubeContext> {
        &self.contexts
    }

    /// The contexts panel, mutably.
    pub const fn contexts_mut(&mut self) -> &mut ListPanel<KubeContext> {
        &mut self.contexts
    }

    /// The connected cluster, if any.
    #[must_use]
    pub const fn connected(&self) -> Option<&ConnectedCluster> {
        self.connected.as_ref()
    }

    /// Records a successful connection and moves to the resource view.
    pub fn set_connected(&mut self, cluster: ConnectedCluster) {
        self.status = format!("Connected to {}", cluster.context);
        self.error = None;
        self.namespace = self
            .contexts
            .selected_item()
            .map(|context| context.namespace.clone());
        self.connected = Some(cluster);
        self.view = K8sView::Resources;
        self.clear_resources();
    }

    /// Drops the connection, closing any SSH forward it held.
    pub fn disconnect(&mut self) {
        self.connected = None;
        self.clear_resources();
        self.view = K8sView::Contexts;
        self.status = "Disconnected".to_string();
    }

    /// Which resource kind is showing.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Moves to the next resource kind.
    pub fn next_kind(&mut self) {
        self.kind = self.kind.next();
        self.stop_filtering();
    }

    /// Moves to the previous resource kind.
    pub fn previous_kind(&mut self) {
        self.kind = self.kind.previous();
        self.stop_filtering();
    }

    /// Shows a specific resource kind.
    pub fn set_kind(&mut self, kind: ResourceKind) {
        self.kind = kind;
        self.stop_filtering();
    }

    /// The namespace filter, or `None` for every namespace.
    #[must_use]
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// Sets the namespace filter.
    pub fn set_namespace(&mut self, namespace: Option<String>) {
        self.namespace = namespace;
    }

    /// The namespace the API layer should be asked for.
    ///
    /// A cluster-scoped kind ignores the filter rather than asking for nodes
    /// "in" a namespace, which would return nothing.
    #[must_use]
    pub fn effective_namespace(&self) -> Option<&str> {
        if self.kind.is_cluster_scoped() {
            None
        } else {
            self.namespace()
        }
    }

    /// What is typed into the filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// True while the filter is being typed into.
    #[must_use]
    pub const fn is_filtering(&self) -> bool {
        self.filtering
    }

    /// Starts typing a filter.
    pub fn start_filtering(&mut self) {
        self.filtering = true;
    }

    /// Stops typing, keeping what was typed.
    pub const fn stop_filtering(&mut self) {
        self.filtering = false;
    }

    /// Adds a character to the filter.
    pub fn push_filter(&mut self, c: char) {
        self.filter.push(c);
    }

    /// Removes the last character of the filter.
    pub fn pop_filter(&mut self) {
        self.filter.pop();
    }

    /// Clears the filter and stops typing.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filtering = false;
    }

    /// The last error, if there is one.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Records an error, which replaces any previous one.
    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Forgets the last error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// The current status line.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Sets the status line.
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    /// Replaces the pod list.
    pub fn set_pods(&mut self, pods: Vec<PodView>) {
        self.pods.replace_or_open(pods);
    }

    /// Replaces the deployment list.
    pub fn set_deployments(&mut self, deployments: Vec<DeploymentView>) {
        self.deployments.replace_or_open(deployments);
    }

    /// Replaces the service list.
    pub fn set_services(&mut self, services: Vec<ServiceView>) {
        self.services.replace_or_open(services);
    }

    /// Replaces the node list.
    pub fn set_nodes(&mut self, nodes: Vec<NodeView>) {
        self.nodes.replace_or_open(nodes);
    }

    /// Replaces the event list.
    pub fn set_events(&mut self, events: Vec<EventView>) {
        self.events.replace_or_open(events);
    }

    /// The pods, as last listed.
    #[must_use]
    pub const fn pods(&self) -> &ListPanel<PodView> {
        &self.pods
    }

    /// The deployments, as last listed.
    #[must_use]
    pub const fn deployments(&self) -> &ListPanel<DeploymentView> {
        &self.deployments
    }

    /// The nodes, as last listed.
    #[must_use]
    pub const fn nodes(&self) -> &ListPanel<NodeView> {
        &self.nodes
    }

    /// Forgets every listed resource.
    ///
    /// Called on connect and disconnect: showing one cluster's pods under
    /// another cluster's name is worse than showing none.
    pub fn clear_resources(&mut self) {
        self.pods.close();
        self.deployments.close();
        self.services.close();
        self.nodes.close();
        self.events.close();
        self.filter.clear();
        self.filtering = false;
    }

    /// The rows the resource table should draw, after filtering.
    #[must_use]
    pub fn rows(&self) -> Vec<ResourceRow> {
        let all = match self.kind {
            ResourceKind::Pods => rows::pod_rows(self.pods.items()),
            ResourceKind::Deployments => rows::deployment_rows(self.deployments.items()),
            ResourceKind::Services => rows::service_rows(self.services.items()),
            ResourceKind::Nodes => rows::node_rows(self.nodes.items()),
            ResourceKind::Events => rows::event_rows(self.events.items()),
        };
        filter_rows(all, &self.filter)
    }

    /// The selected row of the current kind.
    #[must_use]
    pub fn selected_row_index(&self) -> usize {
        match self.kind {
            ResourceKind::Pods => self.pods.selected(),
            ResourceKind::Deployments => self.deployments.selected(),
            ResourceKind::Services => self.services.selected(),
            ResourceKind::Nodes => self.nodes.selected(),
            ResourceKind::Events => self.events.selected(),
        }
    }

    /// Moves the selection down within the filtered rows.
    pub fn select_next(&mut self) {
        let rows = self.rows().len();
        match self.kind {
            ResourceKind::Pods => self.pods.select_next_in(rows),
            ResourceKind::Deployments => self.deployments.select_next_in(rows),
            ResourceKind::Services => self.services.select_next_in(rows),
            ResourceKind::Nodes => self.nodes.select_next_in(rows),
            ResourceKind::Events => self.events.select_next_in(rows),
        }
    }

    /// Selects the first row.
    pub fn select_first_row(&mut self) {
        match self.kind {
            ResourceKind::Pods => self.pods.select_first(),
            ResourceKind::Deployments => self.deployments.select_first(),
            ResourceKind::Services => self.services.select_first(),
            ResourceKind::Nodes => self.nodes.select_first(),
            ResourceKind::Events => self.events.select_first(),
        }
    }

    /// Selects the last row of the filtered list.
    pub fn select_last_row(&mut self) {
        let rows = self.rows().len();
        match self.kind {
            ResourceKind::Pods => self.pods.select_last_in(rows),
            ResourceKind::Deployments => self.deployments.select_last_in(rows),
            ResourceKind::Services => self.services.select_last_in(rows),
            ResourceKind::Nodes => self.nodes.select_last_in(rows),
            ResourceKind::Events => self.events.select_last_in(rows),
        }
    }

    /// Moves the selection up.
    pub fn select_previous(&mut self) {
        match self.kind {
            ResourceKind::Pods => self.pods.select_previous(),
            ResourceKind::Deployments => self.deployments.select_previous(),
            ResourceKind::Services => self.services.select_previous(),
            ResourceKind::Nodes => self.nodes.select_previous(),
            ResourceKind::Events => self.events.select_previous(),
        }
    }

    /// The selected pod, when pods are showing and one is selected.
    #[must_use]
    pub fn selected_pod(&self) -> Option<&PodView> {
        if self.kind != ResourceKind::Pods {
            return None;
        }
        let name = self.rows().get(self.pods.selected())?.key.clone();
        self.pods.items().iter().find(|pod| pod.name == name)
    }

    /// The selected deployment, when deployments are showing.
    #[must_use]
    pub fn selected_deployment(&self) -> Option<&DeploymentView> {
        if self.kind != ResourceKind::Deployments {
            return None;
        }
        let name = self.rows().get(self.deployments.selected())?.key.clone();
        self.deployments
            .items()
            .iter()
            .find(|deployment| deployment.name == name)
    }
}

impl crate::app::panel::Panel for K8sManager {
    fn title(&self) -> String {
        match self.view {
            K8sView::Contexts => "Kubernetes - Contexts".to_string(),
            K8sView::Resources => self.connected.as_ref().map_or_else(
                || "Kubernetes".to_string(),
                |cluster| format!("Kubernetes - {}", cluster.context),
            ),
        }
    }

    fn is_open(&self) -> bool {
        // The manager exists only while it is open: `App` holds it in an
        // `Option` and drops it on close, which is also what closes the SSH
        // forward the client may hold.
        true
    }

    fn close(&mut self) {
        self.disconnect();
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> crate::app::panel::PanelOutcome {
        use crate::app::input_k8s::{K8sAction, action_for};
        use crate::app::panel::PanelOutcome;

        match action_for(key, self.view, self.filtering) {
            K8sAction::Ignored => PanelOutcome::Ignored,
            K8sAction::Close => PanelOutcome::Close,
            // Everything else needs the cluster, which lives on `App`, so the
            // panel reports that it wants the key and `App` acts on it. The
            // trait exists to make focus and titles uniform, not to move
            // cluster access into a widget.
            _ => PanelOutcome::Handled,
        }
    }

    fn render(
        &self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        focused: bool,
    ) {
        use ratatui::widgets::Widget as _;
        K8sManagerWidget::new(self)
            .focused(focused)
            .render(area, buf);
    }

    fn key_hints(&self) -> &'static str {
        match self.view {
            K8sView::Contexts => "[Up/Down] Select  [Enter] Connect  [Esc] Close",
            K8sView::Resources => {
                "[Tab] Kind  [/] Filter  [r] Refresh  [Backspace] Contexts  [Esc] Close"
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::app::panel::{Panel, PanelOutcome};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    /// A plain key press.
    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn the_panel_titles_itself_from_its_screen() {
        let mut manager = K8sManager::new(contexts());
        assert_eq!(Panel::title(&manager), "Kubernetes - Contexts");

        manager.view = K8sView::Resources;
        assert_eq!(
            Panel::title(&manager),
            "Kubernetes",
            "with nothing connected there is no cluster to name"
        );
    }

    #[test]
    fn the_panel_reports_what_it_did_with_a_key() {
        let mut manager = K8sManager::new(contexts());
        assert_eq!(manager.handle_key(press(KeyCode::Esc)), PanelOutcome::Close);
        assert_eq!(
            manager.handle_key(press(KeyCode::Down)),
            PanelOutcome::Handled
        );
        assert_eq!(
            manager.handle_key(press(KeyCode::F(9))),
            PanelOutcome::Ignored
        );
    }

    #[test]
    fn the_panel_offers_hints_for_the_screen_it_is_showing() {
        let mut manager = K8sManager::new(contexts());
        assert!(Panel::key_hints(&manager).contains("Connect"));

        manager.view = K8sView::Resources;
        assert!(Panel::key_hints(&manager).contains("Filter"));
    }

    /// Three contexts, the second being the kubeconfig's current one.
    fn contexts() -> Vec<KubeContext> {
        vec![
            KubeContext {
                name: "dev".to_string(),
                cluster: "dev-cluster".to_string(),
                user: None,
                namespace: "default".to_string(),
                server: Some("https://dev:6443".to_string()),
                is_current: false,
                cluster_defined: true,
            },
            KubeContext {
                name: "staging".to_string(),
                cluster: "staging-cluster".to_string(),
                user: None,
                namespace: "apps".to_string(),
                server: Some("https://staging:6443".to_string()),
                is_current: true,
                cluster_defined: true,
            },
            KubeContext {
                name: "broken".to_string(),
                cluster: "missing".to_string(),
                user: None,
                namespace: "default".to_string(),
                server: None,
                is_current: false,
                cluster_defined: false,
            },
        ]
    }

    #[test]
    fn a_new_manager_starts_on_the_current_context() {
        // Landing anywhere else invites connecting to the wrong cluster.
        let manager = K8sManager::new(contexts());
        assert_eq!(manager.view(), K8sView::Contexts);
        assert_eq!(
            manager.contexts().selected_item().map(|c| c.name.as_str()),
            Some("staging")
        );
    }

    #[test]
    fn a_context_set_with_no_current_starts_at_the_top() {
        let mut set = contexts();
        set[1].is_current = false;

        let manager = K8sManager::new(set);
        assert_eq!(manager.contexts().selected(), 0);
    }

    #[test]
    fn an_unavailable_manager_says_why() {
        let manager = K8sManager::unavailable("no kubeconfig found".to_string());
        assert_eq!(manager.error(), Some("no kubeconfig found"));
        assert!(manager.contexts().is_open(), "the screen still opens");
        assert!(!manager.contexts().has_items());
    }

    #[test]
    fn the_resource_view_is_refused_while_nothing_is_connected() {
        let mut manager = K8sManager::new(contexts());
        assert!(!manager.show_resources());
        assert_eq!(
            manager.view(),
            K8sView::Contexts,
            "an empty resource screen would look like an empty cluster"
        );
    }

    #[test]
    fn nodes_ignore_the_namespace_filter() {
        let mut manager = K8sManager::new(contexts());
        manager.set_namespace(Some("apps".to_string()));

        manager.set_kind(ResourceKind::Pods);
        assert_eq!(manager.effective_namespace(), Some("apps"));

        manager.set_kind(ResourceKind::Nodes);
        assert_eq!(
            manager.effective_namespace(),
            None,
            "asking for nodes in a namespace returns nothing"
        );
    }

    #[test]
    fn changing_kind_stops_filtering_but_keeps_the_text() {
        let mut manager = K8sManager::new(contexts());
        manager.start_filtering();
        manager.push_filter('a');
        manager.push_filter('p');

        manager.next_kind();

        assert!(!manager.is_filtering(), "the filter box closes");
        assert_eq!(manager.filter(), "ap", "what was typed is kept");
    }

    #[test]
    fn the_filter_can_be_edited_and_cleared() {
        let mut manager = K8sManager::new(contexts());
        manager.start_filtering();
        manager.push_filter('x');
        manager.push_filter('y');
        manager.pop_filter();
        assert_eq!(manager.filter(), "x");

        manager.clear_filter();
        assert_eq!(manager.filter(), "");
        assert!(!manager.is_filtering());
    }

    #[test]
    fn popping_an_empty_filter_does_nothing() {
        let mut manager = K8sManager::new(contexts());
        manager.pop_filter();
        assert_eq!(manager.filter(), "");
    }

    #[test]
    fn an_error_replaces_the_previous_one_and_can_be_cleared() {
        let mut manager = K8sManager::new(contexts());
        manager.set_error("first".to_string());
        manager.set_error("second".to_string());
        assert_eq!(manager.error(), Some("second"));

        manager.clear_error();
        assert_eq!(manager.error(), None);
    }

    #[test]
    fn disconnecting_forgets_the_resources_and_returns_to_the_contexts() {
        let mut manager = K8sManager::new(contexts());
        manager.set_pods(vec![]);
        manager.disconnect();

        assert_eq!(manager.view(), K8sView::Contexts);
        assert!(!manager.pods().is_open(), "another cluster's pods are gone");
        assert!(manager.connected().is_none());
    }

    #[test]
    fn selecting_a_pod_while_another_kind_shows_yields_nothing() {
        let mut manager = K8sManager::new(contexts());
        manager.set_kind(ResourceKind::Nodes);
        assert!(manager.selected_pod().is_none());
        assert!(manager.selected_deployment().is_none());
    }
}
