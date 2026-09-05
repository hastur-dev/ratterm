//! Local port forwarding over an SSH session.
//!
//! Equivalent to `ssh -L 127.0.0.1:<local>:<remote_host>:<remote_port>`,
//! implemented with a `direct-tcpip` channel instead of a child `ssh` process.
//!
//! Three things in the design are deliberate:
//!
//! - **One session per forward.** libssh2 sessions are not safe to use from
//!   several threads, so a forward owns its session outright rather than
//!   sharing one with the caller. The cost is an extra TCP connection and
//!   handshake per forward; the benefit is that no lock is held across a
//!   blocking read.
//! - **One pump thread, non-blocking I/O.** Every libssh2 call for a forward
//!   happens on that thread, so several client connections can be in flight at
//!   once without concurrent use of the session.
//! - **Loopback only.** The listener binds `127.0.0.1`, so a forward never
//!   exposes the remote service to the network.
//!
//! This is what lets the rest of the application talk to a remote Docker
//! daemon or Kubernetes API server with an ordinary TCP client, and what makes
//! `ProxyJump` work without shelling out.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use ssh2::Session;
use thiserror::Error;
use tracing::{debug, warn};

/// Size of the per-direction relay buffer.
const RELAY_BUFFER: usize = 32 * 1024;

/// How long the pump sleeps when every connection is idle.
const IDLE_SLEEP: Duration = Duration::from_millis(2);

/// Upper bound on pump iterations, so the loop is not unbounded.
///
/// At the idle sleep above this is several days of continuous operation; the
/// forward is closed long before it matters.
const MAX_PUMP_ITERATIONS: u64 = 1_000_000_000;

/// Maximum simultaneous client connections through one forward.
const MAX_CONNECTIONS: usize = 64;

/// How long [`PortForward::close`] waits for the pump thread.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the pump keeps retrying a channel open before giving up.
const CHANNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors raised while setting up a forward.
#[derive(Debug, Error)]
pub enum ForwardError {
    /// The local listener could not be bound.
    #[error("could not bind a local forwarding port: {0}")]
    Bind(#[from] std::io::Error),

    /// The pump thread could not be started.
    #[error("could not start the forwarding thread: {0}")]
    Thread(String),

    /// The remote endpoint description was unusable.
    #[error("invalid forward target: {0}")]
    InvalidTarget(String),
}

/// A running local port forward.
///
/// Dropping this closes the listener and tears the tunnel down.
pub struct PortForward {
    local_addr: SocketAddr,
    remote: String,
    shutdown: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for PortForward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortForward")
            .field("local", &self.local_addr)
            .field("remote", &self.remote)
            .field("running", &self.is_running())
            .finish()
    }
}

impl PortForward {
    /// Starts forwarding a fresh loopback port to `remote_host:remote_port`
    /// through `session`.
    ///
    /// The session is consumed: it belongs to the forward from here on.
    ///
    /// # Errors
    /// Returns an error if the local port cannot be bound or the thread cannot
    /// be spawned.
    pub fn start(
        session: Session,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<Self, ForwardError> {
        if remote_host.is_empty() {
            return Err(ForwardError::InvalidTarget(
                "remote host must not be empty".to_string(),
            ));
        }
        if remote_port == 0 {
            return Err(ForwardError::InvalidTarget(
                "remote port must not be zero".to_string(),
            ));
        }

        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let local_addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let remote = format!("{remote_host}:{remote_port}");

        let pump = Pump {
            session,
            listener,
            host: remote_host.to_string(),
            port: remote_port,
            shutdown: shutdown.clone(),
        };
        let finished_clone = finished.clone();

        let handle = std::thread::Builder::new()
            .name(format!("ratterm-forward-{local_addr}"))
            .spawn(move || {
                pump.run();
                finished_clone.store(true, Ordering::SeqCst);
            })
            .map_err(|e| ForwardError::Thread(e.to_string()))?;

        debug!("forwarding {} -> {}", local_addr, remote);

        Ok(Self {
            local_addr,
            remote,
            shutdown,
            finished,
            handle: Some(handle),
        })
    }

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

    /// Returns the remote endpoint, as `host:port`.
    #[must_use]
    pub fn remote(&self) -> &str {
        &self.remote
    }

    /// Returns true while the pump thread is alive.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.finished.load(Ordering::SeqCst)
    }

    /// Stops the forward and waits for the pump thread.
    ///
    /// The wait is generous because a loaded machine can leave the pump thread
    /// unscheduled for a while, and reporting a forward as still running when
    /// it has been told to stop is worse than waiting.
    pub fn close(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let deadline = std::time::Instant::now() + CLOSE_TIMEOUT;
            while std::time::Instant::now() < deadline && !self.finished.load(Ordering::SeqCst) {
                std::thread::sleep(IDLE_SLEEP);
            }
            if self.finished.load(Ordering::SeqCst) {
                let _ = handle.join();
            } else {
                warn!("forwarding thread for {} did not stop in time", self.remote);
            }
        }
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        self.close();
    }
}

/// Everything the pump thread owns.
struct Pump {
    session: Session,
    listener: TcpListener,
    host: String,
    port: u16,
    shutdown: Arc<AtomicBool>,
}

impl Pump {
    fn run(self) {
        // Every libssh2 call below happens on this thread, so non-blocking mode
        // is safe to set for the whole session.
        self.session.set_blocking(false);

        let mut pipes: Vec<Pipe> = Vec::new();

        for _ in 0..MAX_PUMP_ITERATIONS {
            if self.shutdown.load(Ordering::SeqCst) {
                break;
            }

            let mut worked = false;

            if pipes.len() < MAX_CONNECTIONS {
                match self.listener.accept() {
                    Ok((stream, peer)) => {
                        worked = true;
                        match self.open_pipe(&stream) {
                            Ok(pipe) => pipes.push(pipe),
                            Err(e) => warn!("forward from {peer} rejected: {e}"),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        warn!("forward listener stopped: {e}");
                        break;
                    }
                }
            }

            pipes.retain_mut(|pipe| pipe.pump(&mut worked));

            if !worked {
                std::thread::sleep(IDLE_SLEEP);
            }
        }

        for pipe in &mut pipes {
            pipe.shutdown();
        }
        debug!("forward to {}:{} stopped", self.host, self.port);
    }

    /// Opens the SSH channel that backs one client connection.
    ///
    /// Takes the stream by reference and clones it on success, so a failed
    /// open closes the client connection rather than leaking it.
    fn open_pipe(&self, stream: &TcpStream) -> Result<Pipe, String> {
        stream.set_nonblocking(true).map_err(|e| e.to_string())?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;

        // In non-blocking mode the channel open may need several attempts.
        // The shutdown flag is checked on every one: without that, closing a
        // forward while a client is connecting would wait out the whole
        // timeout.
        let deadline = std::time::Instant::now() + CHANNEL_OPEN_TIMEOUT;
        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                return Err("forward is shutting down".to_string());
            }

            match self
                .session
                .channel_direct_tcpip(&self.host, self.port, None)
            {
                Ok(channel) => {
                    let owned = stream.try_clone().map_err(|e| e.to_string())?;
                    return Ok(Pipe::new(owned, channel));
                }
                Err(e) if is_ssh_would_block(&e) => {
                    if std::time::Instant::now() >= deadline {
                        return Err("timed out opening a forwarding channel".to_string());
                    }
                    std::thread::sleep(IDLE_SLEEP);
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

/// Returns true if an ssh2 error means "try again", not "failed".
fn is_ssh_would_block(err: &ssh2::Error) -> bool {
    // -37 is LIBSSH2_ERROR_EAGAIN.
    matches!(err.code(), ssh2::ErrorCode::Session(-37))
}

/// One client connection and the SSH channel it is bound to.
struct Pipe {
    tcp: TcpStream,
    channel: ssh2::Channel,
    /// Bytes read from the client, waiting to go to the channel.
    to_remote: VecDeque<u8>,
    /// Bytes read from the channel, waiting to go to the client.
    to_local: VecDeque<u8>,
    /// The client has closed its side.
    local_eof: bool,
    /// The remote has closed its side.
    remote_eof: bool,
}

impl Pipe {
    fn new(tcp: TcpStream, channel: ssh2::Channel) -> Self {
        Self {
            tcp,
            channel,
            to_remote: VecDeque::new(),
            to_local: VecDeque::new(),
            local_eof: false,
            remote_eof: false,
        }
    }

    /// Moves whatever bytes are available in both directions.
    ///
    /// Returns false once the connection is finished and should be dropped.
    /// Sets `worked` when it moved anything, so the caller knows not to sleep.
    fn pump(&mut self, worked: &mut bool) -> bool {
        let mut buf = [0u8; RELAY_BUFFER];

        // Client -> remote.
        if !self.local_eof && self.to_remote.len() < RELAY_BUFFER {
            match self.tcp.read(&mut buf) {
                Ok(0) => self.local_eof = true,
                Ok(n) => {
                    self.to_remote.extend(&buf[..n]);
                    *worked = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.local_eof = true,
            }
        }

        if !self.to_remote.is_empty() {
            let (front, _) = self.to_remote.as_slices();
            match self.channel.write(front) {
                Ok(0) => {}
                Ok(n) => {
                    self.to_remote.drain(..n);
                    *worked = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.remote_eof = true,
            }
        } else if self.local_eof && !self.remote_eof {
            // Tell the remote the client is done writing so it can finish.
            let _ = self.channel.send_eof();
        }

        // Remote -> client.
        if !self.remote_eof && self.to_local.len() < RELAY_BUFFER {
            match self.channel.read(&mut buf) {
                Ok(0) => {
                    if self.channel.eof() {
                        self.remote_eof = true;
                    }
                }
                Ok(n) => {
                    self.to_local.extend(&buf[..n]);
                    *worked = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.remote_eof = true,
            }
        }

        if !self.to_local.is_empty() {
            let (front, _) = self.to_local.as_slices();
            match self.tcp.write(front) {
                Ok(0) => {}
                Ok(n) => {
                    self.to_local.drain(..n);
                    *worked = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.local_eof = true,
            }
        }

        let drained = self.to_local.is_empty() && self.to_remote.is_empty();
        let finished = self.local_eof && self.remote_eof && drained;
        if finished {
            self.shutdown();
        }
        !finished
    }

    fn shutdown(&mut self) {
        let _ = self.channel.send_eof();
        let _ = self.channel.close();
        let _ = self.tcp.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn eagain_is_recognised_as_retryable() {
        let err = ssh2::Error::new(ssh2::ErrorCode::Session(-37), "again");
        assert!(is_ssh_would_block(&err));
    }

    #[test]
    fn other_ssh_errors_are_not_retryable() {
        let err = ssh2::Error::new(ssh2::ErrorCode::Session(-1), "boom");
        assert!(!is_ssh_would_block(&err));
        let err = ssh2::Error::new(ssh2::ErrorCode::SFTP(2), "no such file");
        assert!(!is_ssh_would_block(&err));
    }

    #[test]
    fn an_empty_remote_host_is_refused() {
        let session = Session::new().expect("session");
        match PortForward::start(session, "", 22) {
            Err(ForwardError::InvalidTarget(msg)) => assert!(msg.contains("host"), "{msg}"),
            other => panic!("expected InvalidTarget, got {other:?}"),
        }
    }

    #[test]
    fn a_zero_remote_port_is_refused() {
        let session = Session::new().expect("session");
        match PortForward::start(session, "example.invalid", 0) {
            Err(ForwardError::InvalidTarget(msg)) => assert!(msg.contains("port"), "{msg}"),
            other => panic!("expected InvalidTarget, got {other:?}"),
        }
    }

    #[test]
    fn a_forward_binds_a_loopback_port_and_stops_cleanly() {
        // The session has no transport, so no channel can ever open; this
        // exercises the listener, the thread lifecycle and shutdown, which is
        // what can be tested without a real SSH server.
        let session = Session::new().expect("session");
        let mut forward = PortForward::start(session, "example.invalid", 2222).expect("start");

        assert!(forward.local_addr().ip().is_loopback());
        assert_ne!(forward.local_port(), 0);
        assert_eq!(forward.remote(), "example.invalid:2222");
        assert!(forward.is_running());

        forward.close();
        assert!(!forward.is_running());
    }

    #[test]
    fn closing_twice_is_harmless() {
        let session = Session::new().expect("session");
        let mut forward = PortForward::start(session, "example.invalid", 2222).expect("start");
        forward.close();
        forward.close();
        assert!(!forward.is_running());
    }

    #[test]
    fn dropping_a_forward_releases_the_port() {
        let session = Session::new().expect("session");
        let port = {
            let forward = PortForward::start(session, "example.invalid", 2222).expect("start");
            forward.local_port()
        };

        // Binding the same port again proves the listener was released.
        let rebound = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
        assert!(rebound.is_ok(), "port {port} was not released");
    }

    #[test]
    fn a_client_can_connect_to_the_local_side() {
        let session = Session::new().expect("session");
        let forward = PortForward::start(session, "example.invalid", 2222).expect("start");

        // The forward accepts the TCP connection even though the tunnel behind
        // it cannot open; the client sees a connection that then closes.
        let stream = TcpStream::connect(forward.local_addr());
        assert!(stream.is_ok(), "local side must accept connections");
    }

    #[test]
    fn debug_output_names_both_ends() {
        let session = Session::new().expect("session");
        let forward = PortForward::start(session, "example.invalid", 2222).expect("start");
        let rendered = format!("{forward:?}");
        assert!(rendered.contains("example.invalid:2222"), "{rendered}");
        assert!(rendered.contains("127.0.0.1"), "{rendered}");
    }
}
