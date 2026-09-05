//! Which Docker daemon a command is addressed to.
//!
//! Split out of `container.rs`, which had grown past this project's file-size
//! limit while holding four unrelated groups of types.

use serde::{Deserialize, Serialize};

/// Represents where Docker commands should be executed.
///
/// A remote host is identified by its SSH host id and nothing else. The
/// previous shape copied `hostname`, `port`, `username` and the password in
/// beside the id, so editing a host in the SSH manager left stale copies in
/// `docker_items.toml` and the two could disagree about where to connect.
/// Connection details are resolved from the host registry at call time.
///
/// Files written by earlier versions still load: serde ignores the fields that
/// are gone, and the old `display_name` is read into the `Remote` variant's
/// `cached_label` field.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DockerHost {
    /// Local Docker daemon on this machine.
    #[default]
    Local,
    /// Remote Docker daemon reached through an SSH host.
    Remote {
        /// SSH host id from the host registry.
        host_id: u32,
        /// Label captured when the entry was written.
        ///
        /// Shown before the registry has resolved the real name, and never
        /// used to connect.
        #[serde(
            default,
            alias = "display_name",
            skip_serializing_if = "Option::is_none"
        )]
        cached_label: Option<String>,
    },
}

impl DockerHost {
    /// Creates a remote host from its SSH host id.
    #[must_use]
    pub const fn remote(host_id: u32) -> Self {
        Self::Remote {
            host_id,
            cached_label: None,
        }
    }

    /// Creates a remote host with a label to show until the registry answers.
    #[must_use]
    pub fn remote_labelled(host_id: u32, label: impl Into<String>) -> Self {
        Self::Remote {
            host_id,
            cached_label: Some(label.into()),
        }
    }

    /// Rebuilds a host from the key used by [`DockerHost::fleet_key`].
    ///
    /// `None` is the local daemon; `Some(id)` is the SSH host with that id.
    #[must_use]
    pub const fn from_fleet_key(key: Option<u32>) -> Self {
        match key {
            None => Self::Local,
            Some(id) => Self::remote(id),
        }
    }

    /// Returns true if this is the local host.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// Returns true if this is a remote host.
    #[must_use]
    pub const fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    /// Returns the host ID for remote hosts, None for local.
    #[must_use]
    pub const fn host_id(&self) -> Option<u32> {
        match self {
            Self::Local => None,
            Self::Remote { host_id, .. } => Some(*host_id),
        }
    }

    /// Returns the key this host is filed under in the fleet.
    ///
    /// Identical to [`DockerHost::host_id`] today, but named for its purpose:
    /// the fleet is a `BTreeMap` over this key, so the local daemon (`None`)
    /// always sorts first and remote hosts follow in id order.
    #[must_use]
    pub const fn fleet_key(&self) -> Option<u32> {
        self.host_id()
    }

    /// Returns a name for display without consulting the registry.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Local => "Local".to_string(),
            Self::Remote {
                cached_label: Some(label),
                ..
            } => label.clone(),
            Self::Remote { host_id, .. } => format!("host {host_id}"),
        }
    }

    /// Returns the name the registry knows this host by, falling back to
    /// [`DockerHost::display_name`].
    #[must_use]
    pub fn display_name_in(&self, registry: &crate::hosts::HostRegistry) -> String {
        match self.host_id() {
            None => "Local".to_string(),
            Some(id) => registry.label(id).unwrap_or_else(|| self.display_name()),
        }
    }

    /// Records a label for display.
    pub fn set_cached_label(&mut self, label: impl Into<String>) {
        if let Self::Remote { cached_label, .. } = self {
            *cached_label = Some(label.into());
        }
    }

    /// Returns the storage key for per-host quick-connect.
    #[must_use]
    pub fn storage_key(&self) -> String {
        match self {
            Self::Local => "local".to_string(),
            Self::Remote { host_id, .. } => format!("remote:{}", host_id),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_host_local() {
        let host = DockerHost::Local;
        assert!(host.is_local());
        assert!(!host.is_remote());
        assert_eq!(host.host_id(), None);
        assert_eq!(host.display_name(), "Local");
        assert_eq!(host.storage_key(), "local");
    }

    #[test]
    fn test_docker_host_remote() {
        let host = DockerHost::remote_labelled(1, "My Server");

        assert!(!host.is_local());
        assert!(host.is_remote());
        assert_eq!(host.host_id(), Some(1));
        assert_eq!(host.display_name(), "My Server");
        assert_eq!(host.storage_key(), "remote:1");
    }

    #[test]
    fn an_unlabelled_remote_host_names_itself_by_id() {
        let host = DockerHost::remote(2);
        assert_eq!(host.display_name(), "host 2");
        assert_eq!(host.storage_key(), "remote:2");
    }

    #[test]
    fn a_label_can_be_attached_after_construction() {
        let mut host = DockerHost::remote(3);
        host.set_cached_label("Desk Rock5c");
        assert_eq!(host.display_name(), "Desk Rock5c");

        let mut local = DockerHost::Local;
        local.set_cached_label("ignored");
        assert_eq!(local.display_name(), "Local");
    }

    #[test]
    fn a_remote_host_holds_only_its_id() {
        // The point of the change: no address, user or password is copied in,
        // so nothing here can go stale when the SSH host is edited.
        let host = DockerHost::remote_labelled(7, "label");
        let encoded = serde_json::to_string(&host).expect("serialise");
        assert!(encoded.contains("\"host_id\":7"), "{encoded}");
        assert!(!encoded.contains("hostname"), "{encoded}");
        assert!(!encoded.contains("username"), "{encoded}");
        assert!(!encoded.contains("password"), "{encoded}");
    }

    #[test]
    fn an_entry_written_by_an_older_version_still_loads() {
        let legacy = r#"{
            "type": "remote",
            "host_id": 5,
            "hostname": "10.0.0.18",
            "port": 22,
            "username": "hastur",
            "display_name": "Desk Rock5c"
        }"#;

        let host: DockerHost = serde_json::from_str(legacy).expect("parse legacy entry");
        assert_eq!(host.host_id(), Some(5));
        assert_eq!(
            host.display_name(),
            "Desk Rock5c",
            "the old display name is kept as a label"
        );
    }

    #[test]
    fn a_host_round_trips_through_serde() {
        for host in [
            DockerHost::Local,
            DockerHost::remote(1),
            DockerHost::remote_labelled(2, "named"),
        ] {
            let encoded = serde_json::to_string(&host).expect("serialise");
            let decoded: DockerHost = serde_json::from_str(&encoded).expect("deserialise");
            assert_eq!(decoded, host);
        }
    }

    #[test]
    fn a_fleet_key_round_trips_and_orders_local_first() {
        assert_eq!(DockerHost::Local.fleet_key(), None);
        assert_eq!(DockerHost::remote(4).fleet_key(), Some(4));
        assert_eq!(DockerHost::from_fleet_key(None), DockerHost::Local);
        assert_eq!(DockerHost::from_fleet_key(Some(4)), DockerHost::remote(4));

        let mut keys = vec![Some(9u32), None, Some(1)];
        keys.sort_unstable();
        assert_eq!(keys, vec![None, Some(1), Some(9)]);
    }

    #[test]
    fn a_label_on_a_key_rebuilt_host_is_absent_rather_than_wrong() {
        // Rebuilding from a key cannot invent a label, so the caller sees the
        // id rather than a stale name from another host.
        let rebuilt = DockerHost::from_fleet_key(Some(11));
        assert_eq!(rebuilt.display_name(), "host 11");
    }
}
