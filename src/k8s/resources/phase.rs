//! Pod phase and the status text derived from it.

use k8s_openapi::api::core::v1::Pod;

/// The lifecycle phase of a pod.
///
/// `Other` exists because the phase is a free-form string in the API: a newer
/// cluster can report a phase this build has never heard of, and a list must
/// still render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodPhase {
    /// Accepted by the cluster, not all containers running yet.
    Pending,
    /// Bound to a node with at least one container running.
    Running,
    /// All containers exited successfully.
    Succeeded,
    /// At least one container exited in failure.
    Failed,
    /// The cluster could not determine the state, or did not report one.
    Unknown,
    /// A phase this build does not recognise, kept verbatim.
    Other(String),
}

impl PodPhase {
    /// Parses the API's phase string.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "Pending" => Self::Pending,
            "Running" => Self::Running,
            "Succeeded" => Self::Succeeded,
            "Failed" => Self::Failed,
            "Unknown" | "" => Self::Unknown,
            other => Self::Other(other.to_string()),
        }
    }

    /// Returns the phase as it is written in the API.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Unknown => "Unknown",
            Self::Other(raw) => raw,
        }
    }

    /// True for phases that do not call for attention.
    ///
    /// An unrecognised phase counts as needing attention: this build cannot
    /// tell whether it is benign.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(self, Self::Running | Self::Succeeded)
    }
}

/// Chooses the status column text for a pod.
///
/// The precedence is deletion, then a blocked container, then the pod-level
/// reason, then the phase — the same order `kubectl get pods` uses, because a
/// pod stuck in `CrashLoopBackOff` reports phase `Running`.
#[must_use]
pub fn derive_status_text(pod: &Pod, phase: &PodPhase) -> String {
    if pod.metadata.deletion_timestamp.is_some() {
        return "Terminating".to_string();
    }

    if let Some(status) = pod.status.as_ref() {
        if let Some(statuses) = status.container_statuses.as_ref() {
            for container in statuses {
                if let Some(state) = container.state.as_ref()
                    && let Some(waiting) = state.waiting.as_ref()
                    && let Some(reason) = waiting.reason.as_ref()
                    && !reason.is_empty()
                    && reason != "ContainerCreating"
                {
                    return reason.clone();
                }
            }
        }
        if let Some(reason) = status.reason.as_ref()
            && !reason.is_empty()
        {
            return reason.clone();
        }
    }

    phase.label().to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateWaiting, ContainerStatus, PodStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};

    use super::*;

    fn container_status(state: Option<ContainerState>) -> ContainerStatus {
        ContainerStatus {
            name: "api".to_string(),
            ready: true,
            restart_count: 0,
            image: "nginx:1.27".to_string(),
            image_id: String::new(),
            state,
            ..ContainerStatus::default()
        }
    }

    fn waiting(reason: &str) -> Option<ContainerState> {
        Some(ContainerState {
            waiting: Some(ContainerStateWaiting {
                reason: Some(reason.to_string()),
                message: None,
            }),
            ..ContainerState::default()
        })
    }

    fn running_pod(statuses: Vec<ContainerStatus>) -> Pod {
        Pod {
            status: Some(PodStatus {
                phase: Some("Running".to_string()),
                container_statuses: Some(statuses),
                ..PodStatus::default()
            }),
            ..Pod::default()
        }
    }

    #[test]
    fn known_phases_parse_and_round_trip() {
        for (raw, expected) in [
            ("Pending", PodPhase::Pending),
            ("Running", PodPhase::Running),
            ("Succeeded", PodPhase::Succeeded),
            ("Failed", PodPhase::Failed),
            ("Unknown", PodPhase::Unknown),
            ("", PodPhase::Unknown),
        ] {
            let parsed = PodPhase::parse(raw);
            assert_eq!(parsed, expected, "parsing {raw:?}");
            if !raw.is_empty() {
                assert_eq!(parsed.label(), raw);
            }
        }
    }

    #[test]
    fn an_unrecognised_phase_is_kept_verbatim() {
        let parsed = PodPhase::parse("Rehydrating");
        assert_eq!(parsed, PodPhase::Other("Rehydrating".to_string()));
        assert_eq!(parsed.label(), "Rehydrating");
        assert!(!parsed.is_settled());
    }

    #[test]
    fn only_running_and_succeeded_are_settled() {
        assert!(PodPhase::Running.is_settled());
        assert!(PodPhase::Succeeded.is_settled());
        assert!(!PodPhase::Failed.is_settled());
        assert!(!PodPhase::Pending.is_settled());
        assert!(!PodPhase::Unknown.is_settled());
    }

    #[test]
    fn a_pod_being_deleted_shows_terminating() {
        let mut pod = running_pod(vec![container_status(None)]);
        pod.metadata = ObjectMeta {
            deletion_timestamp: Some(Time(
                k8s_openapi::jiff::Timestamp::from_second(2_000).expect("timestamp"),
            )),
            ..ObjectMeta::default()
        };
        assert_eq!(derive_status_text(&pod, &PodPhase::Running), "Terminating");
    }

    #[test]
    fn a_blocked_container_shows_its_waiting_reason() {
        let pod = running_pod(vec![container_status(waiting("CrashLoopBackOff"))]);
        assert_eq!(
            derive_status_text(&pod, &PodPhase::Running),
            "CrashLoopBackOff"
        );
    }

    #[test]
    fn a_container_merely_being_created_does_not_override_the_phase() {
        let pod = running_pod(vec![container_status(waiting("ContainerCreating"))]);
        assert_eq!(derive_status_text(&pod, &PodPhase::Running), "Running");
    }

    #[test]
    fn an_empty_waiting_reason_does_not_override_the_phase() {
        let pod = running_pod(vec![container_status(waiting(""))]);
        assert_eq!(derive_status_text(&pod, &PodPhase::Running), "Running");
    }

    #[test]
    fn a_pod_level_reason_beats_the_phase() {
        let pod = Pod {
            status: Some(PodStatus {
                phase: Some("Failed".to_string()),
                reason: Some("Evicted".to_string()),
                ..PodStatus::default()
            }),
            ..Pod::default()
        };
        assert_eq!(derive_status_text(&pod, &PodPhase::Failed), "Evicted");
    }

    #[test]
    fn a_pod_with_no_status_falls_back_to_the_phase() {
        assert_eq!(
            derive_status_text(&Pod::default(), &PodPhase::Unknown),
            "Unknown"
        );
    }
}
