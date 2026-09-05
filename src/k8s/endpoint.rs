//! Describing how to reach a cluster, and reading an API server address.
//!
//! An endpoint is the pair of "which kubeconfig context" and "does the
//! connection go through a fleet host". It carries no client and no
//! connection, so it can be built, stored and compared without touching the
//! network — which is what lets the UI keep one per screen.

use std::path::PathBuf;

use super::contexts;
use super::{K8sError, Result};

/// Default port for an API server URL that gives no port.
const DEFAULT_HTTPS_PORT: u16 = 443;

/// Default port for a plain-HTTP API server URL that gives no port.
const DEFAULT_HTTP_PORT: u16 = 80;

/// Splits an API server URL into a host and a port.
///
/// Written by hand rather than with a URL crate because the only thing needed
/// is the authority, and because the result has to survive the shapes that
/// turn up in kubeconfigs: bare hosts with no scheme, IPv6 literals in
/// brackets, and trailing paths.
///
/// # Errors
/// [`K8sError::InvalidRequest`] if there is no host, or the port is not a
/// number.
pub fn split_server_url(server: &str) -> Result<(String, u16)> {
    let trimmed = server.trim();
    if trimmed.is_empty() {
        return Err(K8sError::InvalidRequest(
            "the cluster has no server address; add one to the kubeconfig".to_string(),
        ));
    }

    let (scheme, rest) = match trimmed.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), trimmed),
    };

    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .trim_end_matches('.');
    if authority.is_empty() {
        return Err(K8sError::InvalidRequest(format!(
            "the server address '{server}' has no host; fix the cluster entry in the kubeconfig"
        )));
    }

    let default_port = if scheme == "http" {
        DEFAULT_HTTP_PORT
    } else {
        DEFAULT_HTTPS_PORT
    };

    // An IPv6 literal is bracketed; the port, if any, follows the bracket.
    if let Some(end) = authority.strip_prefix('[').and_then(|a| a.find(']')) {
        let host = &authority[1..=end];
        let remainder = &authority[end + 2..];
        let port = parse_port(remainder.strip_prefix(':'), default_port, server)?;
        return Ok((host.to_string(), port));
    }

    // A bare IPv6 literal has several colons and no port.
    if authority.matches(':').count() > 1 {
        return Ok((authority.to_string(), default_port));
    }

    match authority.split_once(':') {
        Some((host, port)) if !host.is_empty() => Ok((
            host.to_string(),
            parse_port(Some(port), default_port, server)?,
        )),
        Some(_) => Err(K8sError::InvalidRequest(format!(
            "the server address '{server}' has no host; fix the cluster entry in the kubeconfig"
        ))),
        None => Ok((authority.to_string(), default_port)),
    }
}

/// Parses an optional port, falling back to the scheme's default.
fn parse_port(raw: Option<&str>, default: u16, server: &str) -> Result<u16> {
    match raw {
        None | Some("") => Ok(default),
        Some(text) => text.parse::<u16>().map_err(|_| {
            K8sError::InvalidRequest(format!(
                "the server address '{server}' has an invalid port '{text}'; fix the cluster entry \
                 in the kubeconfig"
            ))
        }),
    }
}

/// How to reach a cluster's API server.
///
/// The context name identifies which kubeconfig entry to use;
/// [`ClusterEndpoint::via_ssh_host`] adds an SSH hop in front of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterEndpoint {
    context: String,
    via_ssh_host: Option<u32>,
    kubeconfig_paths: Vec<PathBuf>,
}

impl ClusterEndpoint {
    /// An endpoint whose API server address is reachable from this machine.
    #[must_use]
    pub fn direct(context: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            via_ssh_host: None,
            kubeconfig_paths: Vec::new(),
        }
    }

    /// An endpoint reached through an SSH port forward from a fleet host.
    ///
    /// `host_id` is an id from the SSH host list; the forward is opened
    /// against the API server's own host and port, taken from the kubeconfig.
    #[must_use]
    pub fn via_ssh(context: impl Into<String>, host_id: u32) -> Self {
        Self {
            context: context.into(),
            via_ssh_host: Some(host_id),
            kubeconfig_paths: Vec::new(),
        }
    }

    /// Reads the context from specific kubeconfig files instead of the ones
    /// `KUBECONFIG` and `~/.kube/config` point at.
    ///
    /// This is how a kubeconfig fetched from a fleet host is used.
    #[must_use]
    pub fn with_kubeconfig(mut self, paths: Vec<PathBuf>) -> Self {
        self.kubeconfig_paths = paths;
        self
    }

    /// Returns the context name.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Returns the fleet host the connection tunnels through, if any.
    #[must_use]
    pub const fn via_ssh_host(&self) -> Option<u32> {
        self.via_ssh_host
    }

    /// True when no SSH hop is involved.
    #[must_use]
    pub const fn is_direct(&self) -> bool {
        self.via_ssh_host.is_none()
    }

    /// Returns the kubeconfig files to read, falling back to the machine's
    /// configured ones.
    #[must_use]
    pub fn resolved_kubeconfig_paths(&self) -> Vec<PathBuf> {
        if self.kubeconfig_paths.is_empty() {
            contexts::kubeconfig_paths()
        } else {
            self.kubeconfig_paths.clone()
        }
    }

    /// Returns a one-line description for the status bar.
    #[must_use]
    pub fn describe(&self) -> String {
        match self.via_ssh_host {
            Some(host_id) => format!("{} via SSH host {host_id}", self.context),
            None => self.context.clone(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_direct_endpoint_has_no_ssh_hop() {
        let endpoint = ClusterEndpoint::direct("prod");
        assert_eq!(endpoint.context(), "prod");
        assert!(endpoint.is_direct());
        assert!(endpoint.via_ssh_host().is_none());
        assert_eq!(endpoint.describe(), "prod");
    }

    #[test]
    fn a_via_ssh_endpoint_names_its_host() {
        let endpoint = ClusterEndpoint::via_ssh("prod", 4);
        assert_eq!(endpoint.context(), "prod");
        assert!(!endpoint.is_direct());
        assert_eq!(endpoint.via_ssh_host(), Some(4));
        assert_eq!(endpoint.describe(), "prod via SSH host 4");
    }

    #[test]
    fn a_direct_and_a_via_ssh_endpoint_are_not_equal() {
        assert_ne!(
            ClusterEndpoint::direct("prod"),
            ClusterEndpoint::via_ssh("prod", 4)
        );
    }

    #[test]
    fn an_endpoint_without_explicit_paths_falls_back_to_the_machine_default() {
        let endpoint = ClusterEndpoint::direct("prod");
        assert_eq!(
            endpoint.resolved_kubeconfig_paths(),
            contexts::kubeconfig_paths()
        );
    }

    #[test]
    fn an_endpoint_with_explicit_paths_uses_them() {
        let endpoint =
            ClusterEndpoint::direct("prod").with_kubeconfig(vec![PathBuf::from("/tmp/kc")]);
        assert_eq!(
            endpoint.resolved_kubeconfig_paths(),
            vec![PathBuf::from("/tmp/kc")]
        );
    }

    #[test]
    fn a_server_url_splits_into_host_and_port() {
        assert_eq!(
            split_server_url("https://10.0.0.217:6443").expect("split"),
            ("10.0.0.217".to_string(), 6443)
        );
        assert_eq!(
            split_server_url("https://api.example.invalid:443/").expect("split"),
            ("api.example.invalid".to_string(), 443)
        );
    }

    #[test]
    fn a_server_url_without_a_port_uses_the_scheme_default() {
        assert_eq!(
            split_server_url("https://api.example.invalid").expect("split"),
            ("api.example.invalid".to_string(), 443)
        );
        assert_eq!(
            split_server_url("http://api.example.invalid").expect("split"),
            ("api.example.invalid".to_string(), 80)
        );
    }

    #[test]
    fn a_server_url_without_a_scheme_is_still_split() {
        assert_eq!(
            split_server_url("10.0.0.217:6443").expect("split"),
            ("10.0.0.217".to_string(), 6443)
        );
    }

    #[test]
    fn a_bracketed_ipv6_server_url_splits_correctly() {
        assert_eq!(
            split_server_url("https://[fd00::1]:6443").expect("split"),
            ("fd00::1".to_string(), 6443)
        );
        assert_eq!(
            split_server_url("https://[fd00::1]").expect("split"),
            ("fd00::1".to_string(), 443)
        );
    }

    #[test]
    fn a_bare_ipv6_server_url_takes_the_default_port() {
        assert_eq!(
            split_server_url("https://fd00::1").expect("split"),
            ("fd00::1".to_string(), 443)
        );
    }

    #[test]
    fn a_server_url_with_a_path_keeps_only_the_authority() {
        assert_eq!(
            split_server_url("https://api.example.invalid:6443/k8s/clusters/c-1").expect("split"),
            ("api.example.invalid".to_string(), 6443)
        );
    }

    #[test]
    fn an_empty_server_url_is_refused() {
        assert!(matches!(
            split_server_url("   "),
            Err(K8sError::InvalidRequest(_))
        ));
    }

    #[test]
    fn a_server_url_with_no_host_is_refused() {
        assert!(matches!(
            split_server_url("https://:6443"),
            Err(K8sError::InvalidRequest(_))
        ));
    }

    #[test]
    fn a_server_url_with_a_bad_port_is_refused() {
        match split_server_url("https://api.invalid:not-a-port") {
            Err(K8sError::InvalidRequest(msg)) => assert!(msg.contains("port"), "{msg}"),
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        assert!(matches!(
            split_server_url("https://api.invalid:99999"),
            Err(K8sError::InvalidRequest(_))
        ));
    }
}
