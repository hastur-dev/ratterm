//! One host in the fleet, and where its connection has got to.
//!
//! Split out of `fleet.rs`.

use std::sync::Arc;

use super::client::{DockerClient, HostSnapshot};
use super::compose::{ComposeGrouping, group_by_project};
use super::host::DockerHost;

/// Where one host's connection has got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostConnection {
    /// Known about, not yet tried.
    Idle,
    /// A connection attempt is in progress.
    Connecting,
    /// The daemon answered.
    Connected {
        /// How it is being reached, as [`super::transport::Transport`] names it.
        transport: String,
        /// Why that transport was chosen.
        rationale: &'static str,
        /// Unix seconds the connection was made.
        since: i64,
    },
    /// The connection failed, and why.
    Failed {
        /// The reason, including the next step to try.
        reason: String,
        /// Unix seconds the attempt failed.
        at: i64,
    },
}

impl HostConnection {
    /// True once the daemon has answered.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// True when the last attempt failed.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// A short word for the host header.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Connected { .. } => "connected",
            Self::Failed { .. } => "failed",
        }
    }

    /// The detail line under the host header, if there is one.
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Idle | Self::Connecting => None,
            Self::Connected {
                transport,
                rationale,
                ..
            } => Some(format!("{transport} — {rationale}")),
            Self::Failed { reason, .. } => Some(reason.clone()),
        }
    }
}

/// One host in the fleet.
pub struct FleetHost {
    /// `None` for the local daemon, `Some(id)` for an SSH host.
    pub key: Option<u32>,
    /// The host's display name.
    pub label: String,
    /// Where its connection has got to.
    pub connection: HostConnection,
    /// What the last successful refresh found.
    pub snapshot: HostSnapshot,
    /// Unix seconds of the last successful refresh.
    pub last_refresh: Option<i64>,
    pub(super) client: Option<Arc<DockerClient>>,
}

impl std::fmt::Debug for FleetHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FleetHost")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("connection", &self.connection)
            .field("containers", &self.snapshot.containers.len())
            .finish()
    }
}

impl FleetHost {
    /// A host that is known about but not yet connected to.
    #[must_use]
    pub(super) fn idle(key: Option<u32>, label: &str) -> Self {
        Self {
            key,
            label: label.to_string(),
            connection: HostConnection::Idle,
            snapshot: HostSnapshot::default(),
            last_refresh: None,
            client: None,
        }
    }

    /// The host as the rest of the application addresses it.
    #[must_use]
    pub const fn docker_host(&self) -> DockerHost {
        DockerHost::from_fleet_key(self.key)
    }

    /// The connected client, if there is one.
    #[must_use]
    pub fn client(&self) -> Option<Arc<DockerClient>> {
        self.client.clone()
    }

    /// This host's containers grouped into Compose projects.
    #[must_use]
    pub fn compose(&self) -> ComposeGrouping {
        group_by_project(&self.snapshot.containers)
    }

    /// A one-line header for the host group.
    #[must_use]
    pub fn header(&self) -> String {
        format!(
            "{} [{}] {}/{} running",
            self.label,
            self.connection.as_str(),
            self.snapshot.running_count(),
            self.snapshot.containers.len()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn connection_states_have_words_and_details() {
        assert_eq!(HostConnection::Idle.as_str(), "idle");
        assert_eq!(HostConnection::Connecting.as_str(), "connecting");
        assert!(HostConnection::Connecting.detail().is_none());

        let failed = HostConnection::Failed {
            reason: "no route to host".to_string(),
            at: 1,
        };
        assert_eq!(failed.as_str(), "failed");
        assert_eq!(failed.detail().as_deref(), Some("no route to host"));
        assert!(failed.is_failed());
        assert!(!failed.is_connected());
    }

    #[test]
    fn a_host_with_no_client_reports_none() {
        let host = FleetHost::idle(Some(1), "rock5c");
        assert!(host.client().is_none());
        assert_eq!(host.docker_host(), DockerHost::remote(1));
        assert_eq!(host.header(), "rock5c [idle] 0/0 running");
        assert!(host.compose().is_empty());
    }
}
