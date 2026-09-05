//! Pool of persistent SSH sessions, one per host.
//!
//! Before this, every remote action — a Docker `ps`, a metrics sample, an SFTP
//! read — paid for its own TCP connection and authentication handshake, and on
//! Windows also for spawning `plink.exe`, which is what corrupted console
//! input badly enough to need a documented workaround.
//!
//! The pool keeps one authenticated [`RemoteSession`] per host id and hands it
//! out on demand. A session that has gone quiet past the idle timeout, or that
//! stops answering keepalives, is dropped and replaced on the next request, so
//! a laptop that slept overnight recovers by itself.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tracing::{debug, info};

use super::session::{HostKeyPolicy, RemoteSession, RemoteTarget, SessionError};

/// How long an unused session is kept before it is closed.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Largest number of sessions held at once.
///
/// A fleet larger than this still works; the least recently used session is
/// evicted to make room.
pub const DEFAULT_MAX_SESSIONS: usize = 32;

/// Counters describing what the pool has been doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// Requests served by an already-open session.
    pub hits: u64,
    /// Requests that had to open a session.
    pub misses: u64,
    /// Sessions dropped because they went idle.
    pub idle_evictions: u64,
    /// Sessions dropped because they stopped answering.
    pub dead_evictions: u64,
    /// Sessions dropped to stay under the size limit.
    pub capacity_evictions: u64,
    /// Connection attempts that failed.
    pub failures: u64,
}

impl PoolStats {
    /// Returns the fraction of requests served without reconnecting.
    ///
    /// Returns `None` before the first request.
    #[must_use]
    pub fn hit_ratio(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        if total == 0 {
            None
        } else {
            #[allow(clippy::cast_precision_loss)]
            Some(self.hits as f64 / total as f64)
        }
    }
}

/// A key identifying one pooled session.
///
/// Host id where the host came from the SSH host list, otherwise the endpoint
/// and user, so ad-hoc targets still get pooled.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SessionKey {
    /// A host from the SSH host list.
    Host(u32),
    /// An ad-hoc `user@host:port`.
    Endpoint(String),
}

impl SessionKey {
    /// Derives the key for a target.
    #[must_use]
    pub fn of(target: &RemoteTarget) -> Self {
        match target.host_id {
            Some(id) => Self::Host(id),
            None => Self::Endpoint(format!(
                "{}@{}:{}",
                target.username, target.hostname, target.port
            )),
        }
    }
}

/// Sessions kept open, keyed by host.
pub struct SessionPool {
    sessions: HashMap<SessionKey, RemoteSession>,
    idle_timeout: Duration,
    max_sessions: usize,
    policy: HostKeyPolicy,
    stats: PoolStats,
}

impl std::fmt::Debug for SessionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionPool")
            .field("open", &self.sessions.len())
            .field("idle_timeout", &self.idle_timeout)
            .field("max_sessions", &self.max_sessions)
            .field("policy", &self.policy)
            .field("stats", &self.stats)
            .finish()
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionPool {
    /// Creates a pool with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_sessions: DEFAULT_MAX_SESSIONS,
            policy: HostKeyPolicy::default(),
            stats: PoolStats::default(),
        }
    }

    /// Sets how long an unused session is kept.
    #[must_use]
    pub const fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Sets the largest number of sessions held at once.
    ///
    /// Values below one are raised to one; a pool that can hold nothing would
    /// reconnect on every call.
    #[must_use]
    pub const fn with_max_sessions(mut self, max: usize) -> Self {
        self.max_sessions = if max == 0 { 1 } else { max };
        self
    }

    /// Sets the host-key policy used for new connections.
    #[must_use]
    pub const fn with_host_key_policy(mut self, policy: HostKeyPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Returns the host-key policy.
    #[must_use]
    pub const fn host_key_policy(&self) -> HostKeyPolicy {
        self.policy
    }

    /// Returns the idle timeout.
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Returns the counters.
    #[must_use]
    pub const fn stats(&self) -> PoolStats {
        self.stats
    }

    /// Returns how many sessions are currently open.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Returns true if no sessions are open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Returns true if a session for `key` is currently open.
    #[must_use]
    pub fn contains(&self, key: &SessionKey) -> bool {
        self.sessions.contains_key(key)
    }

    /// Returns a session for `target`, opening one if needed.
    ///
    /// # Errors
    /// Returns an error if a new session cannot be opened.
    pub fn get(&mut self, target: &RemoteTarget) -> Result<&mut RemoteSession, SessionError> {
        let key = SessionKey::of(target);

        // Drop a session that has gone quiet or stopped answering before
        // deciding whether this is a hit.
        if let Some(session) = self.sessions.get_mut(&key) {
            let stale = session.idle_for() > self.idle_timeout;
            let dead = !session.is_alive();
            if stale || dead {
                self.sessions.remove(&key);
                if stale {
                    self.stats.idle_evictions += 1;
                } else {
                    self.stats.dead_evictions += 1;
                }
                debug!("dropped a stale session for {}", target.endpoint());
            }
        }

        if self.sessions.contains_key(&key) {
            self.stats.hits += 1;
        } else {
            self.evict_for_capacity();
            match RemoteSession::connect(target, self.policy) {
                Ok(session) => {
                    info!("opened an SSH session to {}", target.endpoint());
                    self.sessions.insert(key.clone(), session);
                    self.stats.misses += 1;
                }
                Err(e) => {
                    self.stats.failures += 1;
                    return Err(e);
                }
            }
        }

        let session = self
            .sessions
            .get_mut(&key)
            .ok_or_else(|| SessionError::Command("session vanished from the pool".to_string()))?;
        session.touch();
        Ok(session)
    }

    /// Runs a command on `target`, reusing a pooled session.
    ///
    /// # Errors
    /// Returns an error if the session cannot be opened or the command cannot
    /// be run.
    pub fn exec(
        &mut self,
        target: &RemoteTarget,
        command: &str,
    ) -> Result<super::session::CommandOutput, SessionError> {
        self.get(target)?.exec(command)
    }

    /// Closes the session for `key`, if any.
    ///
    /// Returns true if a session was closed.
    pub fn close(&mut self, key: &SessionKey) -> bool {
        self.sessions.remove(key).is_some()
    }

    /// Closes every session.
    pub fn close_all(&mut self) {
        self.sessions.clear();
    }

    /// Drops sessions that have been unused past the idle timeout.
    ///
    /// Returns how many were closed. Call from the application's tick so idle
    /// connections do not sit open on the remote side.
    pub fn reap_idle(&mut self) -> usize {
        let timeout = self.idle_timeout;
        let before = self.sessions.len();
        self.sessions
            .retain(|_, session| session.idle_for() <= timeout);
        let closed = before - self.sessions.len();
        #[allow(clippy::cast_possible_truncation)]
        {
            self.stats.idle_evictions += closed as u64;
        }
        closed
    }

    /// Makes room for one more session.
    fn evict_for_capacity(&mut self) {
        if self.sessions.len() < self.max_sessions {
            return;
        }

        let victim = self
            .sessions
            .iter()
            .max_by_key(|(_, session)| session.idle_for())
            .map(|(key, _)| key.clone());

        if let Some(key) = victim {
            debug!("evicting the least recently used session to stay under the limit");
            self.sessions.remove(&key);
            self.stats.capacity_evictions += 1;
        }
    }

    /// Returns the age of the oldest open session.
    #[must_use]
    pub fn oldest_age(&self) -> Option<Duration> {
        self.sessions.values().map(RemoteSession::age).max()
    }

    /// Returns when the pool last did anything, for the status bar.
    #[must_use]
    pub fn most_recent_use(&self) -> Option<Instant> {
        self.sessions
            .values()
            .map(|s| Instant::now() - s.idle_for())
            .max()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn target(name: &str) -> RemoteTarget {
        RemoteTarget::new(name, "user")
    }

    /// A port nothing is listening on, so `connect` fails quickly.
    fn dead_port() -> u16 {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
        let port = probe.local_addr().expect("addr").port();
        drop(probe);
        port
    }

    #[test]
    fn a_new_pool_is_empty_with_default_limits() {
        let pool = SessionPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.idle_timeout(), DEFAULT_IDLE_TIMEOUT);
        assert_eq!(pool.host_key_policy(), HostKeyPolicy::AcceptNew);
        assert_eq!(pool.stats(), PoolStats::default());
    }

    #[test]
    fn builders_set_the_limits() {
        let pool = SessionPool::new()
            .with_idle_timeout(Duration::from_secs(7))
            .with_max_sessions(3)
            .with_host_key_policy(HostKeyPolicy::Strict);
        assert_eq!(pool.idle_timeout(), Duration::from_secs(7));
        assert_eq!(pool.max_sessions, 3);
        assert_eq!(pool.host_key_policy(), HostKeyPolicy::Strict);
    }

    #[test]
    fn a_zero_capacity_pool_is_raised_to_one() {
        let pool = SessionPool::new().with_max_sessions(0);
        assert_eq!(pool.max_sessions, 1);
    }

    #[test]
    fn keys_use_the_host_id_when_there_is_one() {
        let mut t = target("host");
        t.host_id = Some(5);
        assert_eq!(SessionKey::of(&t), SessionKey::Host(5));
    }

    #[test]
    fn keys_fall_back_to_the_endpoint() {
        let t = target("host").with_port(2222);
        assert_eq!(
            SessionKey::of(&t),
            SessionKey::Endpoint("user@host:2222".to_string())
        );
    }

    #[test]
    fn keys_distinguish_users_ports_and_hosts() {
        let base = target("host");
        let other_user = RemoteTarget::new("host", "other");
        let other_port = target("host").with_port(2222);
        let other_host = target("elsewhere");

        assert_ne!(SessionKey::of(&base), SessionKey::of(&other_user));
        assert_ne!(SessionKey::of(&base), SessionKey::of(&other_port));
        assert_ne!(SessionKey::of(&base), SessionKey::of(&other_host));
    }

    #[test]
    fn two_targets_for_the_same_host_id_share_a_key() {
        let mut a = target("host-a");
        let mut b = target("host-b");
        a.host_id = Some(9);
        b.host_id = Some(9);
        assert_eq!(SessionKey::of(&a), SessionKey::of(&b));
    }

    #[test]
    fn a_failed_connection_is_counted_and_opens_nothing() {
        let mut pool = SessionPool::new().with_host_key_policy(HostKeyPolicy::AcceptAny);
        let t = target("127.0.0.1").with_port(dead_port());

        assert!(pool.get(&t).is_err());
        assert!(pool.is_empty());
        assert_eq!(pool.stats().failures, 1);
        assert_eq!(pool.stats().hits, 0);
        assert_eq!(pool.stats().misses, 0);
    }

    #[test]
    fn repeated_failures_keep_counting() {
        let mut pool = SessionPool::new().with_host_key_policy(HostKeyPolicy::AcceptAny);
        let t = target("127.0.0.1").with_port(dead_port());
        for _ in 0..3 {
            assert!(pool.get(&t).is_err());
        }
        assert_eq!(pool.stats().failures, 3);
    }

    #[test]
    fn closing_an_absent_session_reports_false() {
        let mut pool = SessionPool::new();
        assert!(!pool.close(&SessionKey::Host(1)));
    }

    #[test]
    fn close_all_empties_the_pool() {
        let mut pool = SessionPool::new();
        pool.close_all();
        assert!(pool.is_empty());
    }

    #[test]
    fn reaping_an_empty_pool_closes_nothing() {
        let mut pool = SessionPool::new();
        assert_eq!(pool.reap_idle(), 0);
    }

    #[test]
    fn an_empty_pool_has_no_oldest_session() {
        let pool = SessionPool::new();
        assert!(pool.oldest_age().is_none());
        assert!(pool.most_recent_use().is_none());
    }

    #[test]
    fn the_hit_ratio_is_unknown_before_the_first_request() {
        assert!(PoolStats::default().hit_ratio().is_none());
    }

    #[test]
    fn the_hit_ratio_counts_only_hits_and_misses() {
        let stats = PoolStats {
            hits: 3,
            misses: 1,
            idle_evictions: 7,
            ..PoolStats::default()
        };
        let ratio = stats.hit_ratio().expect("a ratio");
        assert!((ratio - 0.75).abs() < f64::EPSILON, "{ratio}");
    }

    #[test]
    fn debug_output_summarises_the_pool() {
        let pool = SessionPool::new();
        let rendered = format!("{pool:?}");
        assert!(rendered.contains("open: 0"), "{rendered}");
        assert!(rendered.contains("AcceptNew"), "{rendered}");
    }
}
