//! Acting on a container or a whole Compose project.
//!
//! Split out of `session.rs`, which owns the state; this owns what can be done
//! to it.

use super::compose_ops::{ProjectAction, ProjectOutcome, apply_blocking};
use super::error::DockerError;
use super::session::DockerFleetState;

/// What to do to one container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerAction {
    /// Start it.
    Start,
    /// Stop it.
    Stop,
    /// Restart it.
    Restart,
    /// Remove it, killing it first if it is running.
    Remove,
}

impl ContainerAction {
    /// The verb, for status lines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Remove => "remove",
        }
    }
}

impl DockerFleetState {
    /// Applies an action to one container on one host.
    ///
    /// # Errors
    /// Returns [`DockerError::UnknownHost`] if the host is not connected, or
    /// whatever the daemon said.
    pub fn container_action(
        &self,
        key: Option<u32>,
        container_id: &str,
        action: ContainerAction,
    ) -> Result<(), DockerError> {
        let client = self
            .fleet
            .client(key)
            .ok_or(DockerError::UnknownHost(key.unwrap_or(0)))?;

        match action {
            ContainerAction::Start => client.start_container_blocking(container_id),
            ContainerAction::Stop => client.stop_container_blocking(container_id),
            ContainerAction::Restart => client.restart_container_blocking(container_id),
            ContainerAction::Remove => client.remove_container_blocking(container_id, true),
        }
    }

    /// Applies an action to a whole Compose project on one host.
    ///
    /// # Errors
    /// Returns [`DockerError::UnknownHost`] if the host is not connected, or
    /// [`DockerError::UnknownComposeProject`] if no container carries that
    /// project label.
    pub fn compose_action(
        &self,
        key: Option<u32>,
        project: &str,
        action: ProjectAction,
    ) -> Result<ProjectOutcome, DockerError> {
        let client = self
            .fleet
            .client(key)
            .ok_or(DockerError::UnknownHost(key.unwrap_or(0)))?;
        let host = self
            .fleet
            .host(key)
            .ok_or(DockerError::UnknownHost(key.unwrap_or(0)))?;

        apply_blocking(&client, &host.snapshot.containers, project, action)
    }

    /// The Compose projects on one host, as summary lines.
    #[must_use]
    pub fn compose_lines(&self, key: Option<u32>) -> Vec<String> {
        let Some(grouping) = self.fleet.compose(key) else {
            return Vec::new();
        };
        let mut lines: Vec<String> = grouping
            .projects
            .iter()
            .map(super::compose::ComposeProject::summary)
            .collect();
        if !grouping.unmanaged.is_empty() {
            lines.push(format!(
                "{} container{} outside any Compose project",
                grouping.unmanaged.len(),
                if grouping.unmanaged.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        lines
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::docker::host::DockerHost;
    use std::collections::HashMap;

    #[test]
    fn an_action_on_a_host_that_is_not_connected_is_refused() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::remote(7), "rock5c");
        assert!(matches!(
            state.container_action(Some(7), "abc", ContainerAction::Stop),
            Err(DockerError::UnknownHost(7))
        ));
        assert!(matches!(
            state.compose_action(Some(7), "shop", ProjectAction::Stop),
            Err(DockerError::UnknownHost(7))
        ));
    }

    #[test]
    fn compose_lines_for_an_untracked_host_are_empty() {
        let state = DockerFleetState::live_only();
        assert!(state.compose_lines(Some(3)).is_empty());
    }

    /// A container detail with the Compose labels a real project carries.
    fn detail(name: &str, project: Option<&str>) -> crate::docker::model::ContainerDetail {
        use crate::docker::compose::{PROJECT_LABEL, SERVICE_LABEL};
        use crate::docker::container::DockerContainer;

        let labels = match project {
            Some(project) => HashMap::from([
                (PROJECT_LABEL.to_string(), project.to_string()),
                (SERVICE_LABEL.to_string(), name.to_string()),
            ]),
            None => HashMap::new(),
        };
        crate::docker::model::ContainerDetail {
            container: DockerContainer::new(
                format!("id-{name}"),
                name.to_string(),
                "img".to_string(),
                "Up".to_string(),
            ),
            labels,
            state: "running".to_string(),
            command: String::new(),
        }
    }

    #[test]
    fn compose_lines_list_projects_and_count_what_is_outside_them() {
        use crate::docker::client::HostSnapshot;

        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::Local, "Local");
        state.fleet.record_snapshot(
            None,
            "unix socket /var/run/docker.sock".to_string(),
            "the local daemon socket is present",
            HostSnapshot {
                containers: vec![detail("web", Some("shop")), detail("loose", None)],
                ..Default::default()
            },
            100,
        );

        let lines = state.compose_lines(None);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("shop"), "{}", lines[0]);
        assert!(lines[0].contains("1/1 running"), "{}", lines[0]);
        assert!(lines[1].contains("1 container outside"), "{}", lines[1]);
    }

    #[test]
    fn a_host_with_only_compose_containers_has_no_outside_line() {
        use crate::docker::client::HostSnapshot;

        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::Local, "Local");
        state.fleet.record_snapshot(
            None,
            "unix socket".to_string(),
            "test",
            HostSnapshot {
                containers: vec![detail("web", Some("shop"))],
                ..Default::default()
            },
            100,
        );
        let lines = state.compose_lines(None);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains("outside"), "{}", lines[0]);
    }

    #[test]
    fn a_host_with_no_data_has_no_compose_lines() {
        let mut state = DockerFleetState::live_only();
        state.fleet.track(&DockerHost::Local, "Local");
        assert!(state.compose_lines(None).is_empty());
    }

    #[test]
    fn container_actions_have_verbs() {
        assert_eq!(ContainerAction::Start.as_str(), "start");
        assert_eq!(ContainerAction::Stop.as_str(), "stop");
        assert_eq!(ContainerAction::Restart.as_str(), "restart");
        assert_eq!(ContainerAction::Remove.as_str(), "remove");
    }
}
