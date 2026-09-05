//! The node list view.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Node;

use super::{age_at, age_display, labels_of, name_of, to_utc};

/// The label prefix the cluster uses to mark a node's role.
const ROLE_PREFIX: &str = "node-role.kubernetes.io/";

/// The legacy single-role label, still set by some installers.
const LEGACY_ROLE_LABEL: &str = "kubernetes.io/role";

/// Whether a node's `Ready` condition is true, false, or missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeReady {
    /// The kubelet reports the node ready.
    Ready,
    /// The kubelet reports the node not ready.
    NotReady,
    /// No `Ready` condition was reported, or its value was neither `True` nor
    /// `False`. The control plane uses this while a node is unreachable.
    Unknown,
}

impl NodeReady {
    /// Parses the `status` field of a `Ready` condition.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "True" => Self::Ready,
            "False" => Self::NotReady,
            _ => Self::Unknown,
        }
    }

    /// Returns the text for the status column.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::NotReady => "NotReady",
            Self::Unknown => "Unknown",
        }
    }
}

/// A node as a list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    /// Node name.
    pub name: String,
    /// The `Ready` condition.
    pub ready: NodeReady,
    /// Roles taken from the node's role labels, sorted. Empty for a node with
    /// no role label, which `kubectl` shows as `<none>`.
    pub roles: Vec<String>,
    /// False when the node has been cordoned.
    pub schedulable: bool,
    /// Kubelet version, absent when the node has not reported system info.
    pub kubelet_version: Option<String>,
    /// The node's internal address.
    pub internal_ip: Option<String>,
    /// The operating system image the node runs.
    pub os_image: Option<String>,
    /// The node's CPU architecture.
    pub architecture: Option<String>,
    /// Node labels.
    pub labels: BTreeMap<String, String>,
    /// Creation time, absent only on malformed objects.
    pub created: Option<DateTime<Utc>>,
}

impl NodeView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(node: &Node) -> Self {
        let meta = &node.metadata;
        let status = node.status.as_ref();
        let labels = labels_of(meta);

        let ready = status
            .and_then(|s| s.conditions.as_ref())
            .and_then(|conditions| conditions.iter().find(|c| c.type_ == "Ready"))
            .map_or(NodeReady::Unknown, |c| NodeReady::parse(&c.status));

        let internal_ip = status
            .and_then(|s| s.addresses.as_ref())
            .and_then(|addresses| addresses.iter().find(|a| a.type_ == "InternalIP"))
            .map(|a| a.address.clone())
            .filter(|a| !a.is_empty());

        let info = status.and_then(|s| s.node_info.as_ref());

        Self {
            name: name_of(meta),
            ready,
            roles: roles_from_labels(&labels),
            schedulable: !node
                .spec
                .as_ref()
                .and_then(|s| s.unschedulable)
                .unwrap_or(false),
            kubelet_version: info
                .map(|i| i.kubelet_version.clone())
                .filter(|v| !v.is_empty()),
            internal_ip,
            os_image: info.map(|i| i.os_image.clone()).filter(|v| !v.is_empty()),
            architecture: info
                .map(|i| i.architecture.clone())
                .filter(|v| !v.is_empty()),
            labels,
            created: to_utc(meta.creation_timestamp.as_ref()),
        }
    }

    /// Returns the node's age at `now`.
    #[must_use]
    pub fn age(&self, now: DateTime<Utc>) -> Option<Duration> {
        age_at(self.created, now)
    }

    /// Returns the short age string for the list, or `-`.
    #[must_use]
    pub fn age_display(&self, now: DateTime<Utc>) -> String {
        age_display(self.created, now)
    }

    /// Returns the status column: the ready state, plus a cordon marker.
    #[must_use]
    pub fn status_display(&self) -> String {
        if self.schedulable {
            self.ready.label().to_string()
        } else {
            format!("{},SchedulingDisabled", self.ready.label())
        }
    }

    /// Returns the roles column, or `<none>`.
    #[must_use]
    pub fn roles_display(&self) -> String {
        if self.roles.is_empty() {
            "<none>".to_string()
        } else {
            self.roles.join(",")
        }
    }
}

/// Extracts node roles from the node's labels.
///
/// Both the current `node-role.kubernetes.io/<role>` labels and the legacy
/// `kubernetes.io/role` label are read, because clusters built by different
/// installers set different ones. The result is sorted and deduplicated so the
/// column is stable.
fn roles_from_labels(labels: &BTreeMap<String, String>) -> Vec<String> {
    let mut roles: Vec<String> = labels
        .keys()
        .filter_map(|key| key.strip_prefix(ROLE_PREFIX))
        .filter(|role| !role.is_empty())
        .map(ToString::to_string)
        .collect();

    if let Some(legacy) = labels.get(LEGACY_ROLE_LABEL)
        && !legacy.is_empty()
    {
        roles.push(legacy.clone());
    }

    roles.sort();
    roles.dedup();
    roles
}

/// Orders nodes for a list: by name.
///
/// Nodes are cluster-scoped, so the name alone is unique and the order is
/// total.
#[must_use]
pub fn compare_nodes(a: &NodeView, b: &NodeView) -> Ordering {
    a.name.cmp(&b.name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::core::v1::{
        NodeAddress, NodeCondition, NodeSpec, NodeStatus, NodeSystemInfo,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::super::test_support::{time_at, utc_at};
    use super::*;

    fn condition(type_: &str, status: &str) -> NodeCondition {
        NodeCondition {
            type_: type_.to_string(),
            status: status.to_string(),
            ..NodeCondition::default()
        }
    }

    fn full_node() -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some("worker-1".to_string()),
                labels: Some(BTreeMap::from([
                    (format!("{ROLE_PREFIX}worker"), String::new()),
                    ("kubernetes.io/arch".to_string(), "arm64".to_string()),
                ])),
                creation_timestamp: Some(time_at(1_000)),
                ..ObjectMeta::default()
            },
            spec: Some(NodeSpec {
                unschedulable: Some(false),
                ..NodeSpec::default()
            }),
            status: Some(NodeStatus {
                conditions: Some(vec![
                    condition("MemoryPressure", "False"),
                    condition("Ready", "True"),
                ]),
                addresses: Some(vec![
                    NodeAddress {
                        type_: "Hostname".to_string(),
                        address: "worker-1".to_string(),
                    },
                    NodeAddress {
                        type_: "InternalIP".to_string(),
                        address: "10.0.0.217".to_string(),
                    },
                ]),
                node_info: Some(NodeSystemInfo {
                    architecture: "arm64".to_string(),
                    kubelet_version: "v1.33.2".to_string(),
                    os_image: "Debian GNU/Linux 13".to_string(),
                    ..NodeSystemInfo::default()
                }),
                ..NodeStatus::default()
            }),
        }
    }

    #[test]
    fn a_fully_populated_node_converts_every_field() {
        let view = NodeView::from_api(&full_node());

        assert_eq!(view.name, "worker-1");
        assert_eq!(view.ready, NodeReady::Ready);
        assert_eq!(view.roles, vec!["worker"]);
        assert!(view.schedulable);
        assert_eq!(view.kubelet_version.as_deref(), Some("v1.33.2"));
        assert_eq!(view.internal_ip.as_deref(), Some("10.0.0.217"));
        assert_eq!(view.os_image.as_deref(), Some("Debian GNU/Linux 13"));
        assert_eq!(view.architecture.as_deref(), Some("arm64"));
        assert_eq!(view.created, Some(utc_at(1_000)));
        assert_eq!(view.status_display(), "Ready");
        assert_eq!(view.roles_display(), "worker");
    }

    #[test]
    fn a_minimal_node_converts_to_documented_defaults() {
        let view = NodeView::from_api(&Node::default());

        assert_eq!(view.name, "");
        assert_eq!(view.ready, NodeReady::Unknown);
        assert!(view.roles.is_empty());
        assert!(view.schedulable);
        assert!(view.kubelet_version.is_none());
        assert!(view.internal_ip.is_none());
        assert!(view.os_image.is_none());
        assert!(view.architecture.is_none());
        assert!(view.labels.is_empty());
        assert!(view.created.is_none());
        assert_eq!(view.status_display(), "Unknown");
        assert_eq!(view.roles_display(), "<none>");
        assert_eq!(view.age_display(utc_at(10)), "-");
    }

    #[test]
    fn an_unexpected_ready_status_becomes_unknown_rather_than_panicking() {
        let mut node = full_node();
        if let Some(status) = node.status.as_mut()
            && let Some(conditions) = status.conditions.as_mut()
        {
            conditions[1].status = "Maybe".to_string();
        }
        let view = NodeView::from_api(&node);
        assert_eq!(view.ready, NodeReady::Unknown);
        assert_eq!(NodeReady::parse("False"), NodeReady::NotReady);
        assert_eq!(NodeReady::NotReady.label(), "NotReady");
    }

    #[test]
    fn a_node_with_no_ready_condition_at_all_is_unknown() {
        let mut node = full_node();
        if let Some(status) = node.status.as_mut() {
            status.conditions = Some(vec![condition("MemoryPressure", "False")]);
        }
        assert_eq!(NodeView::from_api(&node).ready, NodeReady::Unknown);
    }

    #[test]
    fn a_cordoned_node_is_marked_unschedulable() {
        let mut node = full_node();
        if let Some(spec) = node.spec.as_mut() {
            spec.unschedulable = Some(true);
        }
        let view = NodeView::from_api(&node);
        assert!(!view.schedulable);
        assert_eq!(view.status_display(), "Ready,SchedulingDisabled");
    }

    #[test]
    fn several_role_labels_are_collected_sorted_and_deduplicated() {
        let labels = BTreeMap::from([
            (format!("{ROLE_PREFIX}worker"), String::new()),
            (format!("{ROLE_PREFIX}control-plane"), String::new()),
            (LEGACY_ROLE_LABEL.to_string(), "worker".to_string()),
        ]);
        assert_eq!(roles_from_labels(&labels), vec!["control-plane", "worker"]);
    }

    #[test]
    fn an_empty_role_label_suffix_is_ignored() {
        let labels = BTreeMap::from([(ROLE_PREFIX.to_string(), String::new())]);
        assert!(roles_from_labels(&labels).is_empty());
    }

    #[test]
    fn node_age_uses_the_creation_timestamp() {
        let view = NodeView::from_api(&full_node());
        assert_eq!(view.age(utc_at(87_400)), Some(Duration::from_secs(86_400)));
        assert_eq!(view.age_display(utc_at(87_400)), "1d");
    }

    #[test]
    fn nodes_sort_by_name() {
        let mut views = [
            node_named("worker-2"),
            node_named("control-1"),
            node_named("worker-1"),
        ];
        views.sort_by(compare_nodes);
        let names: Vec<&str> = views.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["control-1", "worker-1", "worker-2"]);

        let a = node_named("a");
        let b = node_named("b");
        assert_eq!(compare_nodes(&a, &a), Ordering::Equal);
        assert_eq!(compare_nodes(&a, &b), Ordering::Less);
        assert_eq!(compare_nodes(&b, &a), Ordering::Greater);
    }

    fn node_named(name: &str) -> NodeView {
        let node = Node {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..ObjectMeta::default()
            },
            ..Node::default()
        };
        NodeView::from_api(&node)
    }
}
