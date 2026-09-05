//! Listing containers and images over the CLI transport.
//!
//! Split out of `discovery.rs`. The typed path in [`super::client`] returns the
//! same shapes without parsing text; this stays for the local CLI and for
//! hosts where the daemon socket cannot be forwarded.

use std::process::Command;

use super::cli::{
    COMMAND_TIMEOUT_MS, REMOTE_TIMEOUT_MS, docker_cmd, run_remote_with_timeout, run_with_timeout,
};
use super::container::{DockerContainer, DockerImage};
use super::discovery::{DockerDiscovery, DockerDiscoveryResult};
use super::host::DockerHost;
use super::parse::{parse_containers_json, parse_images_json};

/// Turns a typed snapshot into the result the existing Docker screens render.
///
/// The typed client returns richer data than `docker ps` does; this narrows it
/// to the shape the list widgets already know, so the typed path can be used
/// without rewriting them. Pure, so the narrowing is testable without a daemon.
#[must_use]
pub fn discovery_from_snapshot(snapshot: &super::client::HostSnapshot) -> DockerDiscoveryResult {
    let containers = snapshot
        .containers
        .iter()
        .map(|detail| detail.container.clone())
        .collect();
    let (running, stopped) = partition_by_state(containers);

    DockerDiscoveryResult {
        running_containers: running,
        stopped_containers: stopped,
        images: snapshot.images.clone(),
        ..DockerDiscoveryResult::available()
    }
}

/// Splits a mixed container list into running and stopped.
///
/// Pure, so the partition rule is testable without a daemon.
#[must_use]
pub fn partition_by_state(
    containers: Vec<DockerContainer>,
) -> (Vec<DockerContainer>, Vec<DockerContainer>) {
    let mut running = Vec::new();
    let mut stopped = Vec::new();
    for container in containers {
        if container.status.is_running() {
            running.push(container);
        } else {
            stopped.push(container);
        }
    }
    (running, stopped)
}

impl DockerDiscovery {
    /// Performs discovery on a specific host (local or remote).
    #[must_use]
    pub fn discover_all_for_host(host: &DockerHost) -> DockerDiscoveryResult {
        let availability = Self::check_availability_for_host(host);
        let where_ = host.display_name();
        if let Some(result) = DockerDiscoveryResult::from_unavailable(&availability, &where_) {
            return result;
        }

        let mut result = DockerDiscoveryResult::available();

        match Self::containers_for_host(host) {
            Ok((running, stopped)) => {
                result.running_containers = running;
                result.stopped_containers = stopped;
            }
            Err(e) => result.add_error(format!("Container discovery failed: {e}")),
        }

        match Self::images_for_host(host) {
            Ok(images) => result.images = images,
            Err(e) => result.add_error(format!("Image discovery failed: {e}")),
        }

        result
    }

    /// Performs full discovery on the local daemon.
    #[must_use]
    pub fn discover_all() -> DockerDiscoveryResult {
        Self::discover_all_for_host(&DockerHost::Local)
    }

    /// Performs full discovery on a remote host via SSH.
    ///
    /// # Panics
    /// Panics if `host` is not remote.
    #[must_use]
    pub fn discover_all_remote(host: &DockerHost) -> DockerDiscoveryResult {
        assert!(host.is_remote(), "host must be remote");
        Self::discover_all_for_host(host)
    }

    /// Lists containers on any host, split into running and stopped.
    pub fn containers_for_host(
        host: &DockerHost,
    ) -> Result<(Vec<DockerContainer>, Vec<DockerContainer>), String> {
        let args = ["ps", "-a", "--format", "{{json .}}"];
        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(docker_cmd());
                cmd.args(args);
                run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => run_remote_with_timeout(host, &args, REMOTE_TIMEOUT_MS),
        }
        .ok_or_else(|| {
            "docker ps did not answer in time; check the daemon is running".to_string()
        })?;

        if !output.status.success() {
            return Err(format!(
                "docker ps -a failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(partition_by_state(parse_containers_json(&stdout, false)))
    }

    /// Lists images on any host.
    pub fn images_for_host(host: &DockerHost) -> Result<Vec<DockerImage>, String> {
        let args = ["images", "--format", "{{json .}}"];
        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(docker_cmd());
                cmd.args(args);
                run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => run_remote_with_timeout(host, &args, REMOTE_TIMEOUT_MS),
        }
        .ok_or_else(|| {
            "docker images did not answer in time; check the daemon is running".to_string()
        })?;

        if !output.status.success() {
            return Err(format!(
                "docker images failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(parse_images_json(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Discovers all containers on a remote host.
    ///
    /// # Panics
    /// Panics if `host` is not remote.
    pub fn discover_all_containers_remote(
        host: &DockerHost,
    ) -> Result<(Vec<DockerContainer>, Vec<DockerContainer>), String> {
        assert!(host.is_remote(), "host must be remote");
        Self::containers_for_host(host)
    }

    /// Discovers all images on a remote host.
    ///
    /// # Panics
    /// Panics if `host` is not remote.
    pub fn discover_images_remote(host: &DockerHost) -> Result<Vec<DockerImage>, String> {
        assert!(host.is_remote(), "host must be remote");
        Self::images_for_host(host)
    }

    /// Discovers all containers on the local daemon.
    pub fn discover_all_containers() -> Result<(Vec<DockerContainer>, Vec<DockerContainer>), String>
    {
        Self::containers_for_host(&DockerHost::Local)
    }

    /// Discovers the running containers on the local daemon.
    pub fn discover_running_containers() -> Result<Vec<DockerContainer>, String> {
        Ok(Self::containers_for_host(&DockerHost::Local)?.0)
    }

    /// Discovers all images on the local daemon.
    pub fn discover_images() -> Result<Vec<DockerImage>, String> {
        Self::images_for_host(&DockerHost::Local)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::discovery::DockerAvailability;
    use super::*;

    fn container(id: &str, status: &str) -> DockerContainer {
        DockerContainer::new(
            id.to_string(),
            id.to_string(),
            "img".to_string(),
            status.to_string(),
        )
    }

    #[test]
    fn an_empty_list_partitions_into_two_empty_lists() {
        let (running, stopped) = partition_by_state(Vec::new());
        assert!(running.is_empty());
        assert!(stopped.is_empty());
    }

    #[test]
    fn one_running_container_lands_on_the_running_side() {
        let (running, stopped) = partition_by_state(vec![container("a", "Up 3 hours")]);
        assert_eq!(running.len(), 1);
        assert!(stopped.is_empty());
    }

    #[test]
    fn a_mixed_list_is_split_and_keeps_its_order() {
        let (running, stopped) = partition_by_state(vec![
            container("a", "Up"),
            container("b", "Exited (0) 1 hour ago"),
            container("c", "Up"),
            container("d", "Created"),
        ]);
        assert_eq!(
            running.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        assert_eq!(
            stopped.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "d"]
        );
    }

    #[test]
    fn discovery_against_an_unreachable_host_reports_why_and_lists_nothing() {
        let host = DockerHost::remote_labelled(912_345, "ghost-box");
        let result = DockerDiscovery::discover_all_for_host(&host);
        assert!(!result.docker_available);
        assert!(!result.has_items());
        assert!(result.error.is_some());
    }

    #[test]
    fn listing_an_unreachable_host_is_an_error_rather_than_an_empty_list() {
        let host = DockerHost::remote(912_346);
        assert!(DockerDiscovery::containers_for_host(&host).is_err());
        assert!(DockerDiscovery::images_for_host(&host).is_err());
    }

    #[test]
    #[should_panic(expected = "host must be remote")]
    fn the_remote_entry_points_refuse_the_local_host() {
        let _ = DockerDiscovery::discover_all_containers_remote(&DockerHost::Local);
    }

    #[test]
    fn an_empty_snapshot_narrows_to_an_available_result_with_nothing_in_it() {
        use super::super::client::HostSnapshot;
        let result = discovery_from_snapshot(&HostSnapshot::default());
        assert!(result.docker_available);
        assert!(!result.has_items());
        assert!(result.error.is_none());
    }

    #[test]
    fn a_snapshot_narrows_to_running_stopped_and_images() {
        use super::super::client::HostSnapshot;
        use super::super::container::DockerImage;
        use super::super::model::ContainerDetail;
        use std::collections::HashMap;

        let detail = |name: &str, status: &str| ContainerDetail {
            container: container(name, status),
            labels: HashMap::new(),
            state: status.to_lowercase(),
            command: String::new(),
        };

        let snapshot = HostSnapshot {
            containers: vec![
                detail("a", "Up 1 hour"),
                detail("b", "Exited (0) 2 hours ago"),
            ],
            images: vec![DockerImage::new(
                "sha256:x".to_string(),
                "nginx".to_string(),
                "latest".to_string(),
            )],
            ..Default::default()
        };

        let result = discovery_from_snapshot(&snapshot);
        assert_eq!(result.running_containers.len(), 1);
        assert_eq!(result.stopped_containers.len(), 1);
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.total_count(), 3);
    }

    #[test]
    fn availability_is_reported_before_any_listing_is_attempted() {
        // A host that cannot be reached must not be reported as "0 containers".
        let host = DockerHost::remote(912_347);
        let result = DockerDiscovery::discover_all_for_host(&host);
        assert_ne!(result.availability, DockerAvailability::Available);
    }
}
