//! The deployment list view.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::api::apps::v1::Deployment;

use super::{age_at, age_display, count_of, labels_of, name_of, namespace_of, to_utc};

/// A deployment as a list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentView {
    /// Deployment name.
    pub name: String,
    /// Namespace the deployment is in.
    pub namespace: String,
    /// Replicas asked for in the spec. Defaults to 1, as the API does.
    pub desired: u32,
    /// Replicas reporting ready.
    pub ready: u32,
    /// Replicas running the current pod template.
    pub up_to_date: u32,
    /// Replicas available for the minimum ready time.
    pub available: u32,
    /// Container images from the pod template, in spec order.
    pub images: Vec<String>,
    /// Deployment labels.
    pub labels: BTreeMap<String, String>,
    /// True when the rollout is paused.
    pub paused: bool,
    /// Creation time, absent only on malformed objects.
    pub created: Option<DateTime<Utc>>,
}

impl DeploymentView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(deployment: &Deployment) -> Self {
        let meta = &deployment.metadata;
        let spec = deployment.spec.as_ref();
        let status = deployment.status.as_ref();

        // The API defaults `spec.replicas` to 1 when it is omitted, so an
        // absent value is one replica rather than none.
        let desired = spec
            .and_then(|s| s.replicas)
            .map_or(1, |r| u32::try_from(r).unwrap_or(0));

        let images = spec
            .and_then(|s| s.template.spec.as_ref())
            .map(|pod_spec| {
                pod_spec
                    .containers
                    .iter()
                    .filter_map(|c| c.image.clone())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            name: name_of(meta),
            namespace: namespace_of(meta),
            desired,
            ready: count_of(status.and_then(|s| s.ready_replicas)),
            up_to_date: count_of(status.and_then(|s| s.updated_replicas)),
            available: count_of(status.and_then(|s| s.available_replicas)),
            images,
            labels: labels_of(meta),
            paused: spec.and_then(|s| s.paused).unwrap_or(false),
            created: to_utc(meta.creation_timestamp.as_ref()),
        }
    }

    /// Returns the deployment's age at `now`.
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
        format!("{}/{}", self.ready, self.desired)
    }

    /// True when every requested replica is ready and up to date.
    #[must_use]
    pub const fn is_rolled_out(&self) -> bool {
        self.ready == self.desired && self.up_to_date == self.desired
    }
}

/// Orders deployments for a list: namespace, then name.
#[must_use]
pub fn compare_deployments(a: &DeploymentView, b: &DeploymentView) -> Ordering {
    a.namespace
        .cmp(&b.namespace)
        .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::apps::v1::{DeploymentSpec, DeploymentStatus};
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};

    use super::super::test_support::{time_at, utc_at};
    use super::*;

    fn full_deployment() -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                name: Some("api".to_string()),
                namespace: Some("web".to_string()),
                labels: Some(BTreeMap::from([("tier".to_string(), "front".to_string())])),
                creation_timestamp: Some(time_at(1_000)),
                ..ObjectMeta::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(3),
                paused: Some(false),
                selector: LabelSelector::default(),
                template: PodTemplateSpec {
                    metadata: None,
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "api".to_string(),
                            image: Some("ghcr.io/example/api:1.4".to_string()),
                            ..Container::default()
                        }],
                        ..PodSpec::default()
                    }),
                },
                ..DeploymentSpec::default()
            }),
            status: Some(DeploymentStatus {
                ready_replicas: Some(2),
                updated_replicas: Some(3),
                available_replicas: Some(2),
                replicas: Some(3),
                ..DeploymentStatus::default()
            }),
        }
    }

    #[test]
    fn a_fully_populated_deployment_converts_every_field() {
        let view = DeploymentView::from_api(&full_deployment());

        assert_eq!(view.name, "api");
        assert_eq!(view.namespace, "web");
        assert_eq!(view.desired, 3);
        assert_eq!(view.ready, 2);
        assert_eq!(view.up_to_date, 3);
        assert_eq!(view.available, 2);
        assert_eq!(view.images, vec!["ghcr.io/example/api:1.4"]);
        assert_eq!(view.labels.get("tier").map(String::as_str), Some("front"));
        assert!(!view.paused);
        assert_eq!(view.created, Some(utc_at(1_000)));
        assert_eq!(view.ready_display(), "2/3");
        assert!(!view.is_rolled_out());
    }

    #[test]
    fn a_minimal_deployment_converts_to_documented_defaults() {
        let view = DeploymentView::from_api(&Deployment::default());

        assert_eq!(view.name, "");
        assert_eq!(view.namespace, "");
        // The API's own default for an omitted replica count.
        assert_eq!(view.desired, 1);
        assert_eq!(view.ready, 0);
        assert_eq!(view.up_to_date, 0);
        assert_eq!(view.available, 0);
        assert!(view.images.is_empty());
        assert!(view.labels.is_empty());
        assert!(!view.paused);
        assert!(view.created.is_none());
        assert_eq!(view.ready_display(), "0/1");
        assert!(!view.is_rolled_out());
    }

    #[test]
    fn a_status_with_unexpected_negative_counts_clamps_to_zero() {
        let mut deployment = full_deployment();
        if let Some(status) = deployment.status.as_mut() {
            status.ready_replicas = Some(-2);
            status.updated_replicas = Some(-1);
            status.available_replicas = None;
        }
        let view = DeploymentView::from_api(&deployment);
        assert_eq!(view.ready, 0);
        assert_eq!(view.up_to_date, 0);
        assert_eq!(view.available, 0);
    }

    #[test]
    fn a_fully_rolled_out_deployment_reports_it() {
        let mut deployment = full_deployment();
        if let Some(status) = deployment.status.as_mut() {
            status.ready_replicas = Some(3);
            status.updated_replicas = Some(3);
            status.available_replicas = Some(3);
        }
        assert!(DeploymentView::from_api(&deployment).is_rolled_out());
    }

    #[test]
    fn a_paused_rollout_is_reported() {
        let mut deployment = full_deployment();
        if let Some(spec) = deployment.spec.as_mut() {
            spec.paused = Some(true);
        }
        assert!(DeploymentView::from_api(&deployment).paused);
    }

    #[test]
    fn a_deployment_scaled_to_zero_keeps_a_zero_desired_count() {
        let mut deployment = full_deployment();
        if let Some(spec) = deployment.spec.as_mut() {
            spec.replicas = Some(0);
        }
        assert_eq!(DeploymentView::from_api(&deployment).desired, 0);
    }

    #[test]
    fn age_uses_the_creation_timestamp() {
        let view = DeploymentView::from_api(&full_deployment());
        assert_eq!(view.age(utc_at(1_060)), Some(Duration::from_secs(60)));
        assert_eq!(view.age_display(utc_at(1_060)), "1m");
        assert_eq!(
            DeploymentView::from_api(&Deployment::default()).age_display(utc_at(10)),
            "-"
        );
    }

    #[test]
    fn deployments_sort_by_namespace_then_name() {
        let mut views = [
            view_named("web", "zebra"),
            view_named("api", "beta"),
            view_named("web", "alpha"),
            view_named("api", "alpha"),
        ];
        views.sort_by(compare_deployments);

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
    fn the_deployment_comparator_is_a_total_order() {
        let a = view_named("web", "alpha");
        let b = view_named("web", "beta");
        assert_eq!(compare_deployments(&a, &a), Ordering::Equal);
        assert_eq!(compare_deployments(&a, &b), Ordering::Less);
        assert_eq!(compare_deployments(&b, &a), Ordering::Greater);

        let mut sorted = vec![a.clone(), b.clone()];
        let before = sorted.clone();
        sorted.sort_by(compare_deployments);
        assert_eq!(sorted, before);
    }

    fn view_named(namespace: &str, name: &str) -> DeploymentView {
        let mut deployment = Deployment::default();
        deployment.metadata.name = Some(name.to_string());
        deployment.metadata.namespace = Some(namespace.to_string());
        DeploymentView::from_api(&deployment)
    }
}
