//! Forwarding a local port to a port on a pod.
//!
//! This is the Kubernetes equivalent of `kubectl port-forward`, and it is not
//! the same mechanism as [`crate::remote::PortForward`]: that one tunnels over
//! SSH, this one tunnels over the API server's `portforward` subresource. A
//! cluster reached through an SSH tunnel uses both, one inside the other.
//!
//! The listener binds loopback only, so a forwarded pod port is never exposed
//! to the network.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;
use tokio::net::TcpListener;

use super::client::K8sClient;
use super::{K8sError, Result, block_on, runtime};

/// Largest number of connections one pod port forward will serve before it
/// stops. This is the bound on the accept loop.
const MAX_FORWARD_CONNECTIONS: usize = 256;

/// A running local port forward to a pod.
///
/// Dropping this stops accepting new connections and aborts the pump.
pub struct PodPortForward {
    local_addr: SocketAddr,
    namespace: String,
    pod: String,
    remote_port: u16,
    stop: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl std::fmt::Debug for PodPortForward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodPortForward")
            .field("local", &self.local_addr)
            .field("pod", &self.target())
            .field("remote_port", &self.remote_port)
            .field("running", &self.is_running())
            .finish()
    }
}

impl PodPortForward {
    /// Returns the loopback address clients should connect to.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the local port.
    #[must_use]
    pub const fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    /// Returns the pod port traffic is forwarded to.
    #[must_use]
    pub const fn remote_port(&self) -> u16 {
        self.remote_port
    }

    /// Returns `namespace/pod`.
    #[must_use]
    pub fn target(&self) -> String {
        format!("{}/{}", self.namespace, self.pod)
    }

    /// True while the forward is still accepting connections.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::SeqCst) && self.task.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Stops the forward.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for PodPortForward {
    fn drop(&mut self) {
        self.stop();
    }
}

impl K8sClient {
    /// Forwards a local port to a port on a pod.
    ///
    /// Pass `0` as `local_port` to let the operating system choose one;
    /// [`PodPortForward::local_port`] reports which.
    ///
    /// # Errors
    /// [`K8sError::InvalidRequest`] for a zero remote port,
    /// [`K8sError::Unreachable`] if the local port cannot be bound.
    pub fn port_forward(
        &self,
        namespace: &str,
        pod: &str,
        local_port: u16,
        remote_port: u16,
    ) -> Result<PodPortForward> {
        if remote_port == 0 {
            return Err(K8sError::InvalidRequest(
                "a pod port of 0 cannot be forwarded; enter the port the container listens on"
                    .to_string(),
            ));
        }

        let api: Api<Pod> = Api::namespaced(self.inner().clone(), namespace);
        let context = self.context().to_string();

        let listener = block_on(async move {
            TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, local_port))).await
        })?
        .map_err(|e| K8sError::Unreachable {
            context: context.clone(),
            reason: format!("local port {local_port} could not be bound: {e}"),
        })?;

        let local_addr = listener.local_addr().map_err(|e| K8sError::Unreachable {
            context,
            reason: format!("the forwarding socket has no address: {e}"),
        })?;

        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = stop.clone();
        let pod_name = pod.to_string();
        let pump_pod = pod_name.clone();

        let task = runtime()?.spawn(async move {
            accept_loop(listener, api, pump_pod, remote_port, pump_stop).await;
        });

        Ok(PodPortForward {
            local_addr,
            namespace: namespace.to_string(),
            pod: pod_name,
            remote_port,
            stop,
            task: Some(task),
        })
    }
}

/// Accepts connections on the forward's listener and bridges each to the pod.
///
/// The loop is bounded by [`MAX_FORWARD_CONNECTIONS`] and by the stop flag, so
/// it cannot run forever on network input.
async fn accept_loop(
    listener: TcpListener,
    api: Api<Pod>,
    pod: String,
    remote_port: u16,
    stop: Arc<AtomicBool>,
) {
    for _ in 0..MAX_FORWARD_CONNECTIONS {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        let Ok((socket, peer)) = listener.accept().await else {
            break;
        };

        let api = api.clone();
        let pod = pod.clone();
        tokio::spawn(async move {
            if let Err(e) = bridge_one(socket, api, &pod, remote_port).await {
                tracing::warn!("pod forward from {peer} failed: {e}");
            }
        });
    }
    tracing::debug!("pod forward to {pod}:{remote_port} stopped accepting");
}

/// Bridges one accepted connection to the pod's port.
async fn bridge_one(
    mut socket: tokio::net::TcpStream,
    api: Api<Pod>,
    pod: &str,
    remote_port: u16,
) -> std::result::Result<(), String> {
    let mut forwarder = api
        .portforward(pod, &[remote_port])
        .await
        .map_err(|e| e.to_string())?;
    let mut upstream = forwarder
        .take_stream(remote_port)
        .ok_or_else(|| format!("the API server did not open port {remote_port}"))?;

    tokio::io::copy_bidirectional(&mut socket, &mut upstream)
        .await
        .map_err(|e| e.to_string())?;
    drop(upstream);
    forwarder.join().await.map_err(|e| e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::endpoint::ClusterEndpoint;
    use super::*;

    const KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: prod
clusters:
  - name: prod-cluster
    cluster:
      server: https://127.0.0.1:6443
      insecure-skip-tls-verify: true
users:
  - name: admin
    user:
      token: abc123
contexts:
  - name: prod
    context: { cluster: prod-cluster, user: admin, namespace: web }
"#;

    /// Builds a client against an address nothing is listening on. Enough for
    /// argument validation and for binding a local socket, which is all that
    /// can be exercised without a cluster.
    fn offline_client(dir: &tempfile::TempDir) -> K8sClient {
        let path = dir.path().join("config");
        std::fs::write(&path, KUBECONFIG).expect("write");
        K8sClient::connect(&ClusterEndpoint::direct("prod").with_kubeconfig(vec![path]))
            .expect("client")
    }

    #[test]
    fn forwarding_to_pod_port_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let client = offline_client(&dir);

        match client.port_forward("web", "api-0", 0, 0) {
            Err(K8sError::InvalidRequest(message)) => {
                assert!(
                    message.contains("port the container listens on"),
                    "{message}"
                );
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
    }

    #[test]
    fn a_forward_binds_a_loopback_port_and_reports_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let client = offline_client(&dir);

        let mut forward = client
            .port_forward("web", "api-0", 0, 8080)
            .expect("forward");
        assert!(forward.local_addr().ip().is_loopback());
        assert_ne!(forward.local_port(), 0);
        assert_eq!(forward.remote_port(), 8080);
        assert_eq!(forward.target(), "web/api-0");
        assert!(forward.is_running());
        assert!(format!("{forward:?}").contains("web/api-0"));

        forward.stop();
        assert!(!forward.is_running());
        // Stopping twice is harmless.
        forward.stop();
    }

    #[test]
    fn dropping_a_forward_releases_the_local_port() {
        let dir = tempfile::tempdir().unwrap();
        let client = offline_client(&dir);

        let port = {
            let forward = client
                .port_forward("web", "api-0", 0, 8080)
                .expect("forward");
            forward.local_port()
        };

        // Binding the same port again proves the listener was released. The
        // abort is asynchronous, so allow a moment for it.
        let mut rebound = None;
        for _ in 0..50 {
            match std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
                Ok(listener) => {
                    rebound = Some(listener);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        assert!(rebound.is_some(), "port {port} was not released");
    }
}
