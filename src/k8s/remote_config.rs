//! Kubeconfigs that live on a fleet host.
//!
//! A single-node cluster on a fleet machine keeps its kubeconfig on that
//! machine. This reads it over SFTP and caches it locally, so the context list
//! and the client can both use it like any other kubeconfig file. Reading is
//! done with SFTP rather than by running a command on the host, so nothing
//! depends on what shell or tooling the host has.

use std::path::PathBuf;

use crate::remote::{SftpClient, with_shared};
use crate::terminal::SSHContext;

use super::atomic::write_atomic;
use super::{K8sError, Result};

/// Where a kubeconfig usually lives on a fleet host, relative to the login
/// directory. SFTP does not expand `~`, so the path is relative.
pub const REMOTE_KUBECONFIG_PATH: &str = ".kube/config";

/// A kubeconfig that lives on a fleet host, cached locally after one read.
///
/// It is cached because reading it costs an SSH round trip and it changes
/// rarely. Use [`RemoteKubeconfig::refresh`] when the cluster has been rebuilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteKubeconfig {
    host_id: u32,
    remote_path: String,
    cache_dir: PathBuf,
}

impl RemoteKubeconfig {
    /// Describes the kubeconfig at [`REMOTE_KUBECONFIG_PATH`] on a host.
    #[must_use]
    pub fn new(host_id: u32) -> Self {
        Self::at(host_id, REMOTE_KUBECONFIG_PATH)
    }

    /// Describes a kubeconfig at an explicit path on a host.
    ///
    /// A k3s host, for example, keeps one at
    /// `/etc/rancher/k3s/k3s.yaml` rather than in a login directory.
    #[must_use]
    pub fn at(host_id: u32, remote_path: impl Into<String>) -> Self {
        Self {
            host_id,
            remote_path: remote_path.into(),
            cache_dir: default_cache_dir(),
        }
    }

    /// Caches into a different directory.
    #[must_use]
    pub fn with_cache_dir(mut self, dir: PathBuf) -> Self {
        self.cache_dir = dir;
        self
    }

    /// Returns the fleet host this kubeconfig comes from.
    #[must_use]
    pub const fn host_id(&self) -> u32 {
        self.host_id
    }

    /// Returns the path on the remote host.
    #[must_use]
    pub fn remote_path(&self) -> &str {
        &self.remote_path
    }

    /// Returns where the local copy is kept.
    #[must_use]
    pub fn cache_path(&self) -> PathBuf {
        self.cache_dir
            .join(format!("host-{}.kubeconfig", self.host_id))
    }

    /// True when a local copy already exists.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cache_path().is_file()
    }

    /// Returns the local copy, fetching it if there is not one yet.
    ///
    /// # Errors
    /// [`K8sError::SshTunnel`] if the host cannot be reached or the file
    /// cannot be read, [`K8sError::Storage`] if the cache cannot be written.
    pub fn ensure(&self) -> Result<PathBuf> {
        let cached = self.cache_path();
        if cached.is_file() {
            return Ok(cached);
        }
        self.refresh()
    }

    /// Reads the remote file again, replacing any cached copy.
    ///
    /// # Errors
    /// See [`RemoteKubeconfig::ensure`].
    pub fn refresh(&self) -> Result<PathBuf> {
        let target = with_shared(|executor| executor.target(self.host_id).cloned()).ok_or(
            K8sError::SshTunnel {
                host_id: self.host_id,
                reason: format!("host {} is not in the SSH host list", self.host_id),
            },
        )?;

        let mut ssh_context = SSHContext::new(
            target.username.clone(),
            target.hostname.clone(),
            target.port,
        );
        ssh_context.host_id = Some(self.host_id);
        ssh_context.password = target.password.as_ref().map(|p| (**p).clone());
        ssh_context.key_path = target.key_path.as_ref().map(|p| p.display().to_string());

        let client = SftpClient::connect(&ssh_context).map_err(|e| K8sError::SshTunnel {
            host_id: self.host_id,
            reason: e.to_string(),
        })?;
        let (content, _) =
            client
                .read_file(&self.remote_path)
                .map_err(|e| K8sError::SshTunnel {
                    host_id: self.host_id,
                    reason: format!("{} could not be read: {e}", self.remote_path),
                })?;

        let cached = self.cache_path();
        write_atomic(&cached, content.as_bytes())?;
        Ok(cached)
    }

    /// Removes the local copy, if there is one.
    ///
    /// # Errors
    /// [`K8sError::Storage`] if the file exists but cannot be removed.
    pub fn forget(&self) -> Result<()> {
        let cached = self.cache_path();
        if !cached.exists() {
            return Ok(());
        }
        std::fs::remove_file(&cached).map_err(|e| {
            K8sError::Storage(format!("{} could not be removed: {e}", cached.display()))
        })
    }
}

/// Where fetched kubeconfigs are cached.
fn default_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ratterm")
        .join("k8s")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_kubeconfig_names_its_host_and_cache_file() {
        let dir = tempfile::tempdir().unwrap();
        let remote = RemoteKubeconfig::new(7).with_cache_dir(dir.path().to_path_buf());

        assert_eq!(remote.host_id(), 7);
        assert_eq!(remote.remote_path(), REMOTE_KUBECONFIG_PATH);
        assert_eq!(remote.cache_path(), dir.path().join("host-7.kubeconfig"));
        assert!(!remote.is_cached());
    }

    #[test]
    fn a_remote_kubeconfig_can_use_an_explicit_path() {
        let remote = RemoteKubeconfig::at(2, "/etc/rancher/k3s/k3s.yaml");
        assert_eq!(remote.remote_path(), "/etc/rancher/k3s/k3s.yaml");
    }

    #[test]
    fn two_hosts_cache_to_different_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = RemoteKubeconfig::new(1).with_cache_dir(dir.path().to_path_buf());
        let second = RemoteKubeconfig::new(2).with_cache_dir(dir.path().to_path_buf());
        assert_ne!(first.cache_path(), second.cache_path());
    }

    #[test]
    fn the_default_cache_directory_is_under_the_ratterm_directory() {
        let path = default_cache_dir();
        assert!(path.ends_with("k8s"), "{}", path.display());
        assert!(
            path.to_string_lossy().contains(".ratterm"),
            "{}",
            path.display()
        );
    }

    #[test]
    fn a_cached_remote_kubeconfig_is_returned_without_touching_the_network() {
        let dir = tempfile::tempdir().unwrap();
        let remote = RemoteKubeconfig::new(9).with_cache_dir(dir.path().to_path_buf());
        std::fs::write(remote.cache_path(), "apiVersion: v1\nkind: Config\n").expect("seed cache");

        assert!(remote.is_cached());
        assert_eq!(remote.ensure().expect("ensure"), remote.cache_path());
    }

    #[test]
    fn refreshing_from_an_unknown_host_reports_the_tunnel_error() {
        let dir = tempfile::tempdir().unwrap();
        let remote = RemoteKubeconfig::new(4_294_967_288).with_cache_dir(dir.path().to_path_buf());

        let started = std::time::Instant::now();
        let outcome = remote.refresh();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "refresh should fail fast, took {:?}",
            started.elapsed()
        );

        match outcome {
            Err(K8sError::SshTunnel { host_id, reason }) => {
                assert_eq!(host_id, 4_294_967_288);
                assert!(reason.contains("host list"), "{reason}");
            }
            other => panic!("expected SshTunnel, got {other:?}"),
        }
    }

    #[test]
    fn ensuring_an_uncached_kubeconfig_from_an_unknown_host_reports_the_same_error() {
        let dir = tempfile::tempdir().unwrap();
        let remote = RemoteKubeconfig::new(4_294_967_287).with_cache_dir(dir.path().to_path_buf());
        assert!(matches!(remote.ensure(), Err(K8sError::SshTunnel { .. })));
    }

    #[test]
    fn forgetting_a_cached_kubeconfig_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let remote = RemoteKubeconfig::new(11).with_cache_dir(dir.path().to_path_buf());
        std::fs::write(remote.cache_path(), "apiVersion: v1\n").expect("seed cache");

        remote.forget().expect("forget");
        assert!(!remote.is_cached());
        // Forgetting again is harmless.
        remote.forget().expect("forget twice");
    }
}
