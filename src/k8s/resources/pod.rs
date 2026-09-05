//! The pod list view.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Pod;

use super::phase::{PodPhase, derive_status_text};
use super::{age_at, age_display, labels_of, name_of, namespace_of, to_utc};

/// A pod as a list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodView {
    /// Pod name.
    pub name: String,
    /// Namespace the pod is in.
    pub namespace: String,
    /// Lifecycle phase.
    pub phase: PodPhase,
    /// What to show in the status column.
    ///
    /// This is not always the phase: a pod being deleted shows `Terminating`,
    /// and a pod whose container is stuck shows the waiting reason
    /// (`CrashLoopBackOff`, `ImagePullBackOff`), which is what the user needs
    /// to see.
    pub status_text: String,
    /// Containers reporting ready.
    pub ready_containers: u32,
    /// Containers in the pod.
    pub total_containers: u32,
    /// Total restarts across all containers.
    pub restarts: u32,
    /// Node the pod is scheduled on, absent while it is unscheduled.
    pub node: Option<String>,
    /// Pod IP, absent until one is allocated.
    pub pod_ip: Option<String>,
    /// Container images, in spec order.
    pub images: Vec<String>,
    /// Pod labels.
    pub labels: BTreeMap<String, String>,
    /// Creation time, absent only on malformed objects.
    pub created: Option<DateTime<Utc>>,
}

impl PodView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(pod: &Pod) -> Self {
        let meta = &pod.metadata;
        let status = pod.status.as_ref();

        let phase = PodPhase::parse(status.and_then(|s| s.phase.as_deref()).unwrap_or_default());

        let container_statuses = status
            .and_then(|s| s.container_statuses.as_ref())
            .map(Vec::as_slice)
            .unwrap_or_default();

        let ready_containers = u32::try_from(container_statuses.iter().filter(|c| c.ready).count())
            .unwrap_or(u32::MAX);
        let restarts = container_statuses
            .iter()
            .map(|c| u32::try_from(c.restart_count).unwrap_or(0))
            .sum();

        let spec = pod.spec.as_ref();
        let images: Vec<String> = spec
            .map(|s| {
                s.containers
                    .iter()
                    .filter_map(|c| c.image.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Prefer the spec's container count: a pod that has not started yet
        // has containers in the spec but no statuses.
        let total_containers = spec.map_or_else(
            || u32::try_from(container_statuses.len()).unwrap_or(u32::MAX),
            |s| u32::try_from(s.containers.len()).unwrap_or(u32::MAX),
        );

        let status_text = derive_status_text(pod, &phase);

        Self {
            name: name_of(meta),
            namespace: namespace_of(meta),
            phase,
            status_text,
            ready_containers,
            total_containers,
            restarts,
            node: spec
                .and_then(|s| s.node_name.clone())
                .filter(|n| !n.is_empty()),
            pod_ip: status
                .and_then(|s| s.pod_ip.clone())
                .filter(|ip| !ip.is_empty()),
            images,
            labels: labels_of(meta),
            created: to_utc(meta.creation_timestamp.as_ref()),
        }
    }

    /// Returns the pod's age at `now`.
    #[must_use]
    pub fn age(&self, now: DateTime<Utc>) -> Option<Duration> {
        age_at(self.created, now)
    }

    /// Returns the short age string for the list, or `-`.
    #[must_use]
    pub fn age_display(&self, now: DateTime<Utc>) -> String {
        age_display(self.created, now)
    }

    /// Returns the ready column, for example `2/3`.
    #[must_use]
    pub fn ready_display(&self) -> String {
        format!("{}/{}", self.ready_containers, self.total_containers)
    }

    /// True when every container in the pod reports ready.
    ///
    /// A pod with no containers at all is not treated as ready; that shape
    /// only occurs on a malformed object.
    #[must_use]
    pub const fn all_ready(&self) -> bool {
        self.total_containers > 0 && self.ready_containers == self.total_containers
    }
}

/// Orders pods for a list: namespace, then name.
///
/// Both keys together are unique within a cluster, so the order is total and
/// does not change between refreshes.
#[must_use]
pub fn compare_pods(a: &PodView, b: &PodView) -> Ordering {
    a.namespace
        .cmp(&b.namespace)
        .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::core::v1::{
        Container, ContainerState, ContainerStateWaiting, ContainerStatus, PodSpec, PodStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::super::test_support::{time_at, utc_at};
    use super::*;

    fn container_status(name: &str, ready: bool, restarts: i32) -> ContainerStatus {
        ContainerStatus {
            name: name.to_string(),
            ready,
            restart_count: restarts,
            image: "nginx:1.27".to_string(),
            image_id: String::new(),
            ..ContainerStatus::default()
        }
    }

    fn full_pod() -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some("api-0".to_string()),
                namespace: Some("web".to_string()),
                labels: Some(BTreeMap::from([("app".to_string(), "api".to_string())])),
                creation_timestamp: Some(time_at(1_000)),
                ..ObjectMeta::default()
            },
            spec: Some(PodSpec {
                node_name: Some("node-1".to_string()),
                containers: vec![
                    Container {
                        name: "api".to_string(),
                        image: Some("ghcr.io/example/api:1.4".to_string()),
                        ..Container::default()
                    },
                    Container {
                        name: "sidecar".to_string(),
                        image: Some("envoy:1.31".to_string()),
                        ..Container::default()
                    },
                ],
                ..PodSpec::default()
            }),
            status: Some(PodStatus {
                phase: Some("Running".to_string()),
                pod_ip: Some("10.42.0.7".to_string()),
                container_statuses: Some(vec![
                    container_status("api", true, 2),
                    container_status("sidecar", false, 1),
                ]),
                ..PodStatus::default()
            }),
        }
    }

    #[test]
    fn a_fully_populated_pod_converts_every_field() {
        let view = PodView::from_api(&full_pod());

        assert_eq!(view.name, "api-0");
        assert_eq!(view.namespace, "web");
        assert_eq!(view.phase, PodPhase::Running);
        assert_eq!(view.status_text, "Running");
        assert_eq!(view.ready_containers, 1);
        assert_eq!(view.total_containers, 2);
        assert_eq!(view.restarts, 3);
        assert_eq!(view.node.as_deref(), Some("node-1"));
        assert_eq!(view.pod_ip.as_deref(), Some("10.42.0.7"));
        assert_eq!(view.images, vec!["ghcr.io/example/api:1.4", "envoy:1.31"]);
        assert_eq!(view.labels.get("app").map(String::as_str), Some("api"));
        assert_eq!(view.created, Some(utc_at(1_000)));
        assert_eq!(view.ready_display(), "1/2");
        assert!(!view.all_ready());
    }

    #[test]
    fn a_minimal_pod_converts_to_documented_defaults() {
        let view = PodView::from_api(&Pod::default());

        assert_eq!(view.name, "");
        assert_eq!(view.namespace, "");
        assert_eq!(view.phase, PodPhase::Unknown);
        assert_eq!(view.status_text, "Unknown");
        assert_eq!(view.ready_containers, 0);
        assert_eq!(view.total_containers, 0);
        assert_eq!(view.restarts, 0);
        assert!(view.node.is_none());
        assert!(view.pod_ip.is_none());
        assert!(view.images.is_empty());
        assert!(view.labels.is_empty());
        assert!(view.created.is_none());
        assert_eq!(view.ready_display(), "0/0");
        assert!(!view.all_ready());
        assert!(view.age(utc_at(1)).is_none());
        assert_eq!(view.age_display(utc_at(1)), "-");
    }

    #[test]
    fn an_unexpected_phase_is_kept_verbatim_without_panicking() {
        let mut pod = full_pod();
        if let Some(status) = pod.status.as_mut() {
            status.phase = Some("Rehydrating".to_string());
        }
        let view = PodView::from_api(&pod);
        assert_eq!(view.phase, PodPhase::Other("Rehydrating".to_string()));
        assert_eq!(view.status_text, "Rehydrating");
    }

    #[test]
    fn an_empty_node_name_or_ip_is_treated_as_absent() {
        let mut pod = full_pod();
        if let Some(spec) = pod.spec.as_mut() {
            spec.node_name = Some(String::new());
        }
        if let Some(status) = pod.status.as_mut() {
            status.pod_ip = Some(String::new());
        }
        let view = PodView::from_api(&pod);
        assert!(view.node.is_none());
        assert!(view.pod_ip.is_none());
    }

    #[test]
    fn a_blocked_container_shows_its_waiting_reason() {
        let mut pod = full_pod();
        if let Some(status) = pod.status.as_mut()
            && let Some(statuses) = status.container_statuses.as_mut()
        {
            statuses[0].state = Some(ContainerState {
                waiting: Some(ContainerStateWaiting {
                    reason: Some("CrashLoopBackOff".to_string()),
                    message: Some("back-off 5m0s".to_string()),
                }),
                ..ContainerState::default()
            });
        }
        assert_eq!(PodView::from_api(&pod).status_text, "CrashLoopBackOff");
    }

    #[test]
    fn a_pod_with_a_spec_but_no_statuses_counts_its_spec_containers() {
        let mut pod = full_pod();
        if let Some(status) = pod.status.as_mut() {
            status.container_statuses = None;
        }
        let view = PodView::from_api(&pod);
        assert_eq!(view.total_containers, 2);
        assert_eq!(view.ready_containers, 0);
        assert_eq!(view.ready_display(), "0/2");
    }

    #[test]
    fn a_pod_with_statuses_but_no_spec_counts_its_statuses() {
        let mut pod = full_pod();
        pod.spec = None;
        let view = PodView::from_api(&pod);
        assert_eq!(view.total_containers, 2);
        assert!(view.images.is_empty());
    }

    #[test]
    fn a_negative_restart_count_is_clamped() {
        let mut pod = full_pod();
        if let Some(status) = pod.status.as_mut()
            && let Some(statuses) = status.container_statuses.as_mut()
        {
            statuses[0].restart_count = -5;
        }
        assert_eq!(PodView::from_api(&pod).restarts, 1);
    }

    #[test]
    fn a_pod_with_every_container_ready_reports_ready() {
        let mut pod = full_pod();
        if let Some(status) = pod.status.as_mut()
            && let Some(statuses) = status.container_statuses.as_mut()
        {
            statuses[1].ready = true;
        }
        let view = PodView::from_api(&pod);
        assert!(view.all_ready());
        assert_eq!(view.ready_display(), "2/2");
    }

    #[test]
    fn age_uses_the_creation_timestamp() {
        let view = PodView::from_api(&full_pod());
        assert_eq!(view.age(utc_at(4_600)), Some(Duration::from_secs(3_600)));
        assert_eq!(view.age_display(utc_at(4_600)), "1h");
    }

    #[test]
    fn pods_sort_by_namespace_then_name() {
        let mut views = [
            view_named("web", "zebra"),
            view_named("api", "beta"),
            view_named("web", "alpha"),
            view_named("api", "alpha"),
        ];
        views.sort_by(compare_pods);

        let keys: Vec<String> = views
            .iter()
            .map(|v| format!("{}/{}", v.namespace, v.name))
            .collect();
        assert_eq!(
            keys,
            vec!["api/alpha", "api/beta", "web/alpha", "web/zebra"]
        );
    }

    #[test]
    fn the_pod_comparator_is_a_total_order() {
        let a = view_named("web", "alpha");
        let b = view_named("web", "beta");
        assert_eq!(compare_pods(&a, &a), Ordering::Equal);
        assert_eq!(compare_pods(&a, &b), Ordering::Less);
        assert_eq!(compare_pods(&b, &a), Ordering::Greater);
    }

    #[test]
    fn sorting_an_already_sorted_list_does_not_move_anything() {
        let mut views = [view_named("a", "one"), view_named("a", "two")];
        let before = views.clone();
        views.sort_by(compare_pods);
        assert_eq!(views, before);
    }

    fn view_named(namespace: &str, name: &str) -> PodView {
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };
        PodView::from_api(&pod)
    }
}
