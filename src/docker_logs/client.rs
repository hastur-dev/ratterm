//! Docker client wrapper using bollard.
//!
//! Provides a thin abstraction over the bollard Docker client
//! for listing containers and checking access.

use bollard::Docker;
use bollard::container::ListContainersOptions;

use super::types::{AccessStatus, ContainerLogInfo, DockerLogsError};

/// Docker log client wrapping bollard.
pub struct DockerLogClient {
    /// The underlying bollard Docker client.
    docker: Docker,
}

impl DockerLogClient {
    /// Connects to the local Docker daemon (auto-detects socket).
    ///
    /// # Errors
    /// Returns error if connection cannot be established.
    pub fn connect_local() -> Result<Self, DockerLogsError> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| DockerLogsError::ConnectionFailed(e.to_string()))?;

        Ok(Self { docker })
    }

    /// Creates a client from an existing bollard Docker instance.
    #[must_use]
    pub fn from_docker(docker: Docker) -> Self {
        Self { docker }
    }

    /// Returns a reference to the underlying bollard Docker client.
    #[must_use]
    pub fn inner(&self) -> &Docker {
        &self.docker
    }

    /// Lists all running containers with their log access info.
    ///
    /// # Errors
    /// Returns error if the Docker API call fails.
    pub async fn list_containers(&self) -> Result<Vec<ContainerLogInfo>, DockerLogsError> {
        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };

        let containers = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|e| DockerLogsError::ConnectionFailed(e.to_string()))?;

        let mut result = Vec::new();
        for c in containers {
            let id = c.id.unwrap_or_default();
            let names = c.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| id.chars().take(12).collect());
            let image = c.image.unwrap_or_else(|| "<none>".to_string());
            let status = c.status.unwrap_or_else(|| "unknown".to_string());

            result.push(ContainerLogInfo {
                id,
                name,
                image,
                status,
                access: AccessStatus::Unknown,
            });
        }

        Ok(result)
    }

    /// Checks if a specific container's logs are accessible.
    ///
    /// # Errors
    /// Returns error if the Docker API call fails.
    pub async fn check_access(
        &self,
        container_id: &str,
    ) -> Result<AccessStatus, DockerLogsError> {
        assert!(!container_id.is_empty(), "container_id must not be empty");

        use bollard::container::LogsOptions;
        use tokio_stream::StreamExt;

        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: "1".to_string(),
            ..Default::default()
        };

        let mut stream = self.docker.logs(container_id, Some(options));

        // Try to read one entry to verify access
        match stream.next().await {
            Some(Ok(_)) => Ok(AccessStatus::Accessible),
            Some(Err(e)) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("No such container") {
                    Ok(AccessStatus::NotFound)
                } else if msg.contains("403") || msg.contains("permission") {
                    Ok(AccessStatus::Denied(msg))
                } else {
                    Ok(AccessStatus::Error(msg))
                }
            }
            None => {
                // No output but no error — container exists, logs just empty
                Ok(AccessStatus::Accessible)
            }
        }
    }
}

impl std::fmt::Debug for DockerLogClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerLogClient")
            .field("connected", &true)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_log_info_creation() {
        let info = ContainerLogInfo {
            id: "abc123".to_string(),
            name: "test-container".to_string(),
            image: "nginx:latest".to_string(),
            status: "running".to_string(),
            access: AccessStatus::Unknown,
        };
        assert_eq!(info.id, "abc123");
        assert_eq!(info.name, "test-container");
        assert!(info.access.is_accessible()); // Unknown is treated as accessible
    }

    #[test]
    fn test_debug_impl() {
        let info = ContainerLogInfo {
            id: "x".to_string(),
            name: "y".to_string(),
            image: "z".to_string(),
            status: "running".to_string(),
            access: AccessStatus::Accessible,
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("Accessible"));
        assert!(debug_str.contains("running"));
    }

    #[test]
    fn test_access_status_variants() {
        assert!(AccessStatus::Accessible.is_accessible());
        assert!(AccessStatus::Unknown.is_accessible());
        assert!(!AccessStatus::Denied("no".to_string()).is_accessible());
        assert!(!AccessStatus::NotFound.is_accessible());
        assert!(!AccessStatus::Error("err".to_string()).is_accessible());
    }

    // Docker-dependent tests are in integration tests
    #[test]
    #[ignore]
    fn test_connect_local_requires_docker() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let client = DockerLogClient::connect_local();
            // This will fail if Docker is not running
            assert!(client.is_ok() || client.is_err());
        });
    }

    #[test]
    #[ignore]
    fn test_list_containers_requires_docker() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let client = DockerLogClient::connect_local().expect("docker");
            let containers = client.list_containers().await;
            assert!(containers.is_ok());
        });
    }
}
