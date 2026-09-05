//! Loopback TCP transport.
//!
//! The third transport, added because the other two do not cross machines
//! well: forwarding a remote Unix socket with `ssh -L /local.sock:/remote.sock`
//! depends on the OpenSSH version at both ends and is unreliable from a
//! Windows client, and a named pipe does not forward at all. A loopback TCP
//! port forwards with plain `ssh -N -L PORT:127.0.0.1:PORT`, which works
//! everywhere.
//!
//! Two rules make that safe:
//!
//! - The listener refuses to bind anything but a loopback address, so the
//!   endpoint is never reachable from the network.
//! - Authentication is mandatory. A loopback port is visible to every local
//!   process, so unlike a 0600 socket it cannot rely on filesystem
//!   permissions.

use std::io::{BufReader, BufWriter};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

use tracing::{debug, error, info};

use crate::api::ApiError;
use crate::api::transport::{BufferedConnection, Connection};

/// Default loopback port when only `--api-tcp` is given with no address.
pub const DEFAULT_TCP_PORT: u16 = 47_113;

/// How long a read waits before returning so the loop can re-check shutdown.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(200);

/// Returns the default loopback address.
#[must_use]
pub fn default_tcp_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_TCP_PORT))
}

/// Loopback TCP server.
pub struct TcpServer {
    listener: TcpListener,
    addr: SocketAddr,
}

impl TcpServer {
    /// Binds a loopback listener.
    ///
    /// # Errors
    /// Returns an error if `addr` is not a loopback address, or if the port
    /// cannot be bound.
    pub fn new(addr: SocketAddr) -> Result<Self, ApiError> {
        if !addr.ip().is_loopback() {
            return Err(ApiError::Protocol(format!(
                "refusing to bind the control API to {addr}: only loopback addresses are allowed"
            )));
        }

        let listener = TcpListener::bind(addr)?;
        let bound = listener.local_addr()?;
        // Non-blocking accept so the server thread can notice a shutdown
        // request instead of sitting in `accept` until someone connects.
        listener.set_nonblocking(true)?;
        info!("API server listening on tcp://{}", bound);

        Ok(Self {
            listener,
            addr: bound,
        })
    }

    /// Returns the bound address, with the real port when 0 was requested.
    #[must_use]
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Accepts a connection if one is waiting.
    ///
    /// Returns `Ok(None)` when nothing is pending, so the caller can check for
    /// shutdown between attempts.
    ///
    /// # Errors
    /// Returns an error if accepting fails for a reason other than "would
    /// block".
    pub fn accept(&self) -> Result<Option<TcpConnection>, ApiError> {
        match self.listener.accept() {
            Ok((stream, peer)) => {
                debug!("API client connected from {}", peer);
                // A peer that is not loopback cannot normally reach a loopback
                // listener, but check rather than assume.
                if !peer.ip().is_loopback() {
                    return Err(ApiError::Protocol(format!(
                        "rejected non-loopback client {peer}"
                    )));
                }
                TcpConnection::new(stream).map(Some)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => {
                error!("Failed to accept TCP connection: {}", e);
                Err(ApiError::Transport(e))
            }
        }
    }

    /// Blocks until a client connects, for callers that want the old shape.
    ///
    /// # Errors
    /// Returns an error if accepting fails.
    pub fn accept_blocking(&self) -> Result<TcpConnection, ApiError> {
        loop {
            if let Some(conn) = self.accept()? {
                return Ok(conn);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// A single accepted TCP connection.
pub struct TcpConnection {
    inner: BufferedConnection<BufReader<TcpStream>, BufWriter<TcpStream>>,
}

impl TcpConnection {
    /// Wraps a stream.
    ///
    /// # Errors
    /// Returns an error if the stream cannot be cloned.
    pub fn new(stream: TcpStream) -> Result<Self, ApiError> {
        stream.set_nodelay(true).map_err(ApiError::Transport)?;
        // A stream accepted from a non-blocking listener must not inherit that
        // mode: the message loop expects a blocking read.
        stream.set_nonblocking(false).map_err(ApiError::Transport)?;
        // Time the read out periodically so an idle client cannot keep the
        // server thread from noticing a shutdown request. Partial lines are
        // preserved by `BufferedConnection`.
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .map_err(ApiError::Transport)?;
        let read_stream = stream.try_clone().map_err(ApiError::Transport)?;
        Ok(Self {
            inner: BufferedConnection::new(BufReader::new(read_stream), BufWriter::new(stream)),
        })
    }
}

impl Connection for TcpConnection {
    fn read_message(&mut self) -> Result<Option<String>, ApiError> {
        self.inner.read_message()
    }

    fn write_message(&mut self, msg: &str) -> Result<(), ApiError> {
        self.inner.write_message(msg)
    }

    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

/// Client side, used by tests and by the MCP bridge.
pub struct TcpClient;

impl TcpClient {
    /// Connects to a loopback control endpoint.
    ///
    /// # Errors
    /// Returns an error if the connection fails.
    pub fn connect(addr: SocketAddr) -> Result<TcpConnection, ApiError> {
        let stream = TcpStream::connect(addr)?;
        TcpConnection::new(stream)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    fn ephemeral() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
    }

    #[test]
    fn the_default_address_is_loopback() {
        assert!(default_tcp_addr().ip().is_loopback());
        assert_eq!(default_tcp_addr().port(), DEFAULT_TCP_PORT);
    }

    #[test]
    fn binding_a_non_loopback_address_is_refused() {
        let addr = SocketAddr::from(([0, 0, 0, 0], 0));
        match TcpServer::new(addr) {
            Err(ApiError::Protocol(msg)) => assert!(msg.contains("loopback"), "{msg}"),
            other => panic!(
                "expected a protocol error, got {other:?}",
                other = other.err()
            ),
        }
    }

    #[test]
    fn ipv6_loopback_is_accepted() {
        let addr = SocketAddr::from((Ipv6Addr::LOCALHOST, 0));
        // Some CI images have IPv6 disabled; a bind failure there is not a
        // policy failure, which is what this test is about.
        if let Ok(server) = TcpServer::new(addr) {
            assert!(server.addr().ip().is_loopback());
        }
    }

    #[test]
    fn an_ephemeral_port_is_reported_after_binding() {
        let server = TcpServer::new(ephemeral()).expect("bind");
        assert_ne!(server.addr().port(), 0);
        assert!(server.addr().ip().is_loopback());
    }

    /// Reads with a deadline, since the transport times reads out on purpose.
    fn read_with_deadline(conn: &mut TcpConnection) -> Option<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match conn.read_message().expect("read") {
                Some(msg) => return Some(msg),
                None if conn.is_open() => std::thread::sleep(std::time::Duration::from_millis(10)),
                None => return None,
            }
        }
        None
    }

    #[test]
    fn a_message_round_trips_over_the_socket() {
        let server = TcpServer::new(ephemeral()).expect("bind");
        let addr = server.addr();

        let client = std::thread::spawn(move || {
            let mut conn = TcpClient::connect(addr).expect("connect");
            conn.write_message(r#"{"id":"1","method":"system.ping"}"#)
                .expect("write");
            read_with_deadline(&mut conn)
        });

        let mut conn = server.accept_blocking().expect("accept");
        let msg = read_with_deadline(&mut conn).expect("a message");
        assert!(msg.contains("system.ping"));
        conn.write_message(r#"{"id":"1","result":{"pong":true}}"#)
            .expect("write");

        let response = client.join().expect("client thread").expect("a response");
        assert!(response.contains("pong"));
    }

    #[test]
    fn a_message_split_across_reads_is_reassembled() {
        let server = TcpServer::new(ephemeral()).expect("bind");
        let addr = server.addr();

        let client = std::thread::spawn(move || {
            use std::io::Write as _;
            let mut stream = std::net::TcpStream::connect(addr).expect("connect");
            // Send half a message, pause past the server's read timeout, then
            // send the rest. A transport that discarded the partial line would
            // never see a complete message.
            stream.write_all(br#"{"id":"1","meth"#).expect("first half");
            stream.flush().expect("flush");
            std::thread::sleep(std::time::Duration::from_millis(400));
            stream
                .write_all(b"od\":\"system.ping\"}\n")
                .expect("second half");
            stream.flush().expect("flush");
            std::thread::sleep(std::time::Duration::from_millis(200));
        });

        let mut conn = server.accept_blocking().expect("accept");
        let msg = read_with_deadline(&mut conn).expect("a reassembled message");
        assert!(msg.contains("system.ping"), "{msg}");
        client.join().expect("client thread");
    }

    #[test]
    fn a_closed_client_reports_the_connection_as_finished() {
        let server = TcpServer::new(ephemeral()).expect("bind");
        let addr = server.addr();

        let client = std::thread::spawn(move || {
            let conn = TcpClient::connect(addr).expect("connect");
            drop(conn);
        });

        let mut conn = server.accept_blocking().expect("accept");
        client.join().expect("client thread");
        assert!(read_with_deadline(&mut conn).is_none());
        assert!(!conn.is_open());
    }

    #[test]
    fn accept_reports_nothing_waiting_instead_of_blocking() {
        let server = TcpServer::new(ephemeral()).expect("bind");
        let started = std::time::Instant::now();
        assert!(server.accept().expect("accept").is_none());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "accept must not block when no client is waiting"
        );
    }
}
