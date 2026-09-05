//! One flat list of every container on every connected host.
//!
//! "What is running on my fleet" used to take one screen per host. This builds
//! the single list the fleet view renders. Everything here is a pure function
//! over rows, so the sort order and the filter can be tested exactly, with no
//! host, daemon or terminal involved.

use super::compose::{PROJECT_LABEL, SERVICE_LABEL};
use super::container::{DockerContainer, DockerStatus};
use super::model::ContainerDetail;

/// One container, on one host, as the fleet list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    /// `None` for the local daemon, `Some(id)` for an SSH host.
    pub host_key: Option<u32>,
    /// The host's display name.
    pub host_label: String,
    /// The container itself.
    pub container: DockerContainer,
    /// Compose project, when the container belongs to one.
    pub project: Option<String>,
    /// Compose service, when the container belongs to one.
    pub service: Option<String>,
}

impl FleetRow {
    /// Builds the rows for one host.
    #[must_use]
    pub fn for_host(
        host_key: Option<u32>,
        host_label: &str,
        containers: &[ContainerDetail],
    ) -> Vec<Self> {
        containers
            .iter()
            .map(|detail| Self {
                host_key,
                host_label: host_label.to_string(),
                container: detail.container.clone(),
                project: detail
                    .label(PROJECT_LABEL)
                    .filter(|p| !p.is_empty())
                    .map(str::to_string),
                service: detail
                    .label(SERVICE_LABEL)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            })
            .collect()
    }

    /// `project/service`, or nothing when the container is not Compose-managed.
    #[must_use]
    pub fn compose_path(&self) -> Option<String> {
        match (self.project.as_deref(), self.service.as_deref()) {
            (Some(project), Some(service)) => Some(format!("{project}/{service}")),
            (Some(project), None) => Some(project.to_string()),
            _ => None,
        }
    }

    /// The text a filter query is matched against.
    #[must_use]
    pub fn search_text(&self) -> String {
        let mut text = format!(
            "{} {} {} {} {}",
            self.host_label,
            self.container.name,
            self.container.image,
            self.container.status.as_str(),
            self.container.id
        );
        if let Some(path) = self.compose_path() {
            text.push(' ');
            text.push_str(&path);
        }
        text.to_lowercase()
    }

    /// One line for the list.
    #[must_use]
    pub fn line(&self) -> String {
        let marker = if self.container.is_running() {
            "*"
        } else {
            " "
        };
        let compose = self
            .compose_path()
            .map(|p| format!(" [{p}]"))
            .unwrap_or_default();
        format!(
            "{marker} {}  {}  {}{compose}  {}",
            self.host_label,
            self.container.display(),
            self.container.image,
            self.container.status.as_str()
        )
    }
}

/// How the fleet list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FleetSort {
    /// Host first, then container name. The default: it keeps a host's
    /// containers together, which is how people think about a fleet.
    #[default]
    Host,
    /// Container name, across every host.
    Name,
    /// Image, then container name.
    Image,
    /// Running first, then host, then name.
    Status,
}

impl FleetSort {
    /// A label for the header.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Name => "name",
            Self::Image => "image",
            Self::Status => "status",
        }
    }

    /// The next order, for a key that cycles through them.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Host => Self::Name,
            Self::Name => Self::Image,
            Self::Image => Self::Status,
            Self::Status => Self::Host,
        }
    }
}

/// Orders a running container ahead of a stopped one.
const fn status_rank(status: DockerStatus) -> u8 {
    match status {
        DockerStatus::Running => 0,
        DockerStatus::Restarting | DockerStatus::Paused | DockerStatus::Created => 1,
        _ => 2,
    }
}

/// Sorts rows in place.
///
/// Every order ends in host then name, so the result is total: two runs over
/// the same rows produce the same list whatever order they arrived in.
pub fn sort_rows(rows: &mut [FleetRow], sort: FleetSort) {
    rows.sort_by(|a, b| {
        let host = a.host_key.cmp(&b.host_key);
        let name = a.container.name.cmp(&b.container.name);
        match sort {
            FleetSort::Host => host.then(name),
            FleetSort::Name => name.then(host),
            FleetSort::Image => a
                .container
                .image
                .cmp(&b.container.image)
                .then(host)
                .then(name),
            FleetSort::Status => status_rank(a.container.status)
                .cmp(&status_rank(b.container.status))
                .then(host)
                .then(name),
        }
    });
}

/// Keeps the rows matching every whitespace-separated term in `query`.
///
/// An empty query keeps everything. Terms are ANDed and matched
/// case-insensitively against the host, name, image, status, id and Compose
/// path, so `shop web` finds the web service of the shop project wherever it
/// runs.
#[must_use]
pub fn filter_rows(rows: &[FleetRow], query: &str) -> Vec<FleetRow> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return rows.to_vec();
    }

    rows.iter()
        .filter(|row| {
            let text = row.search_text();
            terms.iter().all(|term| text.contains(term))
        })
        .cloned()
        .collect()
}

/// Aggregate numbers for the fleet header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FleetCounts {
    /// Hosts in the fleet, connected or not.
    pub hosts: usize,
    /// Hosts whose daemon answered.
    pub connected: usize,
    /// Hosts whose connection failed.
    pub failed: usize,
    /// Containers on connected hosts.
    pub containers: usize,
    /// Of those, how many are running.
    pub running: usize,
}

impl FleetCounts {
    /// The header line: "6 hosts, 4 connected, 23 containers (17 running)".
    #[must_use]
    pub fn headline(&self) -> String {
        let mut line = format!(
            "{} host{}, {} connected",
            self.hosts,
            if self.hosts == 1 { "" } else { "s" },
            self.connected
        );
        if self.failed > 0 {
            line.push_str(&format!(", {} unreachable", self.failed));
        }
        line.push_str(&format!(
            ", {} container{} ({} running)",
            self.containers,
            if self.containers == 1 { "" } else { "s" },
            self.running
        ));
        line
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "fleet_rows_tests.rs"]
mod tests;
