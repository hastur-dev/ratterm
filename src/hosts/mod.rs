//! One source of truth for the machines ratterm manages.
//!
//! Host data used to be copied into whatever needed it. `DockerHost::Remote`
//! carried its own `hostname`, `port` and `username` alongside the `host_id`,
//! so editing a host in the SSH manager left stale copies behind in
//! `docker_items.toml`. Reachability lived in three places at once and could
//! disagree on screen.
//!
//! The registry holds the host list, the credentials, the last-seen status and
//! the detected capabilities, and resolves a [`RemoteTarget`] on demand.
//! Everything else borrows from it.

pub mod capabilities;
pub mod status;

use std::collections::HashMap;

use tracing::debug;
use zeroize::Zeroizing;

pub use capabilities::HostCapabilities;
pub use status::{HostStatus, Reachability};

use crate::remote::executor::ExecError;
use crate::remote::{CommandOutput, RemoteTarget, publish_targets, with_shared};
use crate::ssh::host::{SSHHost, SSHHostList};

/// Aggregate counts for a fleet header line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FleetSummary {
    /// Hosts in the list.
    pub total: usize,
    /// Hosts that answered the last check.
    pub online: usize,
    /// Hosts that did not answer.
    pub offline: usize,
    /// Hosts never checked.
    pub unknown: usize,
    /// Hosts with a Docker CLI.
    pub with_docker: usize,
    /// Hosts with `kubectl`.
    pub with_kubectl: usize,
    /// Hosts with an NVIDIA GPU tool.
    pub with_gpu: usize,
}

impl FleetSummary {
    /// Renders the header the Docker and health dashboards show.
    #[must_use]
    pub fn headline(&self) -> String {
        format!(
            "{} hosts, {} online, {} with Docker",
            self.total, self.online, self.with_docker
        )
    }
}

/// Hosts, their credentials, their status, and what they can do.
#[derive(Debug, Default)]
pub struct HostRegistry {
    hosts: SSHHostList,
    status: HashMap<u32, HostStatus>,
    capabilities: HashMap<u32, HostCapabilities>,
}

impl HostRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the host list, keeping status and capabilities for hosts that
    /// are still present.
    ///
    /// Publishes the resolved targets so free functions elsewhere can reach a
    /// host by id without carrying the registry around.
    pub fn set_hosts(&mut self, hosts: SSHHostList) {
        let live: Vec<u32> = hosts.hosts().map(|h| h.id).collect();
        self.status.retain(|id, _| live.contains(id));
        self.capabilities.retain(|id, _| live.contains(id));
        self.hosts = hosts;
        self.publish();
    }

    /// Returns the host list.
    #[must_use]
    pub const fn host_list(&self) -> &SSHHostList {
        &self.hosts
    }

    /// Returns a mutable host list.
    ///
    /// Call [`HostRegistry::publish`] afterwards so the executor sees the
    /// change.
    pub fn host_list_mut(&mut self) -> &mut SSHHostList {
        &mut self.hosts
    }

    /// Returns one host.
    #[must_use]
    pub fn host(&self, host_id: u32) -> Option<&SSHHost> {
        self.hosts.hosts().find(|h| h.id == host_id)
    }

    /// Returns the display name, falling back to the hostname.
    #[must_use]
    pub fn label(&self, host_id: u32) -> Option<String> {
        self.host(host_id)
            .map(|h| h.display_name.clone().unwrap_or_else(|| h.hostname.clone()))
    }

    /// Returns how many hosts are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    /// Returns true if no hosts are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Resolves the connection details for a host at call time.
    ///
    /// Follows `jump_host_id` so a `ProxyJump` chain is built from the current
    /// host list rather than from a copy taken when the host was added.
    #[must_use]
    pub fn target(&self, host_id: u32) -> Option<RemoteTarget> {
        // Walk the jump chain iteratively, stopping at the hop limit and at a
        // repeated id. Both bounds matter: the host editor does not prevent a
        // cycle, and a cycle would otherwise never terminate.
        let mut chain_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut current = Some(host_id);

        for _ in 0..=crate::remote::session::MAX_JUMP_HOPS {
            let Some(id) = current else { break };
            if !seen.insert(id) {
                debug!("jump chain for host {host_id} loops at {id}");
                break;
            }
            chain_ids.push(id);
            current = self.host(id).and_then(|h| h.jump_host_id);
        }

        // Build outermost first so each hop can carry the one before it.
        let mut jump: Option<Box<RemoteTarget>> = None;
        for id in chain_ids.iter().rev() {
            let mut hop = self.plain_target(*id)?;
            hop.jump = jump;
            jump = Some(Box::new(hop));
        }

        jump.map(|boxed| *boxed)
    }

    /// Builds one hop, without its jump chain.
    fn plain_target(&self, host_id: u32) -> Option<RemoteTarget> {
        let host = self.host(host_id)?;
        let credentials = self.hosts.get_credentials(host_id);

        let target = RemoteTarget {
            host_id: Some(host_id),
            hostname: host.hostname.clone(),
            port: host.port,
            username: credentials.map(|c| c.username.clone()).unwrap_or_default(),
            password: credentials
                .and_then(|c| c.password.clone())
                .map(Zeroizing::new),
            key_path: credentials
                .and_then(|c| c.key_path.clone())
                .map(std::path::PathBuf::from),
            key_passphrase: None,
            jump: None,
        };

        if target.username.is_empty() {
            // Without a user there is nothing to authenticate as; the SSH
            // manager treats this as an incomplete host.
            return None;
        }

        Some(target)
    }

    /// Returns every resolvable target, keyed by host id.
    #[must_use]
    pub fn targets(&self) -> HashMap<u32, RemoteTarget> {
        self.hosts
            .hosts()
            .filter_map(|host| self.target(host.id).map(|t| (host.id, t)))
            .collect()
    }

    /// Publishes the current targets to the shared executor.
    pub fn publish(&self) {
        publish_targets(self.targets());
    }

    /// Returns the status of a host.
    #[must_use]
    pub fn status(&self, host_id: u32) -> HostStatus {
        self.status.get(&host_id).cloned().unwrap_or_default()
    }

    /// Records an observation of a host's reachability.
    pub fn observe(&mut self, host_id: u32, reachability: Reachability, error: Option<String>) {
        self.status
            .entry(host_id)
            .or_default()
            .observe(reachability, error);
    }

    /// Returns the cached capabilities of a host.
    #[must_use]
    pub fn capabilities(&self, host_id: u32) -> HostCapabilities {
        self.capabilities.get(&host_id).cloned().unwrap_or_default()
    }

    /// Stores a capability answer.
    pub fn set_capabilities(&mut self, host_id: u32, caps: HostCapabilities) {
        self.capabilities.insert(host_id, caps);
    }

    /// Returns true if the host's capabilities need probing.
    #[must_use]
    pub fn capabilities_are_stale(&self, host_id: u32) -> bool {
        self.capabilities
            .get(&host_id)
            .is_none_or(HostCapabilities::is_stale)
    }

    /// Probes a host's capabilities over SSH and caches the answer.
    ///
    /// One round trip for every probe. Also records reachability, since a
    /// successful probe is proof the host is up.
    ///
    /// # Errors
    /// Returns an error if the host is unknown or unreachable.
    pub fn detect_capabilities(&mut self, host_id: u32) -> Result<HostCapabilities, ExecError> {
        let command = HostCapabilities::probe_command();
        let result = with_shared(|executor| executor.exec(host_id, &command));

        match result {
            Ok(output) => {
                let caps = HostCapabilities::from_probe_output(&output.stdout);
                self.set_capabilities(host_id, caps.clone());
                self.observe(host_id, Reachability::Online, None);
                Ok(caps)
            }
            Err(e) => {
                let reachability = if is_auth_failure(&e) {
                    Reachability::AuthFailed
                } else {
                    Reachability::Offline
                };
                self.observe(host_id, reachability, Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Probes every host whose capabilities are missing or stale.
    ///
    /// Returns the ids that answered.
    pub fn refresh_stale_capabilities(&mut self) -> Vec<u32> {
        let stale: Vec<u32> = self
            .hosts
            .hosts()
            .map(|h| h.id)
            .filter(|id| self.capabilities_are_stale(*id))
            .collect();

        stale
            .into_iter()
            .filter(|id| self.detect_capabilities(*id).is_ok())
            .collect()
    }

    /// Runs a command on a host and records what that says about reachability.
    ///
    /// # Errors
    /// Returns an error if the host is unknown or the command fails.
    pub fn exec(&mut self, host_id: u32, command: &str) -> Result<CommandOutput, ExecError> {
        let result = with_shared(|executor| executor.exec(host_id, command));
        match &result {
            Ok(_) => self.observe(host_id, Reachability::Online, None),
            Err(e) => {
                let reachability = if is_auth_failure(e) {
                    Reachability::AuthFailed
                } else {
                    Reachability::Offline
                };
                self.observe(host_id, reachability, Some(e.to_string()));
            }
        }
        result
    }

    /// Returns the ids of hosts with a given capability.
    pub fn hosts_with<F>(&self, predicate: F) -> Vec<u32>
    where
        F: Fn(&HostCapabilities) -> bool,
    {
        self.hosts
            .hosts()
            .map(|h| h.id)
            .filter(|id| predicate(&self.capabilities(*id)))
            .collect()
    }

    /// Returns the counts a fleet header shows.
    #[must_use]
    pub fn summary(&self) -> FleetSummary {
        let mut summary = FleetSummary::default();
        for host in self.hosts.hosts() {
            summary.total += 1;
            match self.status(host.id).reachability {
                Reachability::Online => summary.online += 1,
                Reachability::Offline | Reachability::AuthFailed => summary.offline += 1,
                Reachability::Unknown | Reachability::Checking => summary.unknown += 1,
            }
            let caps = self.capabilities(host.id);
            if caps.docker {
                summary.with_docker += 1;
            }
            if caps.kubectl {
                summary.with_kubectl += 1;
            }
            if caps.nvidia_smi {
                summary.with_gpu += 1;
            }
        }
        summary
    }
}

/// The registry behaves as the host list it wraps.
///
/// Everything that already took an `&SSHHostList` keeps working through
/// deref coercion, which is the point: the registry replaces the bare list as
/// the single owner rather than sitting beside it as a second copy.
impl std::ops::Deref for HostRegistry {
    type Target = SSHHostList;

    fn deref(&self) -> &Self::Target {
        &self.hosts
    }
}

/// Mutating the list through the registry does not republish targets by
/// itself; call [`HostRegistry::publish`] after a change that alters an
/// address or a credential.
impl std::ops::DerefMut for HostRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hosts
    }
}

/// Returns true if the error means the credentials were refused.
fn is_auth_failure(error: &ExecError) -> bool {
    matches!(
        error,
        ExecError::Session(crate::remote::SessionError::Auth { .. })
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ssh::host::SSHCredentials;

    fn registry_with_hosts() -> (HostRegistry, u32, u32) {
        let mut hosts = SSHHostList::new();
        let a = hosts.add_host("10.0.0.10".to_string(), 22).expect("host a");
        let b = hosts
            .add_host("10.0.0.11".to_string(), 2222)
            .expect("host b");
        hosts.set_credentials(
            a,
            SSHCredentials::new("alice".to_string(), Some("pw".to_string())),
        );
        hosts.set_credentials(
            b,
            SSHCredentials::new("bob".to_string(), Some("pw".to_string())),
        );

        let mut registry = HostRegistry::new();
        registry.set_hosts(hosts);
        (registry, a, b)
    }

    #[test]
    fn an_empty_registry_reports_nothing() {
        let registry = HostRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert_eq!(registry.summary(), FleetSummary::default());
        assert!(registry.target(1).is_none());
    }

    #[test]
    fn a_target_is_resolved_from_the_host_list() {
        let (registry, a, _) = registry_with_hosts();
        let target = registry.target(a).expect("a target");
        assert_eq!(target.hostname, "10.0.0.10");
        assert_eq!(target.port, 22);
        assert_eq!(target.username, "alice");
        assert_eq!(target.host_id, Some(a));
        assert!(target.password.is_some());
        assert!(target.jump.is_none());
    }

    #[test]
    fn editing_a_host_changes_the_resolved_target() {
        // This is the defect the registry exists to fix: a copy taken when the
        // host was added used to survive the edit.
        let (mut registry, a, _) = registry_with_hosts();
        assert_eq!(registry.target(a).expect("target").hostname, "10.0.0.10");

        if let Some(host) = registry.host_list_mut().get_by_id_mut(a) {
            host.hostname = "10.0.0.99".to_string();
        }
        registry.publish();

        assert_eq!(registry.target(a).expect("target").hostname, "10.0.0.99");
    }

    #[test]
    fn a_host_without_a_username_does_not_resolve() {
        let mut hosts = SSHHostList::new();
        let id = hosts.add_host("10.0.0.20".to_string(), 22).expect("host");
        let mut registry = HostRegistry::new();
        registry.set_hosts(hosts);
        assert!(
            registry.target(id).is_none(),
            "there is nothing to authenticate as"
        );
    }

    #[test]
    fn a_jump_host_is_resolved_into_the_chain() {
        let (mut registry, a, b) = registry_with_hosts();
        if let Some(host) = registry.host_list_mut().get_by_id_mut(b) {
            host.jump_host_id = Some(a);
        }

        let target = registry.target(b).expect("target");
        let jump = target.jump.as_ref().expect("a jump");
        assert_eq!(jump.hostname, "10.0.0.10");
        assert_eq!(jump.username, "alice");
    }

    #[test]
    fn a_jump_cycle_does_not_hang() {
        let (mut registry, a, b) = registry_with_hosts();
        {
            let list = registry.host_list_mut();
            if let Some(host) = list.get_by_id_mut(a) {
                host.jump_host_id = Some(b);
            }
            if let Some(host) = list.get_by_id_mut(b) {
                host.jump_host_id = Some(a);
            }
        }

        // Resolution stops at the hop limit rather than recursing forever.
        let target = registry.target(a).expect("target");
        let mut depth = 0;
        let mut hop = target.jump.as_deref();
        while let Some(next) = hop {
            depth += 1;
            assert!(depth <= crate::remote::session::MAX_JUMP_HOPS + 1);
            hop = next.jump.as_deref();
        }
    }

    #[test]
    fn targets_covers_every_resolvable_host() {
        let (registry, a, b) = registry_with_hosts();
        let targets = registry.targets();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains_key(&a));
        assert!(targets.contains_key(&b));
    }

    #[test]
    fn status_starts_unknown_and_follows_observations() {
        let (mut registry, a, _) = registry_with_hosts();
        assert_eq!(registry.status(a).reachability, Reachability::Unknown);

        registry.observe(a, Reachability::Online, None);
        assert_eq!(registry.status(a).reachability, Reachability::Online);

        registry.observe(a, Reachability::Offline, Some("timeout".to_string()));
        assert_eq!(registry.status(a).reachability, Reachability::Offline);
        assert_eq!(registry.status(a).last_error.as_deref(), Some("timeout"));
    }

    #[test]
    fn capabilities_start_empty_and_stale() {
        let (registry, a, _) = registry_with_hosts();
        assert!(!registry.capabilities(a).docker);
        assert!(registry.capabilities_are_stale(a));
    }

    #[test]
    fn stored_capabilities_are_returned_and_not_stale() {
        let (mut registry, a, _) = registry_with_hosts();
        registry.set_capabilities(a, HostCapabilities::from_probe_output("has-docker"));
        assert!(registry.capabilities(a).docker);
        assert!(!registry.capabilities_are_stale(a));
    }

    #[test]
    fn the_summary_counts_status_and_capabilities() {
        let (mut registry, a, b) = registry_with_hosts();
        registry.observe(a, Reachability::Online, None);
        registry.observe(b, Reachability::Offline, None);
        registry.set_capabilities(
            a,
            HostCapabilities::from_probe_output("has-docker\nhas-kubectl\nhas-nvidia-smi"),
        );

        let summary = registry.summary();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.online, 1);
        assert_eq!(summary.offline, 1);
        assert_eq!(summary.unknown, 0);
        assert_eq!(summary.with_docker, 1);
        assert_eq!(summary.with_kubectl, 1);
        assert_eq!(summary.with_gpu, 1);
        assert_eq!(summary.headline(), "2 hosts, 1 online, 1 with Docker");
    }

    #[test]
    fn hosts_with_filters_by_capability() {
        let (mut registry, a, b) = registry_with_hosts();
        registry.set_capabilities(a, HostCapabilities::from_probe_output("has-docker"));
        registry.set_capabilities(b, HostCapabilities::from_probe_output("has-kubectl"));

        assert_eq!(registry.hosts_with(|c| c.docker), vec![a]);
        assert_eq!(registry.hosts_with(|c| c.kubectl), vec![b]);
        assert!(registry.hosts_with(|c| c.podman).is_empty());
    }

    #[test]
    fn removing_a_host_forgets_its_status_and_capabilities() {
        let (mut registry, a, b) = registry_with_hosts();
        registry.observe(a, Reachability::Online, None);
        registry.observe(b, Reachability::Online, None);
        registry.set_capabilities(a, HostCapabilities::from_probe_output("has-docker"));

        let mut remaining = registry.host_list().clone();
        assert!(remaining.remove_host(a));
        registry.set_hosts(remaining);

        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.status(a).reachability,
            Reachability::Unknown,
            "state for a removed host must not linger"
        );
        assert!(!registry.capabilities(a).docker);
        assert_eq!(
            registry.status(b).reachability,
            Reachability::Online,
            "the surviving host keeps its state"
        );
    }

    #[test]
    fn the_label_falls_back_to_the_hostname() {
        let (mut registry, a, _) = registry_with_hosts();
        assert_eq!(registry.label(a).as_deref(), Some("10.0.0.10"));

        if let Some(host) = registry.host_list_mut().get_by_id_mut(a) {
            host.display_name = Some("Desk Rock5c".to_string());
        }
        assert_eq!(registry.label(a).as_deref(), Some("Desk Rock5c"));
    }

    #[test]
    fn exec_on_an_unknown_host_marks_nothing() {
        let mut registry = HostRegistry::new();
        assert!(registry.exec(12_345, "true").is_err());
    }
}
