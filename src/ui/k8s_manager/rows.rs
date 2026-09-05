//! Turning resource views into table rows, and filtering them.
//!
//! Kept apart from both the state and the widget because this is where the
//! decisions are: what a pod's status column says, how a port list is
//! abbreviated, what a filter matches. Those are worth testing, and none of
//! them needs a cluster or a terminal.

use crate::k8s::resources::age_at;
use crate::k8s::{
    DeploymentView, EventView, NodeReady, NodeView, PodView, ServiceView, format_age,
};

/// Longest message shown in an event row before it is cut.
///
/// Event messages run to several sentences; a table that widens to fit one
/// makes every other column unreadable.
const MAX_EVENT_MESSAGE: usize = 60;

/// Most ports listed in a service row before the rest are counted.
const MAX_PORTS_SHOWN: usize = 3;

/// One row of the resource table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRow {
    /// The resource's name, used to find it again after filtering.
    pub key: String,
    /// The cells, in the order the headings give.
    pub cells: Vec<String>,
    /// True when the row describes something unhealthy.
    ///
    /// Carried on the row rather than worked out while drawing, so the rule
    /// is in one place and can be tested.
    pub unhealthy: bool,
}

impl ResourceRow {
    /// A row that is fine.
    fn healthy(key: impl Into<String>, cells: Vec<String>) -> Self {
        Self {
            key: key.into(),
            cells,
            unhealthy: false,
        }
    }

    /// A row that is not.
    fn unhealthy(key: impl Into<String>, cells: Vec<String>) -> Self {
        Self {
            key: key.into(),
            cells,
            unhealthy: true,
        }
    }
}

/// Rows for a pod list.
#[must_use]
pub fn pod_rows(pods: &[PodView]) -> Vec<ResourceRow> {
    pods.iter()
        .map(|pod| {
            let cells = vec![
                pod.name.clone(),
                format!("{}/{}", pod.ready_containers, pod.total_containers),
                pod.status_text.clone(),
                pod.restarts.to_string(),
                pod.node.clone().unwrap_or_else(|| "<none>".to_string()),
                age_cell(pod.created),
            ];

            // Not every non-Running pod is a problem — a completed job pod is
            // Succeeded — so this asks whether the containers are up rather
            // than comparing against a list of phase names.
            let healthy = pod.ready_containers == pod.total_containers
                && pod.total_containers > 0
                && !pod.status_text.contains("BackOff")
                && !pod.status_text.contains("Error");

            if healthy {
                ResourceRow::healthy(&pod.name, cells)
            } else {
                ResourceRow::unhealthy(&pod.name, cells)
            }
        })
        .collect()
}

/// Rows for a deployment list.
#[must_use]
pub fn deployment_rows(deployments: &[DeploymentView]) -> Vec<ResourceRow> {
    deployments
        .iter()
        .map(|deployment| {
            let cells = vec![
                deployment.name.clone(),
                format!("{}/{}", deployment.ready, deployment.desired),
                deployment.up_to_date.to_string(),
                deployment.available.to_string(),
                age_cell(deployment.created),
            ];

            // A deployment scaled deliberately to zero is not unhealthy; one
            // that wants replicas and does not have them is.
            let short = deployment.desired > 0 && deployment.ready < deployment.desired;
            if short || deployment.paused {
                ResourceRow::unhealthy(&deployment.name, cells)
            } else {
                ResourceRow::healthy(&deployment.name, cells)
            }
        })
        .collect()
}

/// Rows for a service list.
#[must_use]
pub fn service_rows(services: &[ServiceView]) -> Vec<ResourceRow> {
    services
        .iter()
        .map(|service| {
            ResourceRow::healthy(
                &service.name,
                vec![
                    service.name.clone(),
                    service.service_type.clone(),
                    service
                        .cluster_ip
                        .clone()
                        .unwrap_or_else(|| "<none>".to_string()),
                    port_summary(&service.ports),
                    age_cell(service.created),
                ],
            )
        })
        .collect()
}

/// Rows for a node list.
#[must_use]
pub fn node_rows(nodes: &[NodeView]) -> Vec<ResourceRow> {
    nodes
        .iter()
        .map(|node| {
            let status = match node.ready {
                NodeReady::Ready => "Ready".to_string(),
                NodeReady::NotReady => "NotReady".to_string(),
                NodeReady::Unknown => "Unknown".to_string(),
            };
            let status = if node.schedulable {
                status
            } else {
                format!("{status},SchedulingDisabled")
            };

            let cells = vec![
                node.name.clone(),
                status,
                if node.roles.is_empty() {
                    "<none>".to_string()
                } else {
                    node.roles.join(",")
                },
                node.kubelet_version
                    .clone()
                    .unwrap_or_else(|| "<unknown>".to_string()),
                age_cell(node.created),
            ];

            if node.ready == NodeReady::Ready {
                ResourceRow::healthy(&node.name, cells)
            } else {
                ResourceRow::unhealthy(&node.name, cells)
            }
        })
        .collect()
}

/// Rows for an event list.
#[must_use]
pub fn event_rows(events: &[EventView]) -> Vec<ResourceRow> {
    events
        .iter()
        .map(|event| {
            let object = event_object(event);
            let cells = vec![
                event.event_type.clone(),
                event.reason.clone().unwrap_or_else(|| "<none>".to_string()),
                object.clone(),
                truncate(&event.message, MAX_EVENT_MESSAGE),
                age_cell(event.last_seen),
            ];

            // Kubernetes uses exactly two event types, and "Normal" is the
            // one that is not a problem.
            if event.event_type == "Normal" {
                ResourceRow::healthy(object, cells)
            } else {
                ResourceRow::unhealthy(object, cells)
            }
        })
        .collect()
}

/// Keeps the rows matching `query`.
///
/// An empty query keeps everything, so the filter box can be left open.
#[must_use]
pub fn filter_rows(rows: Vec<ResourceRow>, query: &str) -> Vec<ResourceRow> {
    if query.trim().is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| row_matches(row, query))
        .collect()
}

/// True if any cell of `row` contains `query`, ignoring case.
///
/// Substring rather than prefix: the useful search in a pod list is for the
/// deployment name in the middle of a generated pod name.
#[must_use]
pub fn row_matches(row: &ResourceRow, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    row.cells
        .iter()
        .any(|cell| cell.to_lowercase().contains(&needle))
}

/// Names the object an event is about, as `kind/name`.
///
/// Either part can be missing on a malformed event; showing `pod/` or `/api-0`
/// is still more use than an empty column.
fn event_object(event: &EventView) -> String {
    match (event.object_kind.as_deref(), event.object_name.as_deref()) {
        (Some(kind), Some(name)) => format!("{}/{name}", kind.to_lowercase()),
        (Some(kind), None) => kind.to_lowercase(),
        (None, Some(name)) => name.to_string(),
        (None, None) => event.name.clone(),
    }
}

/// The age cell, or a dash when the object carries no creation time.
///
/// Takes the clock rather than reading it, so a test can assert on a rendered
/// age instead of on whatever "now" happens to be while it runs.
fn age_cell_at(
    created: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    age_at(created, now).map_or_else(|| "-".to_string(), format_age)
}

/// The age cell, against the current time.
fn age_cell(created: Option<chrono::DateTime<chrono::Utc>>) -> String {
    age_cell_at(created, chrono::Utc::now())
}

/// Summarises a port list, counting any beyond the first few.
fn port_summary(ports: &[crate::k8s::ServicePortView]) -> String {
    if ports.is_empty() {
        return "<none>".to_string();
    }

    let shown: Vec<String> = ports
        .iter()
        .take(MAX_PORTS_SHOWN)
        .map(|port| match port.node_port {
            Some(node_port) => format!("{}:{}/{}", port.port, node_port, port.protocol),
            None => format!("{}/{}", port.port, port.protocol),
        })
        .collect();

    if ports.len() > MAX_PORTS_SHOWN {
        format!("{} +{}", shown.join(","), ports.len() - MAX_PORTS_SHOWN)
    } else {
        shown.join(",")
    }
}

/// Cuts `text` to `limit` characters, marking that it was cut.
///
/// Counts characters, not bytes: a message with a non-ASCII character in it
/// would otherwise panic on a byte slice that lands mid-character.
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{head}…")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::k8s::PodPhase;
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;

    /// A pod with the given readiness and status text.
    fn pod(name: &str, ready: u32, total: u32, status: &str) -> PodView {
        PodView {
            name: name.to_string(),
            namespace: "default".to_string(),
            phase: PodPhase::Running,
            status_text: status.to_string(),
            ready_containers: ready,
            total_containers: total,
            restarts: 0,
            node: Some("node-1".to_string()),
            pod_ip: None,
            images: Vec::new(),
            labels: BTreeMap::new(),
            created: Utc.timestamp_opt(1_757_000_000, 0).single(),
        }
    }

    /// A deployment with the given replica counts.
    fn deployment(name: &str, ready: u32, desired: u32) -> DeploymentView {
        DeploymentView {
            name: name.to_string(),
            namespace: "default".to_string(),
            desired,
            ready,
            up_to_date: ready,
            available: ready,
            images: Vec::new(),
            labels: BTreeMap::new(),
            paused: false,
            created: Utc.timestamp_opt(1_757_000_000, 0).single(),
        }
    }

    #[test]
    fn an_empty_list_produces_no_rows() {
        assert!(pod_rows(&[]).is_empty());
        assert!(deployment_rows(&[]).is_empty());
        assert!(service_rows(&[]).is_empty());
        assert!(node_rows(&[]).is_empty());
        assert!(event_rows(&[]).is_empty());
    }

    #[test]
    fn a_running_pod_reads_as_healthy() {
        let rows = pod_rows(&[pod("api-0", 2, 2, "Running")]);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].unhealthy);
        assert_eq!(rows[0].key, "api-0");
        assert_eq!(rows[0].cells[1], "2/2");
    }

    #[test]
    fn a_pod_with_a_container_down_reads_as_unhealthy() {
        let rows = pod_rows(&[pod("api-0", 1, 2, "Running")]);
        assert!(rows[0].unhealthy, "one of two containers is not ready");
    }

    #[test]
    fn a_crash_looping_pod_reads_as_unhealthy_even_when_counts_agree() {
        // A pod stuck in CrashLoopBackOff can report 0/1 or 1/1 depending on
        // where in the loop it is; the status text is the reliable signal.
        let rows = pod_rows(&[pod("api-0", 1, 1, "CrashLoopBackOff")]);
        assert!(rows[0].unhealthy);
    }

    #[test]
    fn a_pod_with_no_containers_is_not_called_healthy() {
        // 0/0 satisfies "ready equals total" but describes a pod that has not
        // started, not one that is fine.
        let rows = pod_rows(&[pod("api-0", 0, 0, "Pending")]);
        assert!(rows[0].unhealthy);
    }

    #[test]
    fn an_unscheduled_pod_shows_no_node_rather_than_an_empty_cell() {
        let mut unscheduled = pod("api-0", 0, 1, "Pending");
        unscheduled.node = None;
        let rows = pod_rows(&[unscheduled]);
        assert_eq!(rows[0].cells[4], "<none>");
    }

    #[test]
    fn a_pod_with_no_creation_time_shows_a_dash() {
        let mut undated = pod("api-0", 1, 1, "Running");
        undated.created = None;
        let rows = pod_rows(&[undated]);
        assert_eq!(rows[0].cells[5], "-");
    }

    #[test]
    fn a_deployment_short_of_replicas_reads_as_unhealthy() {
        let rows = deployment_rows(&[deployment("api", 1, 3)]);
        assert!(rows[0].unhealthy);
        assert_eq!(rows[0].cells[1], "1/3");
    }

    #[test]
    fn a_deployment_scaled_to_zero_is_not_unhealthy() {
        // Scaling to zero is something the user did on purpose.
        let rows = deployment_rows(&[deployment("api", 0, 0)]);
        assert!(!rows[0].unhealthy);
    }

    #[test]
    fn a_paused_deployment_reads_as_unhealthy() {
        let mut paused = deployment("api", 3, 3);
        paused.paused = true;
        let rows = deployment_rows(&[paused]);
        assert!(rows[0].unhealthy, "a paused rollout is stuck, not finished");
    }

    #[test]
    fn a_warning_event_reads_as_unhealthy_and_a_normal_one_does_not() {
        let warning = EventView {
            name: "api-0.17abc".to_string(),
            namespace: "default".to_string(),
            event_type: "Warning".to_string(),
            reason: Some("BackOff".to_string()),
            message: "Back-off restarting failed container".to_string(),
            object_kind: Some("Pod".to_string()),
            object_name: Some("api-0".to_string()),
            count: 4,
            last_seen: Utc.timestamp_opt(1_757_000_000, 0).single(),
            first_seen: None,
        };
        let mut normal = warning.clone();
        normal.event_type = "Normal".to_string();

        let rows = event_rows(&[warning, normal]);
        assert!(rows[0].unhealthy);
        assert!(!rows[1].unhealthy);
    }

    #[test]
    fn a_long_event_message_is_cut_rather_than_widening_the_table() {
        let event = EventView {
            name: "api-0.17abd".to_string(),
            namespace: "default".to_string(),
            event_type: "Warning".to_string(),
            reason: Some("Failed".to_string()),
            message: "x".repeat(500),
            object_kind: Some("Pod".to_string()),
            object_name: Some("api-0".to_string()),
            count: 1,
            last_seen: None,
            first_seen: None,
        };

        let rows = event_rows(&[event]);
        assert_eq!(rows[0].cells[3].chars().count(), MAX_EVENT_MESSAGE);
        assert!(rows[0].cells[3].ends_with('…'));
    }

    #[test]
    fn truncation_counts_characters_rather_than_bytes() {
        // A byte slice through a multi-byte character panics.
        let text = "é".repeat(100);
        let cut = truncate(&text, 10);
        assert_eq!(cut.chars().count(), 10);
    }

    #[test]
    fn a_short_message_is_left_alone() {
        assert_eq!(truncate("short", 60), "short");
    }

    #[test]
    fn an_age_is_rendered_against_the_clock_it_is_given() {
        let created = Utc.timestamp_opt(1_757_000_000, 0).single();
        let now = Utc
            .timestamp_opt(1_757_000_000 + 3 * 60 * 60, 0)
            .single()
            .expect("a time");
        assert_eq!(age_cell_at(created, now), "3h");
    }

    #[test]
    fn an_object_with_no_timestamp_ages_to_a_dash() {
        let now = Utc
            .timestamp_opt(1_757_000_000, 0)
            .single()
            .expect("a time");
        assert_eq!(age_cell_at(None, now), "-");
    }

    #[test]
    fn a_timestamp_from_the_future_does_not_render_a_negative_age() {
        // The cluster's clock ahead of this machine's is not a fault the user
        // can act on, and "-2h" would look like a bug in ratterm.
        let created = Utc.timestamp_opt(1_757_000_000 + 7_200, 0).single();
        let now = Utc
            .timestamp_opt(1_757_000_000, 0)
            .single()
            .expect("a time");
        assert_eq!(age_cell_at(created, now), "0s");
    }

    #[test]
    fn an_event_names_the_object_it_is_about() {
        let event = EventView {
            name: "api-0.17abc".to_string(),
            namespace: "default".to_string(),
            event_type: "Warning".to_string(),
            reason: Some("BackOff".to_string()),
            message: "back-off".to_string(),
            object_kind: Some("Pod".to_string()),
            object_name: Some("api-0".to_string()),
            count: 1,
            last_seen: None,
            first_seen: None,
        };
        assert_eq!(event_object(&event), "pod/api-0");

        let mut kind_only = event.clone();
        kind_only.object_name = None;
        assert_eq!(event_object(&kind_only), "pod");

        let mut name_only = event.clone();
        name_only.object_kind = None;
        assert_eq!(event_object(&name_only), "api-0");

        let mut neither = event;
        neither.object_kind = None;
        neither.object_name = None;
        assert_eq!(
            event_object(&neither),
            "api-0.17abc",
            "the event's own name is better than an empty column"
        );
    }

    #[test]
    fn an_event_with_no_reason_says_so_rather_than_leaving_a_gap() {
        let event = EventView {
            name: "api-0.17abc".to_string(),
            namespace: "default".to_string(),
            event_type: "Normal".to_string(),
            reason: None,
            message: "something".to_string(),
            object_kind: None,
            object_name: None,
            count: 1,
            last_seen: None,
            first_seen: None,
        };
        let rows = event_rows(&[event]);
        assert_eq!(rows[0].cells[1], "<none>");
    }

    #[test]
    fn a_service_with_no_ports_says_so() {
        assert_eq!(port_summary(&[]), "<none>");
    }

    #[test]
    fn service_ports_beyond_the_first_few_are_counted() {
        let ports: Vec<crate::k8s::ServicePortView> = (0..7)
            .map(|index| crate::k8s::ServicePortView {
                name: None,
                port: 8000 + index,
                target_port: None,
                node_port: None,
                protocol: "TCP".to_string(),
            })
            .collect();

        let summary = port_summary(&ports);
        assert!(summary.contains("+4"), "{summary}");
        assert!(summary.contains("8000/TCP"), "{summary}");
    }

    #[test]
    fn a_node_port_is_shown_beside_its_port() {
        let ports = vec![crate::k8s::ServicePortView {
            name: None,
            port: 80,
            target_port: None,
            node_port: Some(30_080),
            protocol: "TCP".to_string(),
        }];
        assert_eq!(port_summary(&ports), "80:30080/TCP");
    }

    #[test]
    fn a_not_ready_node_reads_as_unhealthy() {
        let node = NodeView {
            name: "node-1".to_string(),
            ready: NodeReady::NotReady,
            roles: vec!["worker".to_string()],
            schedulable: true,
            kubelet_version: Some("v1.33.0".to_string()),
            internal_ip: None,
            os_image: None,
            architecture: None,
            labels: BTreeMap::new(),
            created: None,
        };
        let rows = node_rows(&[node]);
        assert!(rows[0].unhealthy);
        assert_eq!(rows[0].cells[1], "NotReady");
    }

    #[test]
    fn a_cordoned_node_says_so_in_its_status() {
        let node = NodeView {
            name: "node-1".to_string(),
            ready: NodeReady::Ready,
            roles: Vec::new(),
            schedulable: false,
            kubelet_version: Some("v1.33.0".to_string()),
            internal_ip: None,
            os_image: None,
            architecture: None,
            labels: BTreeMap::new(),
            created: None,
        };
        let rows = node_rows(&[node]);
        assert_eq!(rows[0].cells[1], "Ready,SchedulingDisabled");
        assert_eq!(rows[0].cells[2], "<none>", "a node with no roles");
    }

    #[test]
    fn an_empty_filter_keeps_everything() {
        let rows = pod_rows(&[pod("api-0", 1, 1, "Running"), pod("web-0", 1, 1, "Running")]);
        assert_eq!(filter_rows(rows.clone(), "").len(), 2);
        assert_eq!(filter_rows(rows, "   ").len(), 2);
    }

    #[test]
    fn a_filter_matches_anywhere_in_any_cell() {
        let rows = pod_rows(&[
            pod("checkout-7d9f-abc", 1, 1, "Running"),
            pod("web-0", 1, 1, "Running"),
        ]);

        // The useful search is for the deployment name in the middle of a
        // generated pod name.
        let filtered = filter_rows(rows.clone(), "7d9f");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "checkout-7d9f-abc");

        // And it reaches other columns, such as the node.
        assert_eq!(filter_rows(rows, "node-1").len(), 2);
    }

    #[test]
    fn a_filter_ignores_case() {
        let rows = pod_rows(&[pod("API-0", 1, 1, "Running")]);
        assert_eq!(filter_rows(rows, "api").len(), 1);
    }

    #[test]
    fn a_filter_that_matches_nothing_yields_nothing() {
        let rows = pod_rows(&[pod("api-0", 1, 1, "Running")]);
        assert!(filter_rows(rows, "zzzz").is_empty());
    }

    #[test]
    fn row_matching_is_the_same_rule_the_filter_uses() {
        let rows = pod_rows(&[pod("api-0", 1, 1, "Running")]);
        assert!(row_matches(&rows[0], "api"));
        assert!(row_matches(&rows[0], ""), "an empty query matches");
        assert!(!row_matches(&rows[0], "nope"));
    }
}
