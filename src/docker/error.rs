//! Errors raised by the typed Docker client.
//!
//! Every variant names the next thing a user can do. A message that only says
//! what failed leaves the reader with the same question they started with.

use thiserror::Error;

/// Something went wrong talking to a Docker daemon.
#[derive(Debug, Error)]
pub enum DockerError {
    /// No local daemon endpoint exists on this machine.
    #[error(
        "no local Docker endpoint at {probed}; start Docker (Docker Desktop on Windows and macOS, \
         `systemctl start docker` on Linux) or pick a remote host in the Docker manager"
    )]
    NoLocalEndpoint {
        /// The socket or pipe path that was looked for.
        probed: String,
    },

    /// The SSH host id is not in the registry.
    #[error("no SSH host with id {0}; open the SSH manager and check the host list")]
    UnknownHost(u32),

    /// The SSH port forward to the remote daemon could not be opened.
    #[error(
        "could not forward the Docker port from host {host_id}: {reason}; check the host is \
         reachable and that its daemon listens on {remote}"
    )]
    Forward {
        /// SSH host id the forward was for.
        host_id: u32,
        /// The remote endpoint that was asked for.
        remote: String,
        /// What the forward reported.
        reason: String,
    },

    /// Bollard refused to build a client for the chosen transport.
    #[error("could not open a Docker connection over {transport}: {reason}; {next_step}")]
    Connect {
        /// Which transport was tried, named the way [`super::transport::Transport`] names it.
        transport: String,
        /// What bollard reported.
        reason: String,
        /// What the user should try next.
        next_step: String,
    },

    /// The daemon answered, but not the way the call needed.
    #[error("Docker rejected {operation}: {reason}; {next_step}")]
    Api {
        /// The API call, for example `list containers`.
        operation: String,
        /// What the daemon reported.
        reason: String,
        /// What the user should try next.
        next_step: String,
    },

    /// The async runtime that drives Docker calls could not be used.
    #[error("the Docker runtime is unavailable: {0}; restart ratterm")]
    Runtime(String),

    /// A Compose project was asked for that no container belongs to.
    #[error(
        "no Compose project named {0} is running on this host; refresh the fleet view, or start \
         the project with `docker compose up` on the host"
    )]
    UnknownComposeProject(String),
}

impl DockerError {
    /// Builds an [`DockerError::Api`] from an operation name and a bollard error.
    #[must_use]
    pub fn api(operation: &str, error: &bollard::errors::Error) -> Self {
        let reason = error.to_string();
        let next_step = next_step_for(&reason);
        Self::Api {
            operation: operation.to_string(),
            reason,
            next_step,
        }
    }

    /// Builds a [`DockerError::Connect`] from a transport name and a bollard error.
    #[must_use]
    pub fn connect(transport: &str, error: &bollard::errors::Error) -> Self {
        let reason = error.to_string();
        let next_step = next_step_for(&reason);
        Self::Connect {
            transport: transport.to_string(),
            reason,
            next_step,
        }
    }

    /// True when retrying the same call could plausibly succeed.
    ///
    /// A missing container will not appear on a retry; a refused connection
    /// might, once the daemon is up.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Forward { .. } | Self::Runtime(_) => true,
            Self::Connect { reason, .. } | Self::Api { reason, .. } => {
                let lower = reason.to_lowercase();
                lower.contains("timed out")
                    || lower.contains("timeout")
                    || lower.contains("connection refused")
                    || lower.contains("broken pipe")
            }
            Self::NoLocalEndpoint { .. }
            | Self::UnknownHost(_)
            | Self::UnknownComposeProject(_) => false,
        }
    }
}

/// Chooses the advice line for a daemon message.
///
/// Pure, so every branch is testable without a daemon in that state.
#[must_use]
pub fn next_step_for(reason: &str) -> String {
    let lower = reason.to_lowercase();

    if lower.contains("permission denied") || lower.contains("403") {
        "add your user to the `docker` group on the host, then reconnect".to_string()
    } else if lower.contains("no such container") || lower.contains("404") {
        "refresh the container list; it may have been removed".to_string()
    } else if lower.contains("connection refused") || lower.contains("cannot connect") {
        "start the Docker daemon on that host, then refresh".to_string()
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "check the host's load and network, then retry".to_string()
    } else if lower.contains("409") || lower.contains("conflict") {
        "the container is already in that state; refresh the list".to_string()
    } else {
        "check the Docker daemon log on that host".to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn bollard_error(message: &str) -> bollard::errors::Error {
        bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: message.to_string(),
        }
    }

    #[test]
    fn every_variant_names_a_next_step() {
        let cases = vec![
            DockerError::NoLocalEndpoint {
                probed: "/var/run/docker.sock".to_string(),
            },
            DockerError::UnknownHost(3),
            DockerError::Forward {
                host_id: 3,
                remote: "127.0.0.1:2375".to_string(),
                reason: "refused".to_string(),
            },
            DockerError::Connect {
                transport: "unix socket".to_string(),
                reason: "refused".to_string(),
                next_step: "start the Docker daemon on that host, then refresh".to_string(),
            },
            DockerError::Api {
                operation: "list containers".to_string(),
                reason: "permission denied".to_string(),
                next_step: "add your user to the `docker` group on the host, then reconnect"
                    .to_string(),
            },
            DockerError::Runtime("no worker threads".to_string()),
            DockerError::UnknownComposeProject("web".to_string()),
        ];

        for case in cases {
            let rendered = case.to_string();
            let actionable = [
                "start", "check", "open", "add", "refresh", "restart", "pick",
            ]
            .iter()
            .any(|verb| rendered.contains(verb));
            assert!(actionable, "no next step in: {rendered}");
        }
    }

    #[test]
    fn a_permission_error_points_at_the_docker_group() {
        let step = next_step_for("permission denied while trying to connect");
        assert!(step.contains("docker` group"), "{step}");
    }

    #[test]
    fn a_missing_container_points_at_a_refresh() {
        assert!(next_step_for("No such container: abc").contains("refresh"));
        assert!(next_step_for("404 page not found").contains("refresh"));
    }

    #[test]
    fn a_refused_connection_points_at_starting_the_daemon() {
        assert!(next_step_for("Connection refused (os error 111)").contains("start the Docker"));
        assert!(next_step_for("Cannot connect to the Docker daemon").contains("start the Docker"));
    }

    #[test]
    fn a_conflict_says_the_state_is_already_what_was_asked_for() {
        assert!(next_step_for("409 Conflict").contains("already in that state"));
    }

    #[test]
    fn an_unrecognised_reason_falls_back_to_the_daemon_log() {
        assert!(next_step_for("something entirely new").contains("daemon log"));
    }

    #[test]
    fn an_api_error_carries_the_operation_and_the_advice() {
        let error = DockerError::api("list containers", &bollard_error("permission denied"));
        let rendered = error.to_string();
        assert!(rendered.contains("list containers"), "{rendered}");
        assert!(rendered.contains("docker` group"), "{rendered}");
    }

    #[test]
    fn a_connect_error_names_the_transport() {
        let error = DockerError::connect("npipe //./pipe/docker_engine", &bollard_error("refused"));
        assert!(
            error.to_string().contains("//./pipe/docker_engine"),
            "{error}"
        );
    }

    #[test]
    fn transient_failures_are_told_apart_from_permanent_ones() {
        assert!(
            DockerError::Forward {
                host_id: 1,
                remote: "127.0.0.1:2375".to_string(),
                reason: "refused".to_string(),
            }
            .is_transient()
        );
        assert!(DockerError::api("ping", &bollard_error("timed out")).is_transient());
        assert!(!DockerError::api("start", &bollard_error("No such container")).is_transient());
        assert!(!DockerError::UnknownHost(1).is_transient());
        assert!(
            !DockerError::NoLocalEndpoint {
                probed: "/var/run/docker.sock".to_string()
            }
            .is_transient()
        );
        assert!(!DockerError::UnknownComposeProject("web".to_string()).is_transient());
    }
}
