//! Lightweight background status checker for SSH hosts.
//!
//! Runs TCP reachability checks on saved SSH hosts via background threads,
//! reporting results through an `mpsc` channel.  This mirrors the pattern
//! used by [`super::scanner::NetworkScanner`] and
//! [`super::collector::MetricsCollector`].

use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing::{debug, info};

use super::host::ConnectionStatus;

/// TCP connect timeout for status checks (same as `NetworkScanner`).
const CONNECT_TIMEOUT_MS: u64 = 1500;

/// Maximum hosts to check in a single batch.
const MAX_STATUS_HOSTS: usize = 100;

/// Descriptor for one host to check.
#[derive(Debug, Clone)]
pub struct StatusTarget {
    /// Host ID (matches `SSHHost::id`).
    pub host_id: u32,
    /// Hostname or IP address.
    pub hostname: String,
    /// SSH port.
    pub port: u16,
}

impl StatusTarget {
    /// Creates a new status target.
    #[must_use]
    pub fn new(host_id: u32, hostname: String, port: u16) -> Self {
        assert!(!hostname.is_empty(), "hostname must not be empty");
        assert!(port > 0, "port must be positive");

        Self {
            host_id,
            hostname,
            port,
        }
    }
}

/// Background TCP-reachability checker for SSH hosts.
///
/// Spawns one thread per host, each performing a single
/// `TcpStream::connect_timeout` call.  Results are delivered through an
/// `mpsc` channel and consumed by [`Self::poll_results`].
pub struct StatusChecker {
    /// Receiver end (main thread only).
    results_rx: mpsc::Receiver<(u32, ConnectionStatus)>,
    /// Cancel flag shared with worker threads.
    running: Arc<AtomicBool>,
    /// Worker thread handles.
    handles: Vec<JoinHandle<()>>,
}

impl StatusChecker {
    /// Starts background status checks for the given hosts.
    ///
    /// Each host gets its own thread — the thread does one TCP connect
    /// and then exits, so the pool is bounded by `targets.len()`.
    #[must_use]
    pub fn new(targets: Vec<StatusTarget>) -> Self {
        assert!(
            targets.len() <= MAX_STATUS_HOSTS,
            "Too many status targets (max {})",
            MAX_STATUS_HOSTS
        );

        let (tx, rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let mut handles = Vec::with_capacity(targets.len());

        info!("StatusChecker: starting checks for {} hosts", targets.len());

        for target in targets {
            let tx = tx.clone();
            let running = Arc::clone(&running);

            let handle = thread::spawn(move || {
                if !running.load(Ordering::Relaxed) {
                    return;
                }

                let status = check_tcp_reachable(&target.hostname, target.port);
                debug!("StatusChecker: {} → {:?}", target.hostname, status.as_str());

                let _ = tx.send((target.host_id, status));
            });

            handles.push(handle);
        }

        Self {
            results_rx: rx,
            running,
            handles,
        }
    }

    /// Drains completed results from background threads (non-blocking).
    ///
    /// Returns a `Vec` of `(host_id, ConnectionStatus)` that arrived
    /// since the last call.  Safe to call every tick.
    #[must_use]
    pub fn poll_results(&self) -> Vec<(u32, ConnectionStatus)> {
        let mut results = Vec::new();

        // Drain up to 50 per tick to stay bounded.
        for _ in 0..50 {
            match self.results_rx.try_recv() {
                Ok(pair) => results.push(pair),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }

        results
    }

    /// Returns `true` when all worker threads have finished.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.handles.iter().all(JoinHandle::is_finished)
    }

    /// Signals all workers to stop (best-effort; threads that already
    /// began a TCP connect will complete naturally).
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for StatusChecker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Performs a single TCP connect to `hostname:port` with a timeout.
///
/// Returns [`ConnectionStatus::Reachable`] on success,
/// [`ConnectionStatus::Unreachable`] on failure.
fn check_tcp_reachable(hostname: &str, port: u16) -> ConnectionStatus {
    assert!(!hostname.is_empty(), "hostname must not be empty");
    assert!(port > 0, "port must be positive");

    let addr_str = format!("{hostname}:{port}");
    let addr: SocketAddr = match addr_str.parse() {
        Ok(a) => a,
        Err(_) => {
            // Hostname might need DNS resolution — try via ToSocketAddrs
            use std::net::ToSocketAddrs;
            match addr_str.to_socket_addrs() {
                Ok(mut addrs) => match addrs.next() {
                    Some(a) => a,
                    None => return ConnectionStatus::Unreachable,
                },
                Err(_) => return ConnectionStatus::Unreachable,
            }
        }
    };

    let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);
    if TcpStream::connect_timeout(&addr, timeout).is_ok() {
        ConnectionStatus::Reachable
    } else {
        ConnectionStatus::Unreachable
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── StatusTarget ────────────────────────────────────────────────

    #[test]
    fn test_status_target_creation() {
        let t = StatusTarget::new(1, "192.168.1.1".to_string(), 22);
        assert_eq!(t.host_id, 1);
        assert_eq!(t.hostname, "192.168.1.1");
        assert_eq!(t.port, 22);
    }

    #[test]
    #[should_panic(expected = "hostname must not be empty")]
    fn test_status_target_empty_hostname_panics() {
        let _ = StatusTarget::new(1, String::new(), 22);
    }

    #[test]
    #[should_panic(expected = "port must be positive")]
    fn test_status_target_zero_port_panics() {
        let _ = StatusTarget::new(1, "host".to_string(), 0);
    }

    // ── StatusChecker construction ──────────────────────────────────

    #[test]
    fn test_new_creates_checker_with_empty_list() {
        let checker = StatusChecker::new(Vec::new());
        assert!(checker.is_complete());
        assert!(checker.poll_results().is_empty());
    }

    #[test]
    fn test_poll_empty_returns_nothing() {
        let checker = StatusChecker::new(Vec::new());

        // Multiple polls should all be empty.
        assert!(checker.poll_results().is_empty());
        assert!(checker.poll_results().is_empty());
    }

    // ── Reachability ────────────────────────────────────────────────

    #[test]
    fn test_unreachable_host() {
        // 192.0.2.1 is in the TEST-NET range — guaranteed non-routable.
        let targets = vec![StatusTarget::new(42, "192.0.2.1".to_string(), 22)];
        let checker = StatusChecker::new(targets);

        // Wait for the thread to finish (should be ~1.5s timeout).
        let start = std::time::Instant::now();
        while !checker.is_complete() {
            std::thread::sleep(Duration::from_millis(50));
            assert!(
                start.elapsed().as_secs() < 10,
                "checker timed out waiting for thread"
            );
        }

        let results = checker.poll_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 42);
        assert_eq!(results[0].1, ConnectionStatus::Unreachable);
    }

    #[test]
    fn test_stop_sets_cancel_flag() {
        let checker = StatusChecker::new(Vec::new());
        assert!(checker.running.load(Ordering::Relaxed));

        checker.stop();
        assert!(!checker.running.load(Ordering::Relaxed));
    }

    // ── check_tcp_reachable (unit) ──────────────────────────────────

    #[test]
    fn test_check_tcp_unreachable_ip() {
        let status = check_tcp_reachable("192.0.2.1", 22);
        assert_eq!(status, ConnectionStatus::Unreachable);
    }

    #[test]
    fn test_check_tcp_invalid_hostname() {
        let status = check_tcp_reachable("this.host.does.not.exist.example.invalid", 22);
        assert_eq!(status, ConnectionStatus::Unreachable);
    }
}
