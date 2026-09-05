//! Docker API layer for programmatic access and testing.
//!
//! Provides a simple API for remote Docker management that can be used
//! for testing, extensions, and CLI tools.
//!
//! # Host Management API
//!
//! The `DockerHostManager` provides methods to directly set and test Docker hosts,
//! bypassing the UI for testing and automation purposes.

use super::container::{DockerContainer, DockerImage};
use super::discovery::{DockerDiscovery, DockerDiscoveryResult};
use super::host::DockerHost;
use super::items::DockerItemList;
use crate::remote::{ExecError, exec_on_host};

/// Host manager for programmatic Docker host manipulation.
///
/// This allows setting the Docker host directly without going through the UI,
/// useful for testing, automation, and debugging.
pub struct DockerHostManager<'a> {
    items: &'a mut DockerItemList,
}

impl<'a> DockerHostManager<'a> {
    /// Creates a new host manager for the given item list.
    #[must_use]
    pub fn new(items: &'a mut DockerItemList) -> Self {
        Self { items }
    }

    /// Gets the currently selected host.
    #[must_use]
    pub fn current_host(&self) -> &DockerHost {
        &self.items.selected_host
    }

    /// Sets the host to local Docker.
    pub fn set_local(&mut self) {
        self.items.set_selected_host(DockerHost::Local);
    }

    /// Sets the host to a remote Docker daemon reached through an SSH host.
    ///
    /// Only the host id is stored. Address, user and secret are resolved from
    /// the host registry when a command is run, so this cannot go stale.
    pub fn set_remote(&mut self, host_id: u32) {
        self.items.set_selected_host(DockerHost::remote(host_id));
    }

    /// Sets the host to a remote Docker daemon, with a label for display.
    pub fn set_remote_labelled(&mut self, host_id: u32, label: &str) {
        self.items
            .set_selected_host(DockerHost::remote_labelled(host_id, label));
    }

    /// Tests the current host configuration by attempting discovery.
    ///
    /// Returns diagnostic information about the host.
    pub fn test_current_host(&self) -> Vec<String> {
        let mut results = Vec::new();
        let host = &self.items.selected_host;

        results.push("=== Testing Current Docker Host ===".to_string());
        results.push(format!(
            "Host Type: {}",
            if host.is_local() { "Local" } else { "Remote" }
        ));
        results.push(format!("Display Name: {}", host.display_name()));

        match host {
            DockerHost::Local => {
                results.push("Storage Key: local".to_string());
            }
            DockerHost::Remote { host_id, .. } => {
                results.push(format!("Host ID: {}", host_id));
                results.push(format!("Storage Key: {}", host.storage_key()));
                results.push(format!(
                    "Resolvable: {}",
                    crate::remote::with_shared(|executor| executor.target(*host_id).is_some())
                ));
            }
        }

        results.push("\n--- Discovery Test ---".to_string());
        let discovery_result = DockerDiscovery::discover_all_for_host(host);

        results.push(format!(
            "Docker Available: {}",
            discovery_result.docker_available
        ));
        results.push(format!("Availability: {:?}", discovery_result.availability));
        results.push(format!(
            "Running Containers: {}",
            discovery_result.running_containers.len()
        ));
        results.push(format!(
            "Stopped Containers: {}",
            discovery_result.stopped_containers.len()
        ));
        results.push(format!("Images: {}", discovery_result.images.len()));

        if let Some(err) = discovery_result.error {
            results.push(format!("Error: {}", err));
        }

        results.push("=== Test Complete ===".to_string());
        results
    }

    /// Returns detailed debug info about the host configuration.
    #[must_use]
    pub fn debug_info(&self) -> String {
        let host = &self.items.selected_host;
        match host {
            DockerHost::Local => "DockerHost::Local".to_string(),
            DockerHost::Remote {
                host_id,
                cached_label,
            } => {
                format!("DockerHost::Remote {{ host_id: {host_id}, label: {cached_label:?} }}")
            }
        }
    }
}

/// Docker operations addressed by SSH host id.
///
/// The previous version of this API took `hostname`, `port` and `username` and
/// built an `ssh` (or `sshpass`, or `plink`) command line for each call. It now
/// runs everything through the pooled SSH sessions, so a call costs a channel
/// rather than a process and an authentication handshake, and no secret is ever
/// placed on a command line.
pub struct DockerApi;

impl DockerApi {
    /// Checks that the host answers over SSH.
    ///
    /// # Errors
    /// Returns a message describing why the host could not be reached.
    pub fn test_connection(host_id: u32) -> Result<String, String> {
        match exec_on_host(host_id, "echo ratterm-ssh-ok") {
            Ok(output) if output.trimmed() == "ratterm-ssh-ok" => {
                Ok(format!("SSH to host {host_id} works"))
            }
            Ok(output) => Err(format!(
                "SSH to host {host_id} answered unexpectedly: {:?}",
                output.trimmed()
            )),
            Err(e) => Err(describe(host_id, &e)),
        }
    }

    /// Returns the Docker version reported by a host.
    ///
    /// # Errors
    /// Returns a message if the host is unreachable or has no Docker CLI.
    pub fn docker_version(host_id: u32) -> Result<String, String> {
        let output =
            exec_on_host(host_id, "docker --version").map_err(|e| describe(host_id, &e))?;
        if output.success() {
            Ok(output.trimmed().to_string())
        } else {
            Err(format!(
                "Docker is not usable on host {host_id}: {}",
                output.stderr.trim()
            ))
        }
    }

    /// Lists every container on a host, running or not.
    ///
    /// # Errors
    /// Returns a message if discovery fails.
    pub fn list_containers(host_id: u32) -> Result<Vec<DockerContainer>, String> {
        let result = DockerDiscovery::discover_all_for_host(&DockerHost::remote(host_id));
        if !result.docker_available {
            return Err(result
                .error
                .unwrap_or_else(|| "Docker not available".to_string()));
        }
        let mut all = result.running_containers;
        all.extend(result.stopped_containers);
        Ok(all)
    }

    /// Lists the images on a host.
    ///
    /// # Errors
    /// Returns a message if discovery fails.
    pub fn list_images(host_id: u32) -> Result<Vec<DockerImage>, String> {
        let result = DockerDiscovery::discover_all_for_host(&DockerHost::remote(host_id));
        if !result.docker_available {
            return Err(result
                .error
                .unwrap_or_else(|| "Docker not available".to_string()));
        }
        Ok(result.images)
    }

    /// Runs full discovery against a host.
    #[must_use]
    pub fn discover(host_id: u32) -> DockerDiscoveryResult {
        DockerDiscovery::discover_all_for_host(&DockerHost::remote(host_id))
    }

    /// Runs a raw Docker command on a host.
    ///
    /// Returns standard output, standard error and the exit status.
    ///
    /// # Errors
    /// Returns a message if the host cannot be reached.
    pub fn exec(host_id: u32, docker_args: &str) -> Result<(String, String, i32), String> {
        let command = format!("docker {docker_args}");
        let output = exec_on_host(host_id, &command).map_err(|e| describe(host_id, &e))?;
        Ok((output.stdout, output.stderr, output.exit_status))
    }

    /// Returns a step-by-step report of what works and what does not.
    ///
    /// Written for a user who has just been told "no containers found" and
    /// needs to know which of the possible reasons applies.
    #[must_use]
    pub fn diagnose(host_id: u32) -> Vec<String> {
        let mut results = vec![format!("=== Diagnosing Docker on SSH host {host_id} ===")];

        results.push("[1] SSH connectivity".to_string());
        match Self::test_connection(host_id) {
            Ok(msg) => results.push(format!("    ok: {msg}")),
            Err(msg) => {
                results.push(format!("    failed: {msg}"));
                results.push("    nothing else can be checked without SSH".to_string());
                return results;
            }
        }

        results.push("[2] Docker CLI".to_string());
        match Self::docker_version(host_id) {
            Ok(version) => results.push(format!("    ok: {version}")),
            Err(msg) => {
                results.push(format!("    failed: {msg}"));
                results.push("    Docker may not be installed or not on PATH".to_string());
                return results;
            }
        }

        results.push("[3] Containers".to_string());
        match Self::list_containers(host_id) {
            Ok(containers) => {
                results.push(format!("    ok: {} containers", containers.len()));
                for c in containers.iter().take(5) {
                    let status = if c.is_running() { "running" } else { "stopped" };
                    results.push(format!("      {} ({}) [{}]", c.name, c.image, status));
                }
                if containers.len() > 5 {
                    results.push(format!("      and {} more", containers.len() - 5));
                }
            }
            Err(msg) => results.push(format!("    failed: {msg}")),
        }

        results.push("[4] Images".to_string());
        match Self::list_images(host_id) {
            Ok(images) => {
                results.push(format!("    ok: {} images", images.len()));
                for img in images.iter().take(5) {
                    results.push(format!("      {}:{}", img.repository, img.tag));
                }
                if images.len() > 5 {
                    results.push(format!("      and {} more", images.len() - 5));
                }
            }
            Err(msg) => results.push(format!("    failed: {msg}")),
        }

        results.push("=== Diagnosis complete ===".to_string());
        results
    }
}

/// Turns an executor error into something a user can act on.
fn describe(host_id: u32, error: &ExecError) -> String {
    match error {
        ExecError::UnknownHost(_) => {
            format!("SSH host {host_id} is not in the host list, or has no username set")
        }
        ExecError::Session(e) => e.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_host_is_identified_only_by_its_id() {
        let mut items = DockerItemList::new();
        let mut manager = DockerHostManager::new(&mut items);
        manager.set_remote(7);
        assert_eq!(manager.current_host().host_id(), Some(7));
        assert_eq!(manager.current_host().storage_key(), "remote:7");
    }

    #[test]
    fn a_labelled_remote_host_keeps_its_label() {
        let mut items = DockerItemList::new();
        let mut manager = DockerHostManager::new(&mut items);
        manager.set_remote_labelled(8, "Desk Rock5c");
        assert_eq!(manager.current_host().display_name(), "Desk Rock5c");
    }

    #[test]
    fn switching_back_to_local_clears_the_host_id() {
        let mut items = DockerItemList::new();
        let mut manager = DockerHostManager::new(&mut items);
        manager.set_remote(3);
        manager.set_local();
        assert!(manager.current_host().is_local());
        assert_eq!(manager.current_host().host_id(), None);
    }

    #[test]
    fn debug_info_names_the_variant() {
        let mut items = DockerItemList::new();
        let mut manager = DockerHostManager::new(&mut items);
        assert!(manager.debug_info().contains("Local"));
        manager.set_remote_labelled(4, "label");
        assert!(manager.debug_info().contains("host_id: 4"));
        assert!(manager.debug_info().contains("label"));
    }

    #[test]
    fn an_unknown_host_is_reported_in_plain_language() {
        let message = describe(99, &ExecError::UnknownHost(99));
        assert!(message.contains("99"), "{message}");
        assert!(message.contains("host list"), "{message}");
    }

    #[test]
    fn calls_against_an_unregistered_host_fail_rather_than_hang() {
        // Nothing has published a target for this id, so every entry point
        // must report that rather than trying to connect somewhere.
        let unregistered = 987_654;
        assert!(DockerApi::test_connection(unregistered).is_err());
        assert!(DockerApi::docker_version(unregistered).is_err());
        assert!(DockerApi::exec(unregistered, "ps").is_err());
    }

    #[test]
    fn diagnose_stops_at_the_first_failure() {
        let report = DockerApi::diagnose(987_655);
        assert!(report[0].contains("987655"), "{:?}", report[0]);
        assert!(
            report
                .iter()
                .any(|line| line.contains("nothing else can be checked")),
            "{report:?}"
        );
        assert!(
            !report.iter().any(|line| line.contains("[3] Containers")),
            "must not continue past a failed SSH check: {report:?}"
        );
    }
}
