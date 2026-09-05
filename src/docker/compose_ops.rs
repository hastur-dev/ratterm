//! Starting, stopping and restarting a whole Compose project.
//!
//! Compose has no server-side project object, so `docker compose start` is
//! really "start every container carrying this project label". That is what
//! these do, through the typed client, which means they work identically on a
//! remote host without a `docker compose` binary being installed there.

use super::client::DockerClient;
use super::compose::{ComposeGrouping, ComposeProject};
use super::error::DockerError;
use super::model::ContainerDetail;

/// What to do to every container in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectAction {
    /// Start every container that is not running.
    Start,
    /// Stop every container that is running.
    Stop,
    /// Restart every container.
    Restart,
}

impl ProjectAction {
    /// The verb, for status lines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

/// What happened when an action was applied to a project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectOutcome {
    /// Container ids the action succeeded on.
    pub changed: Vec<String>,
    /// Container ids that were already in the wanted state.
    pub skipped: Vec<String>,
    /// Container ids that failed, with the reason.
    pub failed: Vec<(String, String)>,
}

impl ProjectOutcome {
    /// True when nothing failed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// A one-line summary for the status bar.
    #[must_use]
    pub fn summary(&self, project: &str, action: ProjectAction) -> String {
        let mut line = format!(
            "{} {}: {} changed",
            action.as_str(),
            project,
            self.changed.len()
        );
        if !self.skipped.is_empty() {
            line.push_str(&format!(", {} already there", self.skipped.len()));
        }
        if !self.failed.is_empty() {
            line.push_str(&format!(", {} failed", self.failed.len()));
            if let Some((id, reason)) = self.failed.first() {
                let short: String = id.chars().take(12).collect();
                line.push_str(&format!(" ({short}: {reason})"));
            }
        }
        line
    }
}

/// Decides which containers an action actually needs to touch.
///
/// Pure, so the "already running" and "already stopped" rules are testable
/// without a daemon. Returns `(to_act_on, already_there)` in project order.
#[must_use]
pub fn plan(project: &ComposeProject, action: ProjectAction) -> (Vec<String>, Vec<String>) {
    let mut act = Vec::new();
    let mut skip = Vec::new();

    for container in project.containers() {
        let wanted = match action {
            ProjectAction::Start => !container.is_running(),
            ProjectAction::Stop => container.is_running(),
            ProjectAction::Restart => true,
        };
        if wanted {
            act.push(container.id.clone());
        } else {
            skip.push(container.id.clone());
        }
    }

    (act, skip)
}

/// Applies an action to every container of one project on one host.
///
/// # Errors
/// Returns [`DockerError::UnknownComposeProject`] if no container on the host
/// carries that project label. Per-container failures are reported in the
/// outcome rather than aborting: stopping four of five services and saying
/// which one refused is more useful than stopping none.
pub async fn apply(
    client: &DockerClient,
    containers: &[ContainerDetail],
    project_name: &str,
    action: ProjectAction,
) -> Result<ProjectOutcome, DockerError> {
    let grouping: ComposeGrouping = super::compose::group_by_project(containers);
    let project = grouping
        .project(project_name)
        .ok_or_else(|| DockerError::UnknownComposeProject(project_name.to_string()))?;

    let (act_on, skipped) = plan(project, action);
    let mut outcome = ProjectOutcome {
        skipped,
        ..Default::default()
    };

    for id in act_on {
        let result = match action {
            ProjectAction::Start => client.start_container(&id).await,
            ProjectAction::Stop => client.stop_container(&id).await,
            ProjectAction::Restart => client.restart_container(&id).await,
        };
        match result {
            Ok(()) => outcome.changed.push(id),
            Err(e) => outcome.failed.push((id, e.to_string())),
        }
    }

    Ok(outcome)
}

/// [`apply`] from synchronous code.
///
/// # Errors
/// Returns the reason the action could not be attempted.
pub fn apply_blocking(
    client: &DockerClient,
    containers: &[ContainerDetail],
    project_name: &str,
    action: ProjectAction,
) -> Result<ProjectOutcome, DockerError> {
    super::runtime::block_on(apply(client, containers, project_name, action))?
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::compose::{
        CONTAINER_NUMBER_LABEL, PROJECT_LABEL, SERVICE_LABEL, group_by_project,
    };
    use super::super::container::DockerContainer;
    use super::super::transport::TransportChoice;
    use super::*;
    use std::collections::HashMap;

    fn detail(name: &str, status: &str, service: &str, number: &str) -> ContainerDetail {
        ContainerDetail {
            container: DockerContainer::new(
                format!("id-{name}"),
                name.to_string(),
                "img".to_string(),
                status.to_string(),
            ),
            labels: HashMap::from([
                (PROJECT_LABEL.to_string(), "shop".to_string()),
                (SERVICE_LABEL.to_string(), service.to_string()),
                (CONTAINER_NUMBER_LABEL.to_string(), number.to_string()),
            ]),
            state: status.to_lowercase(),
            command: String::new(),
        }
    }

    fn mixed_project() -> Vec<ContainerDetail> {
        vec![
            detail("shop-web-1", "Up 2 hours", "web", "1"),
            detail("shop-db-1", "Exited (0) 1 hour ago", "db", "1"),
        ]
    }

    #[test]
    fn starting_a_project_only_touches_the_stopped_containers() {
        let grouping = group_by_project(&mixed_project());
        let project = grouping.project("shop").expect("the project is there");
        let (act, skip) = plan(project, ProjectAction::Start);
        assert_eq!(act, vec!["id-shop-db-1"]);
        assert_eq!(skip, vec!["id-shop-web-1"]);
    }

    #[test]
    fn stopping_a_project_only_touches_the_running_containers() {
        let grouping = group_by_project(&mixed_project());
        let project = grouping.project("shop").expect("the project is there");
        let (act, skip) = plan(project, ProjectAction::Stop);
        assert_eq!(act, vec!["id-shop-web-1"]);
        assert_eq!(skip, vec!["id-shop-db-1"]);
    }

    #[test]
    fn restarting_a_project_touches_everything() {
        let grouping = group_by_project(&mixed_project());
        let project = grouping.project("shop").expect("the project is there");
        let (act, skip) = plan(project, ProjectAction::Restart);
        assert_eq!(act.len(), 2);
        assert!(skip.is_empty());
    }

    #[test]
    fn a_fully_running_project_needs_no_start() {
        let containers = vec![detail("shop-web-1", "Up", "web", "1")];
        let grouping = group_by_project(&containers);
        let project = grouping.project("shop").expect("the project is there");
        let (act, skip) = plan(project, ProjectAction::Start);
        assert!(act.is_empty());
        assert_eq!(skip.len(), 1);
    }

    #[test]
    fn an_unknown_project_is_refused_by_name() {
        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client = DockerClient::connect_with(&choice).expect("http clients build lazily");
        let error = apply_blocking(&client, &mixed_project(), "no-such", ProjectAction::Stop)
            .expect_err("no container carries that label");
        assert!(matches!(error, DockerError::UnknownComposeProject(_)));
        assert!(error.to_string().contains("no-such"), "{error}");
        assert!(error.to_string().contains("compose up"), "{error}");
    }

    #[test]
    fn a_daemon_that_refuses_every_call_is_reported_per_container() {
        // Nothing listens on port 1, so every container fails and the outcome
        // says so rather than the whole call erroring out.
        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client = DockerClient::connect_with(&choice).expect("http clients build lazily");
        let outcome = apply_blocking(&client, &mixed_project(), "shop", ProjectAction::Restart)
            .expect("the project exists");
        assert!(!outcome.is_success());
        assert_eq!(outcome.failed.len(), 2);
        assert!(outcome.changed.is_empty());
        let line = outcome.summary("shop", ProjectAction::Restart);
        assert!(line.contains("2 failed"), "{line}");
    }

    #[test]
    fn containers_already_in_the_wanted_state_are_skipped_not_failed() {
        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client = DockerClient::connect_with(&choice).expect("http clients build lazily");
        let outcome = apply_blocking(&client, &mixed_project(), "shop", ProjectAction::Stop)
            .expect("the project exists");
        assert_eq!(outcome.skipped, vec!["id-shop-db-1"]);
        assert_eq!(
            outcome.failed.len(),
            1,
            "only the running one was attempted"
        );
    }

    #[test]
    fn a_clean_outcome_reads_as_success() {
        let outcome = ProjectOutcome {
            changed: vec!["a".to_string()],
            skipped: vec!["b".to_string()],
            failed: Vec::new(),
        };
        assert!(outcome.is_success());
        let line = outcome.summary("shop", ProjectAction::Start);
        assert!(line.contains("start shop"), "{line}");
        assert!(line.contains("1 changed"), "{line}");
        assert!(line.contains("1 already there"), "{line}");
        assert!(!line.contains("failed"), "{line}");
    }

    #[test]
    fn actions_have_verbs() {
        assert_eq!(ProjectAction::Start.as_str(), "start");
        assert_eq!(ProjectAction::Stop.as_str(), "stop");
        assert_eq!(ProjectAction::Restart.as_str(), "restart");
    }
}
