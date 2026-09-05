//! Building a `kube` client for a named context, over a tunnel when needed.
//!
//! Two ways of reaching a cluster are supported. A direct endpoint uses the
//! API server address exactly as the kubeconfig gives it. An SSH endpoint
//! opens a loopback port forward through a fleet host first and points the
//! client at the local end, which is how a cluster whose API server is bound
//! to a private address is reached from this machine.
//!
//! The tunnel is rewritten into the kubeconfig rather than into `kube`'s
//! `Config`, so the change is a plain string edit on a serde struct that can
//! be tested without a cluster, and the TLS server name is preserved so the
//! API server's certificate still verifies against its real hostname.

use kube::config::{Config, KubeConfigOptions, Kubeconfig};

use crate::remote::{PortForward, with_shared};

use super::contexts::{self, KubeContext};
use super::endpoint::{ClusterEndpoint, split_server_url};
use super::{K8sError, Result, block_on};

/// A connected client for one context.
///
/// The port forward, when there is one, is owned here so the tunnel lives
/// exactly as long as the client that depends on it.
pub struct K8sClient {
    client: kube::Client,
    context: String,
    namespace: String,
    server: String,
    forward: Option<PortForward>,
}

impl std::fmt::Debug for K8sClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("K8sClient")
            .field("context", &self.context)
            .field("namespace", &self.namespace)
            .field("server", &self.server)
            .field("tunnelled", &self.forward.is_some())
            .finish()
    }
}

impl K8sClient {
    /// Connects to the cluster the endpoint describes.
    ///
    /// Blocks until the client is built. No request is sent, so this succeeds
    /// for a cluster that is configured but down; the first listing is what
    /// reports [`K8sError::Unreachable`].
    ///
    /// # Errors
    /// Any of [`K8sError::NoKubeconfig`], [`K8sError::UnknownContext`],
    /// [`K8sError::UnknownCluster`], [`K8sError::SshTunnel`] or
    /// [`K8sError::Unreachable`].
    pub fn connect(endpoint: &ClusterEndpoint) -> Result<Self> {
        let paths = endpoint.resolved_kubeconfig_paths();
        let mut kubeconfig = contexts::load_kubeconfig_from(&paths)?;
        let set = contexts::contexts_from(&kubeconfig, paths);
        let context: KubeContext = set.require(endpoint.context())?.clone();

        let server = context
            .server
            .clone()
            .ok_or_else(|| K8sError::UnknownCluster {
                context: context.name.clone(),
                cluster: context.cluster.clone(),
            })?;

        let forward = match endpoint.via_ssh_host() {
            None => None,
            Some(host_id) => {
                let (api_host, api_port) = split_server_url(&server)?;
                let tunnel = open_tunnel(host_id, &api_host, api_port)?;
                retarget_cluster(
                    &mut kubeconfig,
                    &context.cluster,
                    tunnel.local_port(),
                    &api_host,
                );
                Some(tunnel)
            }
        };

        let options = KubeConfigOptions {
            context: Some(context.name.clone()),
            cluster: None,
            user: None,
        };

        let context_name = context.name.clone();
        let built = block_on(async move {
            let config = Config::from_custom_kubeconfig(kubeconfig, &options).await?;
            let namespace = config.default_namespace.clone();
            let client = kube::Client::try_from(config)?;
            Ok::<_, kube::Error>((client, namespace))
        })?;

        let (client, namespace) = built.map_err(|e| K8sError::Unreachable {
            context: context_name.clone(),
            reason: e.to_string(),
        })?;

        Ok(Self {
            client,
            context: context_name,
            namespace,
            server,
            forward,
        })
    }

    /// Returns the context this client was built for.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Returns the namespace requests default to.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the API server address from the kubeconfig, before any
    /// tunnelling.
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    /// True when requests go through an SSH tunnel.
    #[must_use]
    pub const fn is_tunnelled(&self) -> bool {
        self.forward.is_some()
    }

    /// Returns the loopback address the tunnel listens on, if there is one.
    #[must_use]
    pub fn tunnel_addr(&self) -> Option<std::net::SocketAddr> {
        self.forward.as_ref().map(PortForward::local_addr)
    }

    /// True when the tunnel is still up, or when there is no tunnel.
    #[must_use]
    pub fn tunnel_is_healthy(&self) -> bool {
        self.forward.as_ref().is_none_or(PortForward::is_running)
    }

    /// Returns the underlying `kube` client.
    ///
    /// Crate-visible on purpose: handing a `kube::Client` to the UI is exactly
    /// what this module exists to prevent.
    pub(crate) fn inner(&self) -> &kube::Client {
        &self.client
    }
}

/// Opens the SSH port forward that fronts an API server.
///
/// # Errors
/// [`K8sError::SshTunnel`] if the host id is unknown or the tunnel cannot be
/// set up. The failure is immediate rather than a hang: the host list is
/// consulted before any connection is attempted.
fn open_tunnel(host_id: u32, api_host: &str, api_port: u16) -> Result<PortForward> {
    with_shared(|executor| executor.forward(host_id, api_host, api_port)).map_err(|e| {
        K8sError::SshTunnel {
            host_id,
            reason: e.to_string(),
        }
    })
}

/// Points a cluster entry at the local end of a tunnel.
///
/// The original hostname is kept as the TLS server name so certificate
/// verification still checks the API server's real identity; without it every
/// tunnelled connection would have to disable verification.
///
/// Returns true when the cluster was found and rewritten.
pub(crate) fn retarget_cluster(
    kubeconfig: &mut Kubeconfig,
    cluster_name: &str,
    local_port: u16,
    original_host: &str,
) -> bool {
    let Some(named) = kubeconfig
        .clusters
        .iter_mut()
        .find(|c| c.name == cluster_name)
    else {
        return false;
    };
    let Some(cluster) = named.cluster.as_mut() else {
        return false;
    };

    let scheme = cluster
        .server
        .as_deref()
        .and_then(|s| s.split_once("://"))
        .map_or("https", |(scheme, _)| scheme)
        .to_string();

    cluster.server = Some(format!("{scheme}://127.0.0.1:{local_port}"));
    if cluster.tls_server_name.is_none() {
        cluster.tls_server_name = Some(original_host.to_string());
    }
    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
current-context: prod
clusters:
  - name: prod-cluster
    cluster:
      server: https://10.0.0.217:6443
      insecure-skip-tls-verify: true
users:
  - name: admin
    user:
      token: abc123
contexts:
  - name: prod
    context:
      cluster: prod-cluster
      user: admin
      namespace: web
"#;

    fn write_kubeconfig(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("config");
        std::fs::write(&path, KUBECONFIG).expect("write");
        path
    }

    #[test]
    fn a_via_ssh_endpoint_with_no_reachable_host_reports_the_tunnel_error() {
        // Host id 4_294_967_290 is not in the SSH host list, so the forward is
        // refused by the registry rather than attempted over the network. The
        // assertion that matters is that this returns instead of hanging.
        let dir = tempfile::tempdir().unwrap();
        let path = write_kubeconfig(&dir);
        let endpoint = ClusterEndpoint::via_ssh("prod", 4_294_967_290).with_kubeconfig(vec![path]);

        let started = std::time::Instant::now();
        let outcome = K8sClient::connect(&endpoint);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "connect should fail fast, took {:?}",
            started.elapsed()
        );

        match outcome {
            Err(K8sError::SshTunnel { host_id, reason }) => {
                assert_eq!(host_id, 4_294_967_290);
                assert!(!reason.is_empty());
            }
            other => panic!("expected SshTunnel, got {other:?}"),
        }
    }

    #[test]
    fn opening_a_tunnel_to_an_unknown_host_fails_immediately() {
        let started = std::time::Instant::now();
        let outcome = open_tunnel(4_294_967_289, "10.0.0.217", 6443);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert!(
            matches!(outcome, Err(K8sError::SshTunnel { .. })),
            "{outcome:?}"
        );
    }

    #[test]
    fn connecting_to_an_unknown_context_is_reported_before_any_network_use() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_kubeconfig(&dir);
        let endpoint = ClusterEndpoint::direct("staging").with_kubeconfig(vec![path]);
        assert!(matches!(
            K8sClient::connect(&endpoint),
            Err(K8sError::UnknownContext(_))
        ));
    }

    #[test]
    fn connecting_without_a_kubeconfig_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint =
            ClusterEndpoint::direct("prod").with_kubeconfig(vec![dir.path().join("absent")]);
        assert!(matches!(
            K8sClient::connect(&endpoint),
            Err(K8sError::NoKubeconfig)
        ));
    }

    #[test]
    fn a_direct_client_can_be_built_without_reaching_the_cluster() {
        // No request is sent while building a client, so this works against an
        // address that does not exist. It is the closest a unit test gets to
        // the connect path.
        let dir = tempfile::tempdir().unwrap();
        let path = write_kubeconfig(&dir);
        let endpoint = ClusterEndpoint::direct("prod").with_kubeconfig(vec![path]);

        let client = K8sClient::connect(&endpoint).expect("client");
        assert_eq!(client.context(), "prod");
        assert_eq!(client.namespace(), "web");
        assert_eq!(client.server(), "https://10.0.0.217:6443");
        assert!(!client.is_tunnelled());
        assert!(client.tunnel_addr().is_none());
        assert!(client.tunnel_is_healthy());
        assert!(format!("{client:?}").contains("prod"));
    }

    #[test]
    fn retargeting_rewrites_the_server_and_keeps_the_tls_name() {
        let mut kubeconfig: Kubeconfig = serde_yaml::from_str(KUBECONFIG).expect("parse");
        assert!(retarget_cluster(
            &mut kubeconfig,
            "prod-cluster",
            51_234,
            "10.0.0.217"
        ));

        let cluster = kubeconfig.clusters[0].cluster.as_ref().expect("cluster");
        assert_eq!(cluster.server.as_deref(), Some("https://127.0.0.1:51234"));
        assert_eq!(cluster.tls_server_name.as_deref(), Some("10.0.0.217"));
    }

    #[test]
    fn retargeting_keeps_an_existing_tls_server_name() {
        let mut kubeconfig: Kubeconfig = serde_yaml::from_str(KUBECONFIG).expect("parse");
        if let Some(cluster) = kubeconfig.clusters[0].cluster.as_mut() {
            cluster.tls_server_name = Some("api.internal".to_string());
        }
        retarget_cluster(&mut kubeconfig, "prod-cluster", 51_234, "10.0.0.217");

        let cluster = kubeconfig.clusters[0].cluster.as_ref().expect("cluster");
        assert_eq!(cluster.tls_server_name.as_deref(), Some("api.internal"));
    }

    #[test]
    fn retargeting_preserves_a_plain_http_scheme() {
        let mut kubeconfig: Kubeconfig = serde_yaml::from_str(KUBECONFIG).expect("parse");
        if let Some(cluster) = kubeconfig.clusters[0].cluster.as_mut() {
            cluster.server = Some("http://10.0.0.217:8080".to_string());
        }
        retarget_cluster(&mut kubeconfig, "prod-cluster", 9_000, "10.0.0.217");

        let cluster = kubeconfig.clusters[0].cluster.as_ref().expect("cluster");
        assert_eq!(cluster.server.as_deref(), Some("http://127.0.0.1:9000"));
    }

    #[test]
    fn retargeting_an_absent_cluster_changes_nothing() {
        let mut kubeconfig: Kubeconfig = serde_yaml::from_str(KUBECONFIG).expect("parse");
        assert!(!retarget_cluster(&mut kubeconfig, "other", 1, "h"));
        let cluster = kubeconfig.clusters[0].cluster.as_ref().expect("cluster");
        assert_eq!(cluster.server.as_deref(), Some("https://10.0.0.217:6443"));
    }

    #[test]
    fn retargeting_a_cluster_entry_with_no_body_changes_nothing() {
        let mut kubeconfig: Kubeconfig = serde_yaml::from_str(KUBECONFIG).expect("parse");
        kubeconfig.clusters[0].cluster = None;
        assert!(!retarget_cluster(
            &mut kubeconfig,
            "prod-cluster",
            1,
            "10.0.0.217"
        ));
    }
}
