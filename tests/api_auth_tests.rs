//! End-to-end tests for control-API authentication.
//!
//! The IPC endpoint was previously unauthenticated: any local process could
//! connect and read the editor or inject keystrokes into the PTY. These tests
//! drive a real listener over loopback TCP and assert that an unauthenticated
//! request never reaches the application at all.

#![allow(clippy::expect_used)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratterm::api::server::ERROR_UNAUTHENTICATED;
use ratterm::api::transport::Connection;
use ratterm::api::transport::tcp::TcpClient;
use ratterm::api::{ApiEndpoint, ApiResponse, ApiServer, ApiServerConfig, SessionToken};

/// A stand-in for the application's request loop.
///
/// Records every method that reaches it, which is how the tests prove that a
/// refused request was refused at the door rather than answered.
struct FakeApp {
    seen: Arc<Mutex<Vec<String>>>,
    count: Arc<AtomicUsize>,
}

impl FakeApp {
    fn spawn(rx: ratterm::api::RequestReceiver) -> Self {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let seen_clone = seen.clone();
        let count_clone = count.clone();

        std::thread::spawn(move || {
            while let Ok((request, resp_tx)) = rx.recv() {
                if let Ok(mut guard) = seen_clone.lock() {
                    guard.push(request.method.clone());
                }
                count_clone.fetch_add(1, Ordering::SeqCst);
                let _ = resp_tx.send(ApiResponse::success(
                    request.id,
                    serde_json::json!({ "ok": true }),
                ));
            }
        });

        Self { seen, count }
    }

    fn methods(&self) -> Vec<String> {
        self.seen.lock().map(|g| g.clone()).unwrap_or_default()
    }

    fn dispatched(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    /// Waits until at least `n` requests have been dispatched, or gives up.
    fn wait_for(&self, n: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.dispatched() >= n {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}

/// Starts a server on an ephemeral loopback port and returns its address.
fn start_server(token: SessionToken) -> (ApiServer, FakeApp, SocketAddr) {
    // Pick a free port by binding and releasing, then hand it to the server.
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("probe bind");
    let addr = probe.local_addr().expect("probe addr");
    drop(probe);

    let config = ApiServerConfig {
        endpoint: ApiEndpoint::Tcp(addr),
        token: Some(token),
        token_path: None,
    };
    let (server, rx) = ApiServer::start_with(config).expect("start server");
    let app = FakeApp::spawn(rx);

    (server, app, addr)
}

/// Connects, retrying briefly while the listener comes up.
fn connect(addr: SocketAddr) -> ratterm::api::transport::tcp::TcpConnection {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match TcpClient::connect(addr) {
            Ok(conn) => return conn,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("could not connect to {addr}: {e}"),
        }
    }
}

fn call(
    conn: &mut ratterm::api::transport::tcp::TcpConnection,
    id: &str,
    method: &str,
    params: serde_json::Value,
) -> ApiResponse {
    let request = serde_json::json!({ "id": id, "method": method, "params": params });
    conn.write_message(&request.to_string()).expect("write");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match conn.read_message().expect("read") {
            Some(text) => return serde_json::from_str(&text).expect("parse response"),
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            None => panic!("no response to {method}"),
        }
    }
}

#[test]
fn an_unauthenticated_request_is_refused_and_never_reaches_the_app() {
    let token = SessionToken::generate().expect("token");
    let (server, app, addr) = start_server(token);
    let mut conn = connect(addr);

    let response = call(
        &mut conn,
        "1",
        "editor.read_content",
        serde_json::Value::Null,
    );

    let error = response.error.expect("an error");
    assert_eq!(error.code, ERROR_UNAUTHENTICATED);
    assert_eq!(
        app.dispatched(),
        0,
        "the request must not reach the application: saw {:?}",
        app.methods()
    );

    server.join();
}

#[test]
fn presenting_the_token_unlocks_the_connection() {
    let token = SessionToken::generate().expect("token");
    let hex = token.as_hex().to_string();
    let (server, app, addr) = start_server(token);
    let mut conn = connect(addr);

    let handshake = call(
        &mut conn,
        "1",
        "session.authenticate",
        serde_json::json!({ "token": hex }),
    );
    assert!(handshake.error.is_none(), "{:?}", handshake.error);

    let response = call(
        &mut conn,
        "2",
        "editor.read_content",
        serde_json::Value::Null,
    );
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(app.wait_for(1));
    assert_eq!(app.methods(), vec!["editor.read_content".to_string()]);

    server.join();
}

#[test]
fn the_wrong_token_leaves_the_connection_locked() {
    let token = SessionToken::generate().expect("token");
    let (server, app, addr) = start_server(token);
    let mut conn = connect(addr);

    let handshake = call(
        &mut conn,
        "1",
        "session.authenticate",
        serde_json::json!({ "token": "0".repeat(64) }),
    );
    assert_eq!(
        handshake.error.expect("an error").code,
        ERROR_UNAUTHENTICATED
    );

    let response = call(&mut conn, "2", "terminal.send_keys", serde_json::json!({}));
    assert_eq!(
        response.error.expect("an error").code,
        ERROR_UNAUTHENTICATED
    );
    assert_eq!(app.dispatched(), 0);

    server.join();
}

#[test]
fn ping_answers_before_authentication_so_clients_can_probe() {
    let token = SessionToken::generate().expect("token");
    let (server, app, addr) = start_server(token);
    let mut conn = connect(addr);

    let response = call(&mut conn, "1", "system.ping", serde_json::Value::Null);
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(app.wait_for(1));

    server.join();
}

#[test]
fn a_second_connection_must_authenticate_on_its_own() {
    let token = SessionToken::generate().expect("token");
    let hex = token.as_hex().to_string();
    let (server, app, addr) = start_server(token);

    let mut first = connect(addr);
    let handshake = call(
        &mut first,
        "1",
        "session.authenticate",
        serde_json::json!({ "token": hex }),
    );
    assert!(handshake.error.is_none());
    let ok = call(
        &mut first,
        "2",
        "editor.read_content",
        serde_json::Value::Null,
    );
    assert!(ok.error.is_none());
    drop(first);

    let mut second = connect(addr);
    let refused = call(
        &mut second,
        "3",
        "editor.read_content",
        serde_json::Value::Null,
    );
    assert_eq!(
        refused.error.expect("an error").code,
        ERROR_UNAUTHENTICATED,
        "authentication must not be inherited between connections"
    );
    assert_eq!(app.dispatched(), 1);

    server.join();
}

#[test]
fn the_token_file_lets_a_client_authenticate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let token_path = dir.path().join("api.token");

    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("probe bind");
    let addr = probe.local_addr().expect("probe addr");
    drop(probe);

    let token = SessionToken::generate().expect("token");
    let config = ApiServerConfig {
        endpoint: ApiEndpoint::Tcp(addr),
        token: Some(token),
        token_path: Some(token_path.clone()),
    };
    let (server, rx) = ApiServer::start_with(config).expect("start");
    let app = FakeApp::spawn(rx);

    // A client learns the token the same way a real one would.
    let from_file = SessionToken::read_from(&token_path).expect("read token file");
    let mut conn = connect(addr);
    let handshake = call(
        &mut conn,
        "1",
        "session.authenticate",
        serde_json::json!({ "token": from_file.as_hex() }),
    );
    assert!(handshake.error.is_none(), "{:?}", handshake.error);

    let response = call(&mut conn, "2", "system.info", serde_json::Value::Null);
    assert!(response.error.is_none());
    // The handshake is answered by the server, so only the second call is
    // dispatched to the application.
    assert!(app.wait_for(1));
    assert_eq!(app.methods(), vec!["system.info".to_string()]);

    drop(conn);
    server.shutdown();
    assert!(
        !token_path.exists(),
        "the token file must be removed on shutdown"
    );
    server.join();
}

#[test]
fn a_tcp_endpoint_cannot_be_started_without_a_token() {
    let config = ApiServerConfig {
        endpoint: ApiEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 0))),
        token: None,
        token_path: None,
    };
    assert!(
        ApiServer::start_with(config).is_err(),
        "an open TCP control endpoint must be refused"
    );
}

#[test]
fn a_malformed_message_is_answered_with_a_parse_error_and_not_dispatched() {
    let token = SessionToken::generate().expect("token");
    let (server, app, addr) = start_server(token);
    let mut conn = connect(addr);

    conn.write_message("{not json").expect("write");
    let deadline = Instant::now() + Duration::from_secs(10);
    let response: ApiResponse = loop {
        match conn.read_message().expect("read") {
            Some(text) => break serde_json::from_str(&text).expect("parse"),
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            None => panic!("no response"),
        }
    };

    assert_eq!(response.error.expect("an error").code, -32700);
    assert_eq!(app.dispatched(), 0);

    server.join();
}
