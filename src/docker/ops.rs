//! Container and image lifecycle operations over the CLI transport.
//!
//! Split out of `discovery.rs`. These keep the CLI shape because they are
//! reached from synchronous UI code; the typed equivalents on
//! [`super::client::DockerClient`] are what the fleet view uses.

use std::process::Command;

use super::cli::{REMOTE_TIMEOUT_MS, docker_cmd, run_remote_with_timeout, run_with_timeout};
use super::discovery::DockerDiscovery;
use super::host::DockerHost;

/// Builds the argument list for `docker rm` / `docker rmi`.
///
/// Pure, so the `-f` placement is testable without running anything.
#[must_use]
pub fn removal_args<'a>(subcommand: &'a str, force: bool, id: &'a str) -> Vec<&'a str> {
    let mut args = vec![subcommand];
    if force {
        args.push("-f");
    }
    args.push(id);
    args
}

/// Turns a failed command's output into a message naming the next step.
#[must_use]
fn failure_message(action: &str, stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        format!("docker {action} failed with no output; run it by hand on the host to see why")
    } else {
        format!("docker {action} failed: {trimmed}")
    }
}

impl DockerDiscovery {
    /// Runs a lifecycle subcommand on a host and maps the outcome to a message.
    fn run_lifecycle(host: &DockerHost, action: &str, args: &[&str]) -> Result<(), String> {
        let output = match host {
            DockerHost::Local => Command::new(docker_cmd())
                .args(args)
                .output()
                .map_err(|e| format!("could not run the docker CLI: {e}"))?,
            DockerHost::Remote { .. } => run_remote_with_timeout(host, args, REMOTE_TIMEOUT_MS)
                .ok_or_else(|| {
                    format!(
                        "docker {action} did not answer on {}; check the host is reachable in the SSH manager",
                        host.display_name()
                    )
                })?,
        };

        if output.status.success() {
            Ok(())
        } else {
            Err(failure_message(
                action,
                &String::from_utf8_lossy(&output.stderr),
            ))
        }
    }

    /// Starts a container on the local daemon.
    pub fn start_container(container_id: &str) -> Result<(), String> {
        Self::start_container_on_host(container_id, &DockerHost::Local)
    }

    /// Starts a container on the specified host.
    ///
    /// # Panics
    /// Panics if `container_id` is empty.
    pub fn start_container_on_host(container_id: &str, host: &DockerHost) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        Self::run_lifecycle(host, "start", &["start", container_id])
    }

    /// Stops a container on the local daemon.
    pub fn stop_container(container_id: &str) -> Result<(), String> {
        Self::stop_container_on_host(container_id, &DockerHost::Local)
    }

    /// Stops a container on the specified host.
    ///
    /// # Panics
    /// Panics if `container_id` is empty.
    pub fn stop_container_on_host(container_id: &str, host: &DockerHost) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        Self::run_lifecycle(host, "stop", &["stop", container_id])
    }

    /// Removes a container on the local daemon.
    pub fn remove_container(container_id: &str, force: bool) -> Result<(), String> {
        Self::remove_container_on_host(container_id, force, &DockerHost::Local)
    }

    /// Removes a container on the specified host.
    ///
    /// # Panics
    /// Panics if `container_id` is empty.
    pub fn remove_container_on_host(
        container_id: &str,
        force: bool,
        host: &DockerHost,
    ) -> Result<(), String> {
        assert!(!container_id.is_empty(), "container_id must not be empty");
        let args = removal_args("rm", force, container_id);
        Self::run_lifecycle(host, "rm", &args)
    }

    /// Removes an image on the local daemon.
    pub fn remove_image(image_id: &str, force: bool) -> Result<(), String> {
        Self::remove_image_on_host(image_id, force, &DockerHost::Local)
    }

    /// Removes an image on the specified host.
    ///
    /// # Panics
    /// Panics if `image_id` is empty.
    pub fn remove_image_on_host(
        image_id: &str,
        force: bool,
        host: &DockerHost,
    ) -> Result<(), String> {
        assert!(!image_id.is_empty(), "image_id must not be empty");
        let args = removal_args("rmi", force, image_id);
        Self::run_lifecycle(host, "rmi", &args)
    }

    /// Starts Docker Desktop (Windows/macOS) or the Docker service (Linux).
    ///
    /// Returns Ok(()) if the start was requested successfully.
    pub fn start_docker_desktop() -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", "start", "", "Docker Desktop"])
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    format!("could not start Docker Desktop: {e}; start it from the Start menu")
                })
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .args(["-a", "Docker"])
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    format!("could not start Docker Desktop: {e}; start it from Applications")
                })
        }

        #[cfg(target_os = "linux")]
        {
            match Command::new("systemctl").args(["start", "docker"]).output() {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output) => Err(format!(
                    "could not start the docker service: {}; try `sudo systemctl start docker`",
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(e) => Err(format!(
                    "could not run systemctl: {e}; start the docker service by hand"
                )),
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Err("this platform has no known way to start Docker; start it by hand".to_string())
        }
    }

    /// Returns the Docker version string from the local daemon, or None.
    #[must_use]
    pub fn docker_version() -> Option<String> {
        let mut cmd = Command::new(docker_cmd());
        cmd.args(["version", "--format", "{{.Server.Version}}"]);

        let output = run_with_timeout(&mut cmd, super::cli::QUICK_TIMEOUT_MS)?;
        if !output.status.success() {
            return None;
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if version.is_empty() {
            None
        } else {
            Some(version)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn removal_without_force_has_no_flag() {
        assert_eq!(removal_args("rm", false, "abc"), vec!["rm", "abc"]);
        assert_eq!(removal_args("rmi", false, "img"), vec!["rmi", "img"]);
    }

    #[test]
    fn removal_with_force_puts_the_flag_before_the_id() {
        assert_eq!(removal_args("rm", true, "abc"), vec!["rm", "-f", "abc"]);
        assert_eq!(removal_args("rmi", true, "img"), vec!["rmi", "-f", "img"]);
    }

    #[test]
    fn a_silent_failure_still_says_what_to_do_next() {
        let message = failure_message("stop", "   ");
        assert!(message.contains("no output"), "{message}");
        assert!(message.contains("by hand"), "{message}");
    }

    #[test]
    fn a_failure_with_output_repeats_what_docker_said() {
        let message = failure_message("rm", "  No such container: abc\n");
        assert_eq!(message, "docker rm failed: No such container: abc");
    }

    #[test]
    fn lifecycle_calls_against_an_unreachable_host_report_the_host_by_name() {
        let host = DockerHost::remote_labelled(765_432, "ghost-box");
        let err = DockerDiscovery::stop_container_on_host("abc", &host).unwrap_err();
        assert!(err.contains("ghost-box"), "{err}");
        assert!(err.contains("SSH manager"), "{err}");

        let err = DockerDiscovery::remove_container_on_host("abc", true, &host).unwrap_err();
        assert!(err.contains("ghost-box"), "{err}");

        let err = DockerDiscovery::remove_image_on_host("img", false, &host).unwrap_err();
        assert!(err.contains("ghost-box"), "{err}");

        let err = DockerDiscovery::start_container_on_host("abc", &host).unwrap_err();
        assert!(err.contains("ghost-box"), "{err}");
    }

    #[test]
    #[should_panic(expected = "container_id must not be empty")]
    fn an_empty_container_id_is_refused() {
        let _ = DockerDiscovery::stop_container_on_host("", &DockerHost::Local);
    }

    #[test]
    #[should_panic(expected = "image_id must not be empty")]
    fn an_empty_image_id_is_refused() {
        let _ = DockerDiscovery::remove_image_on_host("", false, &DockerHost::Local);
    }
}
