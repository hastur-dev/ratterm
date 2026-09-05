//! Process-wide access to pooled SSH sessions.
//!
//! Discovery and metrics collection are written as free functions that run
//! from several places — the UI thread, the status checker, background
//! refreshes — and none of them carry application state. They used to reach
//! for a subprocess because that needs nothing but a command line.
//!
//! This gives them somewhere to reach instead: one [`SessionPool`] for the
//! process, plus the resolved [`RemoteTarget`] for each SSH host id, so a
//! caller that knows only a host id can run a command on it.
//!
//! Access is serialised behind a mutex. libssh2 sessions must not be driven
//! from two threads at once, and remote calls here are short (a `docker ps`, a
//! metrics sample), so a lock is the right shape. Anything long-lived — a log
//! stream, a forwarded port — takes its own connection instead of holding the
//! lock.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use thiserror::Error;

use super::forward::PortForward;
use super::pool::{PoolStats, SessionKey, SessionPool};
use super::session::{CommandOutput, HostKeyPolicy, RemoteTarget, SessionError};

/// Errors raised when running something on a host id.
#[derive(Debug, Error)]
pub enum ExecError {
    /// The host id is not in the registry.
    #[error("no SSH host with id {0}; open the SSH manager and check the host list")]
    UnknownHost(u32),

    /// The session or the command failed.
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// Pooled sessions plus the targets they are reached through.
pub struct RemoteExecutor {
    pool: SessionPool,
    targets: HashMap<u32, RemoteTarget>,
}

impl std::fmt::Debug for RemoteExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecutor")
            .field("known_hosts", &self.targets.len())
            .field("pool", &self.pool)
            .finish()
    }
}

impl Default for RemoteExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteExecutor {
    /// Creates an executor with an empty pool and no known hosts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pool: SessionPool::new(),
            targets: HashMap::new(),
        }
    }

    /// Replaces the known targets.
    ///
    /// Called whenever the SSH host list changes, so a host edited in the
    /// manager is reached at its new address rather than a stale copy. Any
    /// pooled session for a host whose target changed is closed, because it is
    /// connected to the old address.
    pub fn set_targets(&mut self, targets: HashMap<u32, RemoteTarget>) {
        let changed: Vec<u32> = self
            .targets
            .iter()
            .filter(|(id, old)| {
                targets.get(*id).is_none_or(|new| {
                    new.endpoint() != old.endpoint() || new.username != old.username
                })
            })
            .map(|(id, _)| *id)
            .collect();

        for id in changed {
            self.pool.close(&SessionKey::Host(id));
        }

        self.targets = targets;
    }

    /// Adds or replaces one target.
    pub fn set_target(&mut self, host_id: u32, target: RemoteTarget) {
        let replaced = self
            .targets
            .get(&host_id)
            .is_some_and(|old| old.endpoint() != target.endpoint());
        if replaced {
            self.pool.close(&SessionKey::Host(host_id));
        }
        self.targets.insert(host_id, target);
    }

    /// Returns the target for a host id.
    #[must_use]
    pub fn target(&self, host_id: u32) -> Option<&RemoteTarget> {
        self.targets.get(&host_id)
    }

    /// Returns how many hosts the executor can reach.
    #[must_use]
    pub fn known_hosts(&self) -> usize {
        self.targets.len()
    }

    /// Returns the host ids the executor knows about.
    pub fn host_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.targets.keys().copied()
    }

    /// Sets the host-key policy used for new connections.
    pub fn set_host_key_policy(&mut self, policy: HostKeyPolicy) {
        let pool = std::mem::take(&mut self.pool);
        self.pool = pool.with_host_key_policy(policy);
    }

    /// Runs a command on a host from the SSH host list.
    ///
    /// # Errors
    /// Returns [`ExecError::UnknownHost`] if the id is not known, otherwise
    /// whatever the session reported.
    pub fn exec(&mut self, host_id: u32, command: &str) -> Result<CommandOutput, ExecError> {
        let target = self
            .targets
            .get(&host_id)
            .ok_or(ExecError::UnknownHost(host_id))?
            .clone();
        Ok(self.pool.exec(&target, command)?)
    }

    /// Runs a command on an explicit target, pooling the session.
    ///
    /// # Errors
    /// Returns an error if the session cannot be opened or the command fails.
    pub fn exec_target(
        &mut self,
        target: &RemoteTarget,
        command: &str,
    ) -> Result<CommandOutput, SessionError> {
        self.pool.exec(target, command)
    }

    /// Forwards a loopback port to `remote_host:remote_port` on a host.
    ///
    /// The forward owns its own connection, so it keeps working while other
    /// callers use the pool.
    ///
    /// # Errors
    /// Returns an error if the host is unknown or the tunnel cannot be set up.
    pub fn forward(
        &mut self,
        host_id: u32,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<PortForward, ExecError> {
        let target = self
            .targets
            .get(&host_id)
            .ok_or(ExecError::UnknownHost(host_id))?
            .clone();
        let policy = self.pool.host_key_policy();
        let session = self.pool.get(&target)?;
        Ok(session.forward_local_port(remote_host, remote_port, policy)?)
    }

    /// Returns the pool counters.
    #[must_use]
    pub const fn pool_stats(&self) -> PoolStats {
        self.pool.stats()
    }

    /// Returns how many sessions are open.
    #[must_use]
    pub fn open_sessions(&self) -> usize {
        self.pool.len()
    }

    /// Closes sessions that have gone idle. Returns how many were closed.
    pub fn reap_idle(&mut self) -> usize {
        self.pool.reap_idle()
    }

    /// Closes every session.
    pub fn close_all(&mut self) {
        self.pool.close_all();
    }
}

/// The process-wide executor.
static SHARED: OnceLock<Mutex<RemoteExecutor>> = OnceLock::new();

/// Returns the process-wide executor.
fn shared() -> &'static Mutex<RemoteExecutor> {
    SHARED.get_or_init(|| Mutex::new(RemoteExecutor::new()))
}

/// Locks the shared executor, recovering from a poisoned mutex.
///
/// A panic while holding the lock must not make every remote feature
/// permanently unavailable: the pool's invariants do not survive a panic any
/// worse than a dropped session does, and reconnecting is the recovery path
/// anyway.
fn lock_shared() -> MutexGuard<'static, RemoteExecutor> {
    match shared().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("the remote executor lock was poisoned; recovering");
            let mut guard = poisoned.into_inner();
            guard.close_all();
            guard
        }
    }
}

/// Runs `f` with the shared executor.
pub fn with_shared<R>(f: impl FnOnce(&mut RemoteExecutor) -> R) -> R {
    let mut guard = lock_shared();
    f(&mut guard)
}

/// Runs a command on a host id using the shared executor.
///
/// # Errors
/// Returns an error if the host is unknown or the command fails.
pub fn exec_on_host(host_id: u32, command: &str) -> Result<CommandOutput, ExecError> {
    with_shared(|executor| executor.exec(host_id, command))
}

/// Publishes the resolved targets to the shared executor.
pub fn publish_targets(targets: HashMap<u32, RemoteTarget>) {
    with_shared(|executor| executor.set_targets(targets));
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn target(name: &str, id: u32) -> RemoteTarget {
        let mut t = RemoteTarget::new(name, "user");
        t.host_id = Some(id);
        t
    }

    /// A port nothing is listening on, so connecting fails immediately.
    fn dead_port() -> u16 {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
        let port = probe.local_addr().expect("addr").port();
        drop(probe);
        port
    }

    #[test]
    fn a_new_executor_knows_no_hosts() {
        let executor = RemoteExecutor::new();
        assert_eq!(executor.known_hosts(), 0);
        assert_eq!(executor.open_sessions(), 0);
        assert!(executor.target(1).is_none());
    }

    #[test]
    fn targets_can_be_published_and_read_back() {
        let mut executor = RemoteExecutor::new();
        let mut targets = HashMap::new();
        targets.insert(1, target("host-a", 1));
        targets.insert(2, target("host-b", 2));
        executor.set_targets(targets);

        assert_eq!(executor.known_hosts(), 2);
        assert_eq!(
            executor.target(1).map(|t| t.hostname.as_str()),
            Some("host-a")
        );
        let mut ids: Vec<u32> = executor.host_ids().collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn setting_one_target_replaces_it() {
        let mut executor = RemoteExecutor::new();
        executor.set_target(1, target("old", 1));
        executor.set_target(1, target("new", 1));
        assert_eq!(executor.target(1).map(|t| t.hostname.as_str()), Some("new"));
        assert_eq!(executor.known_hosts(), 1);
    }

    #[test]
    fn an_unknown_host_is_reported_by_id() {
        let mut executor = RemoteExecutor::new();
        match executor.exec(42, "true") {
            Err(ExecError::UnknownHost(id)) => assert_eq!(id, 42),
            other => panic!("expected UnknownHost, got {other:?}"),
        }
    }

    #[test]
    fn forwarding_from_an_unknown_host_is_reported() {
        let mut executor = RemoteExecutor::new();
        assert!(matches!(
            executor.forward(7, "127.0.0.1", 80),
            Err(ExecError::UnknownHost(7))
        ));
    }

    #[test]
    fn a_failed_command_reports_the_session_error() {
        let mut executor = RemoteExecutor::new();
        executor.set_host_key_policy(HostKeyPolicy::AcceptAny);
        executor.set_target(1, target("127.0.0.1", 1).with_port(dead_port()));

        match executor.exec(1, "true") {
            Err(ExecError::Session(_)) => {}
            other => panic!("expected a session error, got {other:?}"),
        }
        assert_eq!(executor.pool_stats().failures, 1);
    }

    #[test]
    fn republishing_targets_drops_sessions_for_changed_hosts() {
        // No session can be established in a unit test, so this checks the
        // bookkeeping: a target whose endpoint changed is treated as different.
        let mut executor = RemoteExecutor::new();
        executor.set_target(1, target("host-a", 1));

        let mut updated = HashMap::new();
        updated.insert(1, target("host-a", 1).with_port(2222));
        executor.set_targets(updated);

        assert_eq!(executor.target(1).map(|t| t.port), Some(2222));
    }

    #[test]
    fn removing_a_host_from_the_published_set_forgets_it() {
        let mut executor = RemoteExecutor::new();
        executor.set_target(1, target("host-a", 1));
        executor.set_targets(HashMap::new());
        assert_eq!(executor.known_hosts(), 0);
        assert!(executor.target(1).is_none());
    }

    #[test]
    fn reaping_and_closing_an_empty_executor_is_harmless() {
        let mut executor = RemoteExecutor::new();
        assert_eq!(executor.reap_idle(), 0);
        executor.close_all();
        assert_eq!(executor.open_sessions(), 0);
    }

    #[test]
    fn the_shared_executor_is_reachable_and_stable() {
        // Two calls see the same instance.
        with_shared(|executor| executor.set_target(9_999, target("shared-test", 9_999)));
        let seen = with_shared(|executor| executor.target(9_999).map(|t| t.hostname.clone()));
        assert_eq!(seen.as_deref(), Some("shared-test"));

        // Leave the shared state as we found it for other tests.
        with_shared(|executor| {
            let remaining: HashMap<u32, RemoteTarget> = executor
                .host_ids()
                .filter(|id| *id != 9_999)
                .filter_map(|id| executor.target(id).map(|t| (id, t.clone())))
                .collect();
            executor.set_targets(remaining);
        });
    }

    #[test]
    fn exec_on_host_reports_an_unknown_id() {
        assert!(matches!(
            exec_on_host(4_242_424, "true"),
            Err(ExecError::UnknownHost(_))
        ));
    }

    #[test]
    fn debug_output_summarises_the_executor() {
        let mut executor = RemoteExecutor::new();
        executor.set_target(1, target("host", 1));
        let rendered = format!("{executor:?}");
        assert!(rendered.contains("known_hosts: 1"), "{rendered}");
    }
}
