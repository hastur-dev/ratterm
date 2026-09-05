//! Grouping containers into Compose projects and services.
//!
//! Compose does not keep server-side state: a project is nothing more than a
//! set of containers carrying the same `com.docker.compose.project` label.
//! That means the grouping is a pure function over the container list, which
//! is how it is written here — no file on disk is read and no `docker compose`
//! binary is needed, so a project on a remote host groups exactly as one on
//! this machine does.

use std::collections::BTreeMap;

use super::container::DockerContainer;
use super::model::ContainerDetail;

/// Label Compose puts the project name in.
pub const PROJECT_LABEL: &str = "com.docker.compose.project";

/// Label Compose puts the service name in.
pub const SERVICE_LABEL: &str = "com.docker.compose.service";

/// Label naming the compose files the project was built from.
pub const CONFIG_FILES_LABEL: &str = "com.docker.compose.project.config_files";

/// Label naming the directory the project was brought up in.
pub const WORKING_DIR_LABEL: &str = "com.docker.compose.project.working_dir";

/// Label holding the replica number within a service.
pub const CONTAINER_NUMBER_LABEL: &str = "com.docker.compose.container-number";

/// How much of a project is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectState {
    /// Every container is running.
    Running,
    /// Some are running and some are not.
    Partial,
    /// Nothing is running.
    Stopped,
}

impl ProjectState {
    /// A word for the header line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Partial => "partial",
            Self::Stopped => "stopped",
        }
    }
}

/// One service within a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeService {
    /// Service name from the compose file.
    pub name: String,
    /// The containers backing it, replica order first.
    pub containers: Vec<DockerContainer>,
}

impl ComposeService {
    /// How many of this service's containers are running.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.containers.iter().filter(|c| c.is_running()).count()
    }
}

/// One Compose project on one host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeProject {
    /// Project name.
    pub name: String,
    /// Directory the project was brought up in, if the label was set.
    pub working_dir: Option<String>,
    /// The compose files it was built from.
    pub config_files: Vec<String>,
    /// Services, in name order.
    pub services: Vec<ComposeService>,
}

impl ComposeProject {
    /// Every container in the project.
    #[must_use]
    pub fn containers(&self) -> Vec<&DockerContainer> {
        self.services
            .iter()
            .flat_map(|s| s.containers.iter())
            .collect()
    }

    /// Every container id in the project, service order then replica order.
    #[must_use]
    pub fn container_ids(&self) -> Vec<String> {
        self.containers().iter().map(|c| c.id.clone()).collect()
    }

    /// How many containers the project has.
    #[must_use]
    pub fn container_count(&self) -> usize {
        self.services.iter().map(|s| s.containers.len()).sum()
    }

    /// How many are running.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.services
            .iter()
            .map(ComposeService::running_count)
            .sum()
    }

    /// Whether the project is up, down or in between.
    #[must_use]
    pub fn state(&self) -> ProjectState {
        let total = self.container_count();
        let running = self.running_count();
        if total == 0 || running == 0 {
            ProjectState::Stopped
        } else if running == total {
            ProjectState::Running
        } else {
            ProjectState::Partial
        }
    }

    /// A one-line header for the project.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} [{}] {}/{} running, {} service{}",
            self.name,
            self.state().as_str(),
            self.running_count(),
            self.container_count(),
            self.services.len(),
            if self.services.len() == 1 { "" } else { "s" }
        )
    }
}

/// Containers split into Compose projects and everything else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposeGrouping {
    /// Projects, in name order.
    pub projects: Vec<ComposeProject>,
    /// Containers with no project label, in the order they arrived.
    pub unmanaged: Vec<DockerContainer>,
}

impl ComposeGrouping {
    /// Looks a project up by name.
    #[must_use]
    pub fn project(&self, name: &str) -> Option<&ComposeProject> {
        self.projects.iter().find(|p| p.name == name)
    }

    /// True when no container on the host belongs to a project.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }
}

/// Reads the replica number a container was given, defaulting to 1.
///
/// Compose numbers replicas from 1; a container without the label sorts as if
/// it were the first, which keeps single-replica services in name order.
#[must_use]
fn replica_number(detail: &ContainerDetail) -> u32 {
    detail
        .label(CONTAINER_NUMBER_LABEL)
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

/// Splits the comma-separated config-files label.
#[must_use]
fn config_files(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Groups containers by their Compose labels.
///
/// A container with no `com.docker.compose.project` label — anything started
/// by hand, by another tool, or by the daemon itself — lands in `unmanaged`
/// rather than being dropped or invented a project for. A container that has
/// a project but no service name is filed under a service named for the
/// container, so it still appears.
#[must_use]
pub fn group_by_project(details: &[ContainerDetail]) -> ComposeGrouping {
    // (project, service) -> containers, with the replica number for ordering.
    let mut buckets: BTreeMap<String, BTreeMap<String, Vec<(u32, DockerContainer)>>> =
        BTreeMap::new();
    let mut metadata: BTreeMap<String, (Option<String>, Vec<String>)> = BTreeMap::new();
    let mut unmanaged = Vec::new();

    for detail in details {
        let Some(project) = detail
            .label(PROJECT_LABEL)
            .map(str::trim)
            .filter(|p| !p.is_empty())
        else {
            unmanaged.push(detail.container.clone());
            continue;
        };

        let service = detail
            .label(SERVICE_LABEL)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(detail.container.name.as_str())
            .to_string();

        let entry = metadata
            .entry(project.to_string())
            .or_insert_with(|| (None, Vec::new()));
        if entry.0.is_none() {
            entry.0 = detail
                .label(WORKING_DIR_LABEL)
                .filter(|d| !d.is_empty())
                .map(str::to_string);
        }
        if entry.1.is_empty() {
            entry.1 = config_files(detail.label(CONFIG_FILES_LABEL));
        }

        buckets
            .entry(project.to_string())
            .or_default()
            .entry(service)
            .or_default()
            .push((replica_number(detail), detail.container.clone()));
    }

    let projects = buckets
        .into_iter()
        .map(|(name, services)| {
            let (working_dir, config_files) = metadata.remove(&name).unwrap_or((None, Vec::new()));
            let services = services
                .into_iter()
                .map(|(service_name, mut containers)| {
                    containers.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
                    ComposeService {
                        name: service_name,
                        containers: containers.into_iter().map(|(_, c)| c).collect(),
                    }
                })
                .collect();
            ComposeProject {
                name,
                working_dir,
                config_files,
                services,
            }
        })
        .collect();

    ComposeGrouping {
        projects,
        unmanaged,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "compose_tests.rs"]
mod tests;
