//! A typed Docker client.
//!
//! Replaces the `ssh host docker ps --format ...` string-and-parse path with
//! bollard talking the Docker API. The daemon is reached over whichever
//! transport [`super::transport::choose_transport`] picked; for a remote host
//! that is an SSH port forward, and the client owns the forward so the tunnel
//! lives exactly as long as the connection does.

use bollard::container::{
    ListContainersOptions, RemoveContainerOptions, RestartContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::image::ListImagesOptions;
use bollard::network::ListNetworksOptions;
use bollard::volume::ListVolumesOptions;
use bollard::{API_DEFAULT_VERSION, Docker};

use crate::remote::{ExecError, PortForward};

use super::connect::{API_TIMEOUT_SECS, connect_pipe, connect_socket};
use super::error::DockerError;
use super::host::DockerHost;
use super::model::{
    ContainerDetail, DockerNetwork, DockerVolume, container_from_summary, images_from_summary,
    network_from_api, volume_from_api,
};

use super::transport::{Transport, TransportChoice, Unreachable, choose_transport};

// Probing lives in `connect`, but every caller reaches for it alongside the
// client, so it is re-exported here.
pub use super::connect::{local_endpoint_present, probe};

/// Seconds the daemon waits for a container to stop before killing it.
const STOP_GRACE_SECS: i64 = 10;

/// Everything one host's daemon reported in a single refresh.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSnapshot {
    /// Containers, running and stopped, with their labels.
    pub containers: Vec<ContainerDetail>,
    /// Images, one row per repository tag.
    pub images: Vec<super::container::DockerImage>,
    /// Volumes.
    pub volumes: Vec<DockerVolume>,
    /// Networks.
    pub networks: Vec<DockerNetwork>,
}

impl HostSnapshot {
    /// How many containers are running.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.containers
            .iter()
            .filter(|c| c.container.is_running())
            .count()
    }
}

/// A connection to one Docker daemon.
pub struct DockerClient {
    docker: Docker,
    transport: Transport,
    /// Kept alive for the life of the client; dropping it closes the tunnel.
    _forward: Option<PortForward>,
}

impl std::fmt::Debug for DockerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerClient")
            .field("transport", &self.transport)
            .finish()
    }
}

impl DockerClient {
    /// Connects to the daemon for `host`, choosing the transport automatically.
    ///
    /// # Errors
    /// Returns the reason no connection could be made, naming the next step.
    pub fn connect(host: &DockerHost) -> Result<Self, DockerError> {
        Self::connect_with(&choose_transport(&probe(host)))
    }

    /// Connects using an already-decided transport.
    ///
    /// # Errors
    /// Returns the reason the transport could not be opened.
    pub fn connect_with(choice: &TransportChoice) -> Result<Self, DockerError> {
        let name = choice.describe();
        match choice {
            TransportChoice::UnixSocket(path) => {
                let docker = connect_socket(path).map_err(|e| DockerError::connect(&name, &e))?;
                Ok(Self {
                    docker,
                    transport: Transport::UnixSocket(path.clone()),
                    _forward: None,
                })
            }
            TransportChoice::NamedPipe(path) => {
                let docker = connect_pipe(path).map_err(|e| DockerError::connect(&name, &e))?;
                Ok(Self {
                    docker,
                    transport: Transport::NamedPipe(path.clone()),
                    _forward: None,
                })
            }
            TransportChoice::Environment(endpoint) => {
                let docker =
                    Docker::connect_with_http(endpoint, API_TIMEOUT_SECS, API_DEFAULT_VERSION)
                        .map_err(|e| DockerError::connect(&name, &e))?;
                Ok(Self {
                    docker,
                    transport: Transport::Environment(endpoint.clone()),
                    _forward: None,
                })
            }
            TransportChoice::SshForward {
                host_id,
                remote_host,
                remote_port,
            } => Self::connect_forwarded(*host_id, remote_host, *remote_port),
            TransportChoice::Unavailable(Unreachable::NoLocalEndpoint { probed }) => {
                Err(DockerError::NoLocalEndpoint {
                    probed: probed.clone(),
                })
            }
        }
    }

    /// Opens an SSH forward to a remote daemon and connects to its local end.
    fn connect_forwarded(
        host_id: u32,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<Self, DockerError> {
        let remote = format!("{remote_host}:{remote_port}");
        let forward = crate::remote::with_shared(|executor| {
            executor.forward(host_id, remote_host, remote_port)
        })
        .map_err(|e| match e {
            ExecError::UnknownHost(id) => DockerError::UnknownHost(id),
            ExecError::Session(session) => DockerError::Forward {
                host_id,
                remote: remote.clone(),
                reason: session.to_string(),
            },
        })?;

        let local = forward.local_addr().to_string();
        let endpoint = format!("http://{local}");
        let transport = Transport::Forwarded {
            host_id,
            local,
            remote,
        };

        let docker = Docker::connect_with_http(&endpoint, API_TIMEOUT_SECS, API_DEFAULT_VERSION)
            .map_err(|e| DockerError::connect(&transport.to_string(), &e))?;

        Ok(Self {
            docker,
            transport,
            _forward: Some(forward),
        })
    }

    /// The transport this client is using, and therefore why.
    #[must_use]
    pub const fn transport(&self) -> &Transport {
        &self.transport
    }

    /// The underlying bollard client, for callers that need the raw API.
    #[must_use]
    pub const fn inner(&self) -> &Docker {
        &self.docker
    }

    /// Asks the daemon to confirm it is there.
    ///
    /// # Errors
    /// Returns an error if the daemon does not answer.
    pub async fn ping(&self) -> Result<String, DockerError> {
        self.docker
            .ping()
            .await
            .map_err(|e| DockerError::api("ping", &e))
    }

    /// Lists containers, stopped ones included.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn list_containers(&self) -> Result<Vec<ContainerDetail>, DockerError> {
        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };
        let summaries = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|e| DockerError::api("list containers", &e))?;
        Ok(summaries
            .iter()
            .filter_map(container_from_summary)
            .collect())
    }

    /// Lists images, one row per repository tag.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn list_images(&self) -> Result<Vec<super::container::DockerImage>, DockerError> {
        let options = ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        };
        let summaries = self
            .docker
            .list_images(Some(options))
            .await
            .map_err(|e| DockerError::api("list images", &e))?;
        Ok(summaries.iter().flat_map(images_from_summary).collect())
    }

    /// Lists volumes.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn list_volumes(&self) -> Result<Vec<DockerVolume>, DockerError> {
        let response = self
            .docker
            .list_volumes(Some(ListVolumesOptions::<String>::default()))
            .await
            .map_err(|e| DockerError::api("list volumes", &e))?;
        Ok(response
            .volumes
            .unwrap_or_default()
            .iter()
            .map(volume_from_api)
            .collect())
    }

    /// Lists networks.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn list_networks(&self) -> Result<Vec<DockerNetwork>, DockerError> {
        let networks = self
            .docker
            .list_networks(Some(ListNetworksOptions::<String>::default()))
            .await
            .map_err(|e| DockerError::api("list networks", &e))?;
        Ok(networks.iter().map(network_from_api).collect())
    }

    /// Collects containers, images, volumes and networks in one pass.
    ///
    /// # Errors
    /// Returns the first call that failed; a host that answers partially is
    /// better reported as failed than as half a picture.
    pub async fn snapshot(&self) -> Result<HostSnapshot, DockerError> {
        Ok(HostSnapshot {
            containers: self.list_containers().await?,
            images: self.list_images().await?,
            volumes: self.list_volumes().await?,
            networks: self.list_networks().await?,
        })
    }

    /// Starts a container.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn start_container(&self, id: &str) -> Result<(), DockerError> {
        self.docker
            .start_container(id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| DockerError::api("start container", &e))
    }

    /// Stops a container, giving it a grace period first.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn stop_container(&self, id: &str) -> Result<(), DockerError> {
        let options = StopContainerOptions { t: STOP_GRACE_SECS };
        self.docker
            .stop_container(id, Some(options))
            .await
            .map_err(|e| DockerError::api("stop container", &e))
    }

    /// Restarts a container.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn restart_container(&self, id: &str) -> Result<(), DockerError> {
        let options = RestartContainerOptions {
            t: STOP_GRACE_SECS as isize,
        };
        self.docker
            .restart_container(id, Some(options))
            .await
            .map_err(|e| DockerError::api("restart container", &e))
    }

    /// Removes a container.
    ///
    /// # Errors
    /// Returns an error if the daemon refuses the call.
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<(), DockerError> {
        let options = RemoveContainerOptions {
            force,
            ..Default::default()
        };
        self.docker
            .remove_container(id, Some(options))
            .await
            .map_err(|e| DockerError::api("remove container", &e))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::model::ContainerDetail;
    use super::*;
    use crate::docker::container::DockerContainer;
    use std::collections::HashMap;

    fn detail(id: &str, status: &str) -> ContainerDetail {
        ContainerDetail {
            container: DockerContainer::new(
                id.to_string(),
                id.to_string(),
                "img".to_string(),
                status.to_string(),
            ),
            labels: HashMap::new(),
            state: status.to_lowercase(),
            command: String::new(),
        }
    }

    #[test]
    fn a_snapshot_counts_only_running_containers() {
        let snapshot = HostSnapshot {
            containers: vec![
                detail("a", "Up 2 hours"),
                detail("b", "Exited (0) 1 hour ago"),
                detail("c", "Up 5 minutes"),
            ],
            ..Default::default()
        };
        assert_eq!(snapshot.running_count(), 2);
    }

    #[test]
    fn an_empty_snapshot_counts_nothing() {
        assert_eq!(HostSnapshot::default().running_count(), 0);
    }

    #[test]
    fn connecting_with_no_local_endpoint_says_what_it_looked_for() {
        let choice = TransportChoice::Unavailable(Unreachable::NoLocalEndpoint {
            probed: "/var/run/docker.sock".to_string(),
        });
        match DockerClient::connect_with(&choice) {
            Err(DockerError::NoLocalEndpoint { probed }) => {
                assert_eq!(probed, "/var/run/docker.sock");
            }
            other => panic!("expected NoLocalEndpoint, got {other:?}"),
        }
    }

    #[test]
    fn connecting_through_an_unregistered_ssh_host_reports_the_id() {
        let choice = TransportChoice::SshForward {
            host_id: 883_311,
            remote_host: "127.0.0.1",
            remote_port: 2375,
        };
        match DockerClient::connect_with(&choice) {
            Err(DockerError::UnknownHost(id)) => assert_eq!(id, 883_311),
            other => panic!("expected UnknownHost, got {other:?}"),
        }
    }

    #[test]
    fn connecting_to_a_socket_path_that_does_not_exist_fails_with_the_transport_named() {
        let missing = std::env::temp_dir().join("ratterm-missing-docker.sock");
        let path = missing.to_string_lossy().to_string();
        // A Unix socket is refused on every platform: where one can exist
        // bollard checks the path, and where one cannot the stub says so.
        match DockerClient::connect_with(&TransportChoice::UnixSocket(path)) {
            Err(DockerError::Connect { transport, .. }) => {
                assert!(transport.contains("unix socket"), "{transport}");
            }
            other => panic!("expected a Connect error, got {other:?}"),
        }
    }

    #[test]
    fn a_client_built_over_http_reports_its_transport() {
        // `connect_with_http` does no I/O, so this builds a client without a
        // daemon and proves the transport is recorded and printable.
        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        let client = DockerClient::connect_with(&choice).expect("http clients build lazily");
        assert_eq!(client.transport().endpoint(), "http://127.0.0.1:1/");
        assert!(!client.transport().is_forwarded());
        assert!(format!("{client:?}").contains("DOCKER_HOST"));
    }
}
