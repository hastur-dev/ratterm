//! Docker Hub search, image presence and pulls, plus the command strings the
//! terminal multiplexer runs for exec, run, stats and logs.
//!
//! Split out of `discovery.rs`.

use std::process::Command;

use super::cli::{
    COMMAND_TIMEOUT_MS, PULL_TIMEOUT_MS, REMOTE_TIMEOUT_MS, docker_cmd, run_remote_with_timeout,
    run_with_timeout,
};
use super::create::DockerSearchResult;
use super::discovery::DockerDiscovery;
use super::host::DockerHost;
use super::parse::parse_search_results;

/// Docker Hub refuses more than 100 results per query.
const MAX_SEARCH_LIMIT: usize = 100;

/// Assembles a `docker run` command line for an interactive throwaway
/// container.
///
/// Pure: takes the mounts and the optional startup command and returns the
/// string, so the argument order is testable without a daemon.
#[must_use]
pub fn create_container_command(
    docker_binary: &str,
    image: &str,
    volume_mounts: &[String],
    startup_command: Option<&str>,
) -> String {
    assert!(!image.is_empty(), "image must not be empty");

    let mut parts = vec![
        docker_binary.to_string(),
        "run".to_string(),
        "-it".to_string(),
        "--rm".to_string(),
    ];

    for mount in volume_mounts {
        parts.push("-v".to_string());
        parts.push(mount.clone());
    }

    parts.push(image.to_string());

    if let Some(cmd) = startup_command {
        for arg in cmd.split_whitespace() {
            parts.push(arg.to_string());
        }
    }

    parts.join(" ")
}

impl DockerDiscovery {
    /// Searches Docker Hub for images matching the given term.
    ///
    /// # Panics
    /// Panics if `search_term` is empty.
    pub fn search_docker_hub(
        host: &DockerHost,
        search_term: &str,
        limit: usize,
    ) -> Result<Vec<DockerSearchResult>, String> {
        assert!(!search_term.is_empty(), "search_term must not be empty");

        let limit_arg = format!("--limit={}", limit.min(MAX_SEARCH_LIMIT));
        let format_arg = "--format={{json .}}";

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(docker_cmd());
                cmd.args(["search", &limit_arg, format_arg, search_term]);
                run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => run_remote_with_timeout(
                host,
                &["search", &limit_arg, format_arg, search_term],
                REMOTE_TIMEOUT_MS,
            ),
        };

        let output = output.ok_or_else(|| {
            "docker search did not answer in time; check the daemon and your network".to_string()
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("docker search failed: {}", stderr);
            return Err(format!("docker search failed: {}", stderr.trim()));
        }

        Ok(parse_search_results(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    /// Checks whether an image is already present on a host.
    ///
    /// # Panics
    /// Panics if `image_name` is empty.
    pub fn image_exists_on_host(host: &DockerHost, image_name: &str) -> Result<bool, String> {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(docker_cmd());
                cmd.args(["images", "-q", image_name]);
                run_with_timeout(&mut cmd, COMMAND_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => {
                run_remote_with_timeout(host, &["images", "-q", image_name], REMOTE_TIMEOUT_MS)
            }
        };

        let output = output.ok_or_else(|| {
            "docker images did not answer in time; check the daemon is running".to_string()
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker images check failed: {}", stderr.trim()));
        }

        Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }

    /// Pulls an image on the specified host.
    ///
    /// # Panics
    /// Panics if `image_name` is empty.
    pub fn pull_image_on_host(host: &DockerHost, image_name: &str) -> Result<(), String> {
        assert!(!image_name.is_empty(), "image_name must not be empty");

        tracing::info!("Pulling image '{}' on {:?}", image_name, host);

        let output = match host {
            DockerHost::Local => {
                let mut cmd = Command::new(docker_cmd());
                cmd.args(["pull", image_name]);
                run_with_timeout(&mut cmd, PULL_TIMEOUT_MS)
            }
            DockerHost::Remote { .. } => {
                run_remote_with_timeout(host, &["pull", image_name], PULL_TIMEOUT_MS)
            }
        };

        let output = output.ok_or_else(|| {
            "the image pull ran past its ten-minute limit; pull it on the host directly".to_string()
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("docker pull failed: {}", stderr);
            return Err(format!("docker pull failed: {}", stderr.trim()));
        }

        Ok(())
    }

    /// Builds the command to exec into a container.
    ///
    /// # Panics
    /// Panics if `container_id` or `shell` is empty.
    #[must_use]
    pub fn build_exec_command(container_id: &str, shell: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        assert!(!shell.is_empty(), "shell must not be empty");
        format!("{} exec -it {} {}", docker_cmd(), container_id, shell)
    }

    /// Builds the command to run an image as a new container.
    ///
    /// # Panics
    /// Panics if `image` or `shell` is empty.
    #[must_use]
    pub fn build_run_command(image: &str, shell: &str) -> String {
        assert!(!image.is_empty(), "image must not be empty");
        assert!(!shell.is_empty(), "shell must not be empty");
        format!("{} run -it --rm {} {}", docker_cmd(), image, shell)
    }

    /// Builds the command to show container stats.
    ///
    /// # Panics
    /// Panics if `container_id` is empty.
    #[must_use]
    pub fn build_stats_command(container_id: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        format!("{} stats {}", docker_cmd(), container_id)
    }

    /// Builds the command to follow container logs.
    ///
    /// # Panics
    /// Panics if `container_id` is empty.
    #[must_use]
    pub fn build_logs_command(container_id: &str) -> String {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        format!("{} logs -f {}", docker_cmd(), container_id)
    }

    /// Builds the docker run command for container creation.
    ///
    /// # Panics
    /// Panics if `image` is empty.
    #[must_use]
    pub fn build_create_container_command(
        image: &str,
        volume_mounts: &[String],
        startup_command: Option<&str>,
    ) -> String {
        create_container_command(docker_cmd(), image, volume_mounts, startup_command)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_exec_command() {
        let cmd = DockerDiscovery::build_exec_command("abc123", "/bin/bash");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("exec -it"));
        assert!(cmd.contains("abc123"));
        assert!(cmd.contains("/bin/bash"));
    }

    #[test]
    fn test_build_run_command() {
        let cmd = DockerDiscovery::build_run_command("nginx:latest", "/bin/sh");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("run -it --rm"));
        assert!(cmd.contains("nginx:latest"));
        assert!(cmd.contains("/bin/sh"));
    }

    #[test]
    fn test_build_stats_command() {
        let cmd = DockerDiscovery::build_stats_command("abc123");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("stats"));
        assert!(cmd.contains("abc123"));
    }

    #[test]
    fn test_build_logs_command() {
        let cmd = DockerDiscovery::build_logs_command("abc123");
        assert!(cmd.contains("docker"));
        assert!(cmd.contains("logs -f"));
        assert!(cmd.contains("abc123"));
    }

    #[test]
    fn a_create_command_with_no_extras_is_just_run_rm_image() {
        let cmd = create_container_command("docker", "nginx", &[], None);
        assert_eq!(cmd, "docker run -it --rm nginx");
    }

    #[test]
    fn a_create_command_places_mounts_before_the_image() {
        let mounts = vec!["/srv:/data".to_string(), "/etc:/conf".to_string()];
        let cmd = create_container_command("docker", "nginx", &mounts, Some("  sh -c ls  "));
        assert_eq!(
            cmd,
            "docker run -it --rm -v /srv:/data -v /etc:/conf nginx sh -c ls"
        );
    }

    #[test]
    fn a_blank_startup_command_adds_nothing() {
        let cmd = create_container_command("docker", "nginx", &[], Some("   "));
        assert_eq!(cmd, "docker run -it --rm nginx");
    }

    #[test]
    #[should_panic(expected = "image must not be empty")]
    fn a_create_command_without_an_image_is_refused() {
        let _ = create_container_command("docker", "", &[], None);
    }

    #[test]
    #[should_panic(expected = "search_term must not be empty")]
    fn an_empty_search_term_is_refused() {
        let _ = DockerDiscovery::search_docker_hub(&DockerHost::Local, "", 10);
    }

    #[test]
    fn hub_calls_against_an_unreachable_host_fail_rather_than_hang() {
        let host = DockerHost::remote(754_321);
        assert!(DockerDiscovery::search_docker_hub(&host, "nginx", 5).is_err());
        assert!(DockerDiscovery::image_exists_on_host(&host, "nginx").is_err());
        assert!(DockerDiscovery::pull_image_on_host(&host, "nginx").is_err());
    }

    #[test]
    fn a_pull_failure_names_the_host_side_fallback() {
        let host = DockerHost::remote(754_322);
        let err = DockerDiscovery::pull_image_on_host(&host, "nginx").unwrap_err();
        assert!(err.contains("on the host"), "{err}");
    }
}
