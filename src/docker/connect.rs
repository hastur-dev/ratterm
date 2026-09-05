//! Finding and opening the platform's local Docker endpoint.
//!
//! Split out of `client.rs`. Probing touches the filesystem and the
//! environment; deciding what to do with what it found is
//! [`super::transport::choose_transport`], which stays pure.

use std::path::Path;

use bollard::{API_DEFAULT_VERSION, Docker};

use super::host::DockerHost;
use super::transport::{Platform, TransportProbe};

/// Read and write timeout, in seconds, for one API call.
pub(super) const API_TIMEOUT_SECS: u64 = 20;

/// Whether the platform's local Docker endpoint exists.
///
/// Kept out of [`choose_transport`](crate::docker::transport::choose_transport)
/// so the decision stays pure. On Windows a named pipe does not always answer
/// `metadata`, so the pipe directory is listed as a fallback; that is the
/// reliable way to see whether Docker Desktop is up.
#[must_use]
pub fn local_endpoint_present(path: &str) -> bool {
    if Path::new(path).exists() {
        return true;
    }

    #[cfg(windows)]
    {
        let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
        if !name.is_empty()
            && let Ok(entries) = std::fs::read_dir(r"\\.\pipe\")
        {
            return entries
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy() == name);
        }
    }

    false
}

/// Probes a host without deciding anything.
#[must_use]
pub fn probe(host: &DockerHost) -> TransportProbe {
    match host.host_id() {
        Some(id) => TransportProbe::remote(id),
        None => {
            let platform = Platform::current();
            TransportProbe::local(local_endpoint_present(platform.default_endpoint()))
                .on(platform)
                .with_env(std::env::var("DOCKER_HOST").ok())
        }
    }
}

/// Opens a Unix socket connection. Only compiled where one can exist.
#[cfg(unix)]
pub(super) fn connect_socket(path: &str) -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_unix(path, API_TIMEOUT_SECS, API_DEFAULT_VERSION)
}

/// A Unix socket cannot exist on this platform.
#[cfg(not(unix))]
pub(super) fn connect_socket(path: &str) -> Result<Docker, bollard::errors::Error> {
    Err(bollard::errors::Error::SocketNotFoundError(
        path.to_string(),
    ))
}

/// Opens a named pipe connection. Only compiled where one can exist.
#[cfg(windows)]
pub(super) fn connect_pipe(path: &str) -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_named_pipe(path, API_TIMEOUT_SECS, API_DEFAULT_VERSION)
}

/// A named pipe cannot exist on this platform.
#[cfg(not(windows))]
pub(super) fn connect_pipe(path: &str) -> Result<Docker, bollard::errors::Error> {
    Err(bollard::errors::Error::SocketNotFoundError(
        path.to_string(),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_local_endpoint_is_reported_rather_than_connected_to() {
        let path = std::env::temp_dir().join("ratterm-no-such-docker-endpoint-4f2a");
        assert!(
            !local_endpoint_present(&path.to_string_lossy()),
            "the probe must not claim a path that is not there"
        );
    }

    #[test]
    fn a_path_that_does_exist_is_reported_as_present() {
        let dir = std::env::temp_dir();
        assert!(local_endpoint_present(&dir.to_string_lossy()));
    }

    #[test]
    fn probing_a_remote_host_never_looks_at_this_machine() {
        let probe = probe(&DockerHost::remote(12));
        assert_eq!(probe.host_id, Some(12));
        assert!(!probe.local_endpoint_present);
        assert!(probe.docker_host_env.is_none());
    }

    #[test]
    fn probing_the_local_host_uses_this_platform() {
        let probe = probe(&DockerHost::Local);
        assert!(probe.host_id.is_none());
        assert_eq!(probe.platform, Platform::current());
    }

    /// A path nothing is listening on, in the temp directory.
    fn missing_endpoint() -> String {
        std::env::temp_dir()
            .join("ratterm-no-such-endpoint-7c31")
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn a_missing_socket_path_is_refused_on_every_platform() {
        // On Unix bollard checks the path exists; elsewhere the stub reports
        // that a Unix socket cannot exist here. Either way the caller learns
        // this endpoint is unusable before it makes a request.
        assert!(connect_socket(&missing_endpoint()).is_err());
    }

    #[test]
    fn a_named_pipe_is_only_opened_where_one_can_exist() {
        let result = connect_pipe(&missing_endpoint());
        if cfg!(windows) {
            // bollard's named-pipe connector is lazy: it builds a client and
            // connects on the first request, so a missing pipe is reported
            // when the daemon is asked something, not here.
            assert!(result.is_ok());
        } else {
            assert!(
                result.is_err(),
                "a named pipe cannot exist on this platform"
            );
        }
    }
}
