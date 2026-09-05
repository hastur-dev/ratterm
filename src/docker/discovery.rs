//! Whether Docker is usable on a host, and what discovery found.
//!
//! The `DockerDiscovery` entry point is spread over four files that each stay
//! inside the project's size limit: availability here, the actual listing in
//! [`super::scan`], lifecycle calls in [`super::ops`], and Docker Hub in
//! [`super::images`]. They share one type so every existing call site keeps
//! working.

use super::cli::{
    COMMAND_TIMEOUT_MS, QUICK_TIMEOUT_MS, REMOTE_QUICK_TIMEOUT_MS, REMOTE_TIMEOUT_MS, docker_cmd,
    run_remote_with_timeout, run_with_timeout,
};
use super::container::{DockerContainer, DockerImage};
use super::host::DockerHost;
use std::process::Command;

/// Longest daemon error text kept for display.
const MAX_ERROR_CHARS: usize = 200;

/// Docker availability status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DockerAvailability {
    /// Docker status not yet checked.
    #[default]
    Unknown,
    /// Docker CLI is not installed on the system.
    NotInstalled,
    /// Docker is installed but the daemon is not running.
    NotRunning,
    /// Docker daemon returned an error (API issue, etc.).
    DaemonError(String),
    /// Docker is available and running.
    Available,
}

impl DockerAvailability {
    /// Returns true if Docker is available and running.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Returns true if Docker is installed (but may not be running).
    #[must_use]
    pub fn is_installed(&self) -> bool {
        matches!(
            self,
            Self::Available | Self::NotRunning | Self::DaemonError(_)
        )
    }

    /// Returns the error message if there's a daemon error.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::DaemonError(msg) => Some(msg.as_str()),
            _ => None,
        }
    }
}

/// Classifies the standard error of a failed `docker ps`.
///
/// Pure, so every branch is testable without a daemon in any particular state.
#[must_use]
pub fn classify_daemon_error(stderr: &str) -> DockerAvailability {
    let trimmed = stderr.trim();

    if trimmed.contains("Cannot connect to the Docker daemon")
        || trimmed.contains("Is the docker daemon running")
        || trimmed.contains("docker daemon is not running")
    {
        return DockerAvailability::NotRunning;
    }

    if trimmed.is_empty() {
        return DockerAvailability::NotRunning;
    }

    let truncated: String = trimmed.chars().take(MAX_ERROR_CHARS).collect();
    if truncated.len() < trimmed.len() {
        DockerAvailability::DaemonError(format!("{truncated}..."))
    } else {
        DockerAvailability::DaemonError(truncated)
    }
}

/// Result of Docker discovery operation.
#[derive(Debug, Clone, Default)]
pub struct DockerDiscoveryResult {
    /// Running containers.
    pub running_containers: Vec<DockerContainer>,
    /// Stopped containers.
    pub stopped_containers: Vec<DockerContainer>,
    /// Available images.
    pub images: Vec<DockerImage>,
    /// Whether Docker CLI is available.
    pub docker_available: bool,
    /// Docker availability status (more detailed).
    pub availability: DockerAvailability,
    /// Error message if discovery failed.
    pub error: Option<String>,
}

impl DockerDiscoveryResult {
    /// Creates a result indicating Docker is not installed.
    #[must_use]
    pub fn not_installed() -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::NotInstalled,
            error: Some(
                "Docker is not installed on this system. Install Docker, then refresh.".to_string(),
            ),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker is not running.
    #[must_use]
    pub fn not_running() -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::NotRunning,
            error: Some(
                "Docker is installed but not running. Start the daemon, then refresh.".to_string(),
            ),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker daemon has an error.
    #[must_use]
    pub fn daemon_error(error: String) -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::DaemonError(error.clone()),
            error: Some(error),
            ..Default::default()
        }
    }

    /// Creates a result indicating Docker is not available.
    #[must_use]
    pub fn not_available(error: String) -> Self {
        Self {
            docker_available: false,
            availability: DockerAvailability::Unknown,
            error: Some(error),
            ..Default::default()
        }
    }

    /// Creates a result for a host that answered.
    #[must_use]
    pub fn available() -> Self {
        Self {
            docker_available: true,
            availability: DockerAvailability::Available,
            ..Default::default()
        }
    }

    /// Appends an error, keeping any already recorded.
    pub fn add_error(&mut self, message: String) {
        match self.error.as_mut() {
            Some(existing) => {
                existing.push_str("; ");
                existing.push_str(&message);
            }
            None => self.error = Some(message),
        }
    }

    /// Returns total count of all items.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.running_containers.len() + self.stopped_containers.len() + self.images.len()
    }

    /// Returns true if any containers or images were found.
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.running_containers.is_empty()
            || !self.stopped_containers.is_empty()
            || !self.images.is_empty()
    }

    /// Turns an availability that is not `Available` into the matching result.
    ///
    /// Returns `None` when Docker is usable and discovery should continue.
    #[must_use]
    pub fn from_unavailable(availability: &DockerAvailability, where_: &str) -> Option<Self> {
        match availability {
            DockerAvailability::NotInstalled => Some(Self::not_installed()),
            DockerAvailability::NotRunning => Some(Self::not_running()),
            DockerAvailability::DaemonError(msg) => Some(Self::daemon_error(msg.clone())),
            DockerAvailability::Unknown => Some(Self::not_available(format!(
                "Unable to determine Docker status on {where_}. Check the host is reachable."
            ))),
            DockerAvailability::Available => None,
        }
    }
}

/// Docker discovery service.
pub struct DockerDiscovery;

impl DockerDiscovery {
    /// Checks if Docker is available on a remote host via SSH.
    ///
    /// # Panics
    /// Panics if `host` is not remote.
    #[must_use]
    pub fn is_docker_available_remote(host: &DockerHost) -> bool {
        assert!(host.is_remote(), "host must be remote");

        run_remote_with_timeout(
            host,
            &["version", "--format", "{{.Server.Version}}"],
            REMOTE_QUICK_TIMEOUT_MS,
        )
        .map(|o| o.status.success())
        .unwrap_or(false)
    }

    /// Checks if the local Docker CLI and daemon answer.
    #[must_use]
    pub fn is_docker_available() -> bool {
        let mut cmd = Command::new(docker_cmd());
        cmd.args(["version", "--format", "{{.Server.Version}}"]);

        run_with_timeout(&mut cmd, QUICK_TIMEOUT_MS)
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Checks Docker availability on a specific host.
    #[must_use]
    pub fn check_availability_for_host(host: &DockerHost) -> DockerAvailability {
        match host {
            DockerHost::Local => Self::check_availability(),
            DockerHost::Remote { .. } => Self::check_availability_remote(host),
        }
    }

    /// Checks Docker availability on the local machine, with detail.
    #[must_use]
    pub fn check_availability() -> DockerAvailability {
        let mut cmd = Command::new(docker_cmd());
        cmd.arg("--version");

        let cli_exists = run_with_timeout(&mut cmd, QUICK_TIMEOUT_MS)
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !cli_exists {
            return DockerAvailability::NotInstalled;
        }

        // `docker ps` is much faster than `docker info` and answers the same
        // question: can we talk to the daemon.
        let mut cmd = Command::new(docker_cmd());
        cmd.args(["ps", "-q", "--no-trunc"]);

        match run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS) {
            Some(output) if output.status.success() => DockerAvailability::Available,
            Some(output) => classify_daemon_error(&String::from_utf8_lossy(&output.stderr)),
            None => DockerAvailability::DaemonError(
                "the docker command timed out; the daemon is not responding".to_string(),
            ),
        }
    }

    /// Checks Docker availability on a remote host via SSH.
    ///
    /// # Panics
    /// Panics if `host` is not remote.
    #[must_use]
    pub fn check_availability_remote(host: &DockerHost) -> DockerAvailability {
        assert!(host.is_remote(), "host must be remote");

        let output = run_remote_with_timeout(host, &["--version"], REMOTE_QUICK_TIMEOUT_MS);
        let cli_exists = output.as_ref().map(|o| o.status.success()).unwrap_or(false);

        if !cli_exists {
            let err_msg = output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_else(|| {
                    "the SSH connection failed or timed out; check the host in the SSH manager"
                        .to_string()
                });

            if err_msg.contains("not found") || err_msg.contains("command not found") {
                return DockerAvailability::NotInstalled;
            }
            return DockerAvailability::DaemonError(err_msg);
        }

        match run_remote_with_timeout(host, &["ps", "-q", "--no-trunc"], REMOTE_TIMEOUT_MS) {
            Some(o) if o.status.success() => DockerAvailability::Available,
            Some(o) => classify_daemon_error(&String::from_utf8_lossy(&o.stderr)),
            None => DockerAvailability::DaemonError(
                "the remote docker command timed out; check the host's load".to_string(),
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_result_not_available() {
        let result = DockerDiscoveryResult::not_available("Docker not found".to_string());
        assert!(!result.docker_available);
        assert!(result.error.is_some());
        assert!(!result.has_items());
        assert_eq!(result.total_count(), 0);
    }

    #[test]
    fn availability_classifies_its_own_state() {
        assert!(DockerAvailability::Available.is_available());
        assert!(DockerAvailability::Available.is_installed());
        assert!(DockerAvailability::NotRunning.is_installed());
        assert!(!DockerAvailability::NotInstalled.is_installed());
        assert!(!DockerAvailability::Unknown.is_installed());
        assert_eq!(
            DockerAvailability::DaemonError("boom".to_string()).error_message(),
            Some("boom")
        );
        assert_eq!(DockerAvailability::Available.error_message(), None);
    }

    #[test]
    fn a_refused_connection_reads_as_not_running() {
        let stderr = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock.";
        assert_eq!(
            classify_daemon_error(stderr),
            DockerAvailability::NotRunning
        );
        assert_eq!(
            classify_daemon_error("Is the docker daemon running?"),
            DockerAvailability::NotRunning
        );
    }

    #[test]
    fn silence_reads_as_not_running_rather_than_an_empty_error() {
        assert_eq!(classify_daemon_error("   "), DockerAvailability::NotRunning);
    }

    #[test]
    fn an_unrecognised_error_is_kept_and_truncated() {
        let long = "x".repeat(MAX_ERROR_CHARS + 50);
        match classify_daemon_error(&long) {
            DockerAvailability::DaemonError(msg) => {
                assert!(msg.ends_with("..."), "{msg}");
                assert_eq!(msg.chars().count(), MAX_ERROR_CHARS + 3);
            }
            other => panic!("expected a daemon error, got {other:?}"),
        }
    }

    #[test]
    fn a_short_error_is_kept_whole() {
        match classify_daemon_error("  permission denied  ") {
            DockerAvailability::DaemonError(msg) => assert_eq!(msg, "permission denied"),
            other => panic!("expected a daemon error, got {other:?}"),
        }
    }

    #[test]
    fn an_error_that_ends_in_a_multibyte_character_is_truncated_safely() {
        // Slicing by bytes here would panic; the classifier takes characters.
        let long = "é".repeat(MAX_ERROR_CHARS + 10);
        assert!(matches!(
            classify_daemon_error(&long),
            DockerAvailability::DaemonError(_)
        ));
    }

    #[test]
    fn every_unavailable_state_maps_to_a_result_and_available_maps_to_none() {
        assert!(
            DockerDiscoveryResult::from_unavailable(&DockerAvailability::Available, "x").is_none()
        );
        for state in [
            DockerAvailability::NotInstalled,
            DockerAvailability::NotRunning,
            DockerAvailability::DaemonError("bad".to_string()),
            DockerAvailability::Unknown,
        ] {
            let result =
                DockerDiscoveryResult::from_unavailable(&state, "host 3").expect("a result");
            assert!(!result.docker_available);
            assert!(result.error.is_some(), "{state:?} must explain itself");
        }
    }

    #[test]
    fn errors_accumulate_rather_than_overwrite() {
        let mut result = DockerDiscoveryResult::available();
        result.add_error("containers failed".to_string());
        result.add_error("images failed".to_string());
        let error = result.error.expect("an error");
        assert!(error.contains("containers failed"), "{error}");
        assert!(error.contains("images failed"), "{error}");
    }

    #[test]
    fn an_available_result_starts_empty_but_usable() {
        let result = DockerDiscoveryResult::available();
        assert!(result.docker_available);
        assert!(result.error.is_none());
        assert!(!result.has_items());
    }

    #[test]
    #[should_panic(expected = "host must be remote")]
    fn a_local_host_cannot_be_checked_over_ssh() {
        let _ = DockerDiscovery::check_availability_remote(&DockerHost::Local);
    }

    #[test]
    fn an_unreachable_remote_host_is_not_reported_as_available() {
        let host = DockerHost::remote(998_877);
        assert!(!DockerDiscovery::is_docker_available_remote(&host));
        assert!(!DockerDiscovery::check_availability_remote(&host).is_available());
    }
}
