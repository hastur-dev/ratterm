//! API server implementation.
//!
//! Runs in a background thread, accepts connections, and communicates
//! with the main thread via channels.
//!
//! Three transports share one accept loop: a named pipe on Windows, a Unix
//! domain socket elsewhere, and a loopback TCP port on every platform. Every
//! connection starts unauthenticated and must present the session token before
//! anything but `system.ping` is answered.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use tracing::{debug, error, info, warn};

use crate::api::auth::{AUTHENTICATE_METHOD, ConnectionAuth, SessionToken};
use crate::api::protocol::{ApiRequest, ApiResponse};
use crate::api::transport::Connection;
use crate::api::transport::tcp::TcpServer;
use crate::api::{ApiError, RequestSender, request_channel};

#[cfg(windows)]
use crate::api::transport::windows::WindowsServer;

#[cfg(unix)]
use crate::api::transport::unix::UnixServer;

/// Maximum time to wait for response (ms).
const RESPONSE_TIMEOUT_MS: u64 = 5000;

/// How long the accept loop sleeps when no client is waiting.
const POLL_INTERVAL_MS: u64 = 10;

/// How long [`ApiServer::join`] waits for the server thread before detaching.
///
/// A transport whose accept or read blocks with no way to cancel it (the
/// Windows named pipe) would otherwise hang the caller; the thread dies with
/// the process instead.
const JOIN_TIMEOUT_MS: u64 = 2000;

/// JSON-RPC error code returned when a token is required or wrong.
pub const ERROR_UNAUTHENTICATED: i32 = -32_001;

/// Where the control API listens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ApiEndpoint {
    /// Platform default: named pipe on Windows, Unix socket elsewhere.
    #[default]
    PlatformDefault,
    /// An explicit named pipe name (Windows) or socket path (Unix).
    Local(String),
    /// A loopback TCP address.
    Tcp(std::net::SocketAddr),
}

impl ApiEndpoint {
    /// Renders the endpoint for logs and for the `system.info` reply.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::PlatformDefault => {
                #[cfg(windows)]
                {
                    crate::api::transport::DEFAULT_PIPE_NAME.to_string()
                }
                #[cfg(not(windows))]
                {
                    crate::api::transport::default_socket_path()
                        .display()
                        .to_string()
                }
            }
            Self::Local(name) => name.clone(),
            Self::Tcp(addr) => format!("tcp://{addr}"),
        }
    }
}

/// How to start the control API.
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Where to listen.
    pub endpoint: ApiEndpoint,
    /// Token every client must present; `None` disables authentication.
    pub token: Option<SessionToken>,
    /// Where to publish the token so a local client can read it.
    pub token_path: Option<PathBuf>,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            endpoint: ApiEndpoint::PlatformDefault,
            token: None,
            token_path: None,
        }
    }
}

impl ApiServerConfig {
    /// A configuration that mints a fresh token and publishes it at the
    /// default path.
    ///
    /// # Errors
    /// Returns an error if the system random source is unavailable.
    pub fn secure_default() -> Result<Self, ApiError> {
        let token = SessionToken::generate()
            .map_err(|e| ApiError::Internal(format!("could not mint an API token: {e}")))?;
        Ok(Self {
            endpoint: ApiEndpoint::PlatformDefault,
            token: Some(token),
            token_path: Some(SessionToken::default_path()),
        })
    }

    /// Sets the endpoint.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: ApiEndpoint) -> Self {
        self.endpoint = endpoint;
        self
    }

    /// Returns true if this configuration leaves the endpoint open.
    #[must_use]
    pub const fn is_unauthenticated(&self) -> bool {
        self.token.is_none()
    }
}

/// Something that produces connections.
///
/// `Ok(None)` means "nothing waiting right now", which is what lets the accept
/// loop notice a shutdown request instead of parking until a client appears.
trait Acceptor: Send {
    /// The connection type produced.
    type Conn: Connection;

    /// Accepts a client if one is waiting.
    fn accept(&self) -> Result<Option<Self::Conn>, ApiError>;
}

#[cfg(windows)]
impl Acceptor for WindowsServer {
    type Conn = crate::api::transport::windows::WindowsConnection;

    fn accept(&self) -> Result<Option<Self::Conn>, ApiError> {
        WindowsServer::accept(self).map(Some)
    }
}

#[cfg(unix)]
impl Acceptor for UnixServer {
    type Conn = crate::api::transport::unix::UnixConnection;

    fn accept(&self) -> Result<Option<Self::Conn>, ApiError> {
        UnixServer::accept(self).map(Some)
    }
}

impl Acceptor for TcpServer {
    type Conn = crate::api::transport::tcp::TcpConnection;

    fn accept(&self) -> Result<Option<Self::Conn>, ApiError> {
        TcpServer::accept(self)
    }
}

/// API server that runs in a background thread.
pub struct ApiServer {
    /// Thread handle for the server.
    thread_handle: Option<JoinHandle<()>>,
    /// Shutdown flag.
    shutdown: Arc<AtomicBool>,
    /// Request sender (to main thread) - kept for potential future use.
    #[allow(dead_code)]
    request_tx: RequestSender,
    /// Human-readable endpoint, for status display.
    endpoint: String,
    /// Where the token was published, so it can be removed on shutdown.
    token_path: Option<PathBuf>,
    /// Set by the server thread just before it returns.
    finished: Arc<AtomicBool>,
}

impl ApiServer {
    /// Starts the API server on the platform default endpoint with no
    /// authentication.
    ///
    /// Retained so existing callers keep compiling; prefer
    /// [`ApiServer::start_with`] with a token.
    ///
    /// # Errors
    /// Returns an error if the server thread cannot be spawned.
    #[cfg(windows)]
    pub fn start(pipe_name: Option<&str>) -> Result<(Self, crate::api::RequestReceiver), ApiError> {
        let endpoint = pipe_name.map_or(ApiEndpoint::PlatformDefault, |n| {
            ApiEndpoint::Local(n.to_string())
        });
        Self::start_with(ApiServerConfig {
            endpoint,
            token: None,
            token_path: None,
        })
    }

    /// Starts the API server on the platform default endpoint with no
    /// authentication.
    ///
    /// # Errors
    /// Returns an error if the server thread cannot be spawned.
    #[cfg(unix)]
    pub fn start(
        socket_path: Option<std::path::PathBuf>,
    ) -> Result<(Self, crate::api::RequestReceiver), ApiError> {
        let endpoint = socket_path.map_or(ApiEndpoint::PlatformDefault, |p| {
            ApiEndpoint::Local(p.display().to_string())
        });
        Self::start_with(ApiServerConfig {
            endpoint,
            token: None,
            token_path: None,
        })
    }

    /// Starts the API server from an explicit configuration.
    ///
    /// # Errors
    /// Returns an error if the endpoint cannot be bound, the token cannot be
    /// published, or the server thread cannot be spawned.
    pub fn start_with(
        config: ApiServerConfig,
    ) -> Result<(Self, crate::api::RequestReceiver), ApiError> {
        if matches!(config.endpoint, ApiEndpoint::Tcp(_)) && config.is_unauthenticated() {
            return Err(ApiError::Protocol(
                "the TCP control endpoint requires a token; every local process can reach it"
                    .to_string(),
            ));
        }

        if let (Some(token), Some(path)) = (config.token.as_ref(), config.token_path.as_ref()) {
            token
                .write_to(path)
                .map_err(|e| ApiError::Internal(format!("could not publish the API token: {e}")))?;
            info!("API token written to {}", path.display());
        }

        let endpoint_label = config.endpoint.describe();
        let (request_tx, request_rx) = request_channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let request_tx_clone = request_tx.clone();
        let token = config.token.clone();
        let endpoint = config.endpoint.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();

        let thread_handle = thread::Builder::new()
            .name("api-server".into())
            .spawn(move || {
                Self::run(endpoint, token, request_tx_clone, shutdown_clone);
                finished_clone.store(true, Ordering::SeqCst);
            })?;

        Ok((
            Self {
                thread_handle: Some(thread_handle),
                shutdown,
                request_tx,
                endpoint: endpoint_label,
                token_path: config.token_path,
                finished,
            },
            request_rx,
        ))
    }

    /// Returns the endpoint the server is listening on.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Signals the server to shut down and removes the published token.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(path) = self.token_path.as_ref()
            && let Err(e) = SessionToken::remove_file(path)
        {
            warn!("Could not remove the API token file: {}", e);
        }
    }

    /// Waits for the server thread to finish, giving up after
    /// [`JOIN_TIMEOUT_MS`].
    ///
    /// Returns true if the thread ended, false if it was detached.
    pub fn join(mut self) -> bool {
        self.shutdown();

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(JOIN_TIMEOUT_MS);
        while std::time::Instant::now() < deadline {
            if self.finished.load(Ordering::SeqCst) {
                if let Some(handle) = self.thread_handle.take() {
                    let _ = handle.join();
                }
                return true;
            }
            thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
        }

        warn!("API server thread did not stop in time; detaching it");
        self.thread_handle.take();
        false
    }

    /// Dispatches to the transport-specific accept loop.
    fn run(
        endpoint: ApiEndpoint,
        token: Option<SessionToken>,
        request_tx: RequestSender,
        shutdown: Arc<AtomicBool>,
    ) {
        match endpoint {
            ApiEndpoint::Tcp(addr) => match TcpServer::new(addr) {
                Ok(server) => Self::accept_loop(&server, token.as_ref(), &request_tx, &shutdown),
                Err(e) => error!("Failed to bind the TCP control endpoint: {}", e),
            },
            other => Self::run_local(&other, token.as_ref(), &request_tx, &shutdown),
        }
    }

    #[cfg(windows)]
    fn run_local(
        endpoint: &ApiEndpoint,
        token: Option<&SessionToken>,
        request_tx: &RequestSender,
        shutdown: &Arc<AtomicBool>,
    ) {
        let name = match endpoint {
            ApiEndpoint::Local(name) => Some(name.as_str()),
            _ => None,
        };
        let server = WindowsServer::new(name);
        info!("API server started on pipe: {}", server.pipe_name());
        Self::accept_loop(&server, token, request_tx, shutdown);
    }

    #[cfg(unix)]
    fn run_local(
        endpoint: &ApiEndpoint,
        token: Option<&SessionToken>,
        request_tx: &RequestSender,
        shutdown: &Arc<AtomicBool>,
    ) {
        let path = match endpoint {
            ApiEndpoint::Local(p) => Some(std::path::PathBuf::from(p)),
            _ => None,
        };
        let server = match UnixServer::new(path) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create API server: {}", e);
                return;
            }
        };
        info!("API server started on socket: {:?}", server.socket_path());
        Self::accept_loop(&server, token, request_tx, shutdown);
    }

    /// Accepts connections until shutdown.
    fn accept_loop<A: Acceptor>(
        server: &A,
        token: Option<&SessionToken>,
        request_tx: &RequestSender,
        shutdown: &Arc<AtomicBool>,
    ) {
        while !shutdown.load(Ordering::SeqCst) {
            match server.accept() {
                Ok(Some(mut conn)) => {
                    info!("API client connected");
                    let auth = match token {
                        Some(t) => ConnectionAuth::requiring(t.clone()),
                        None => ConnectionAuth::disabled(),
                    };
                    Self::handle_connection(&mut conn, auth, request_tx, shutdown);
                    info!("API client disconnected");
                }
                Ok(None) => {
                    thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(e) => {
                    if !shutdown.load(Ordering::SeqCst) {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        }

        info!("API server shutting down");
    }

    /// Handles a single client connection.
    fn handle_connection<C: Connection>(
        conn: &mut C,
        mut auth: ConnectionAuth,
        request_tx: &RequestSender,
        shutdown: &Arc<AtomicBool>,
    ) {
        const MAX_ITERATIONS: usize = 1_000_000;

        for _ in 0..MAX_ITERATIONS {
            if shutdown.load(Ordering::SeqCst) || !conn.is_open() {
                break;
            }

            // Read message
            let msg = match conn.read_message() {
                Ok(Some(m)) => m,
                Ok(None) => {
                    // No message available or connection closed
                    if !conn.is_open() {
                        break;
                    }
                    thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    warn!("Error reading message: {}", e);
                    break;
                }
            };

            debug!("Received API request: {}", msg);

            // Parse request
            let request: ApiRequest = match serde_json::from_str(&msg) {
                Ok(r) => r,
                Err(e) => {
                    let error_resp =
                        ApiResponse::error("".to_string(), -32700, format!("Parse error: {}", e));
                    if let Err(e) = Self::send_response(conn, &error_resp) {
                        warn!("Failed to send error response: {}", e);
                    }
                    continue;
                }
            };

            // The handshake and the gate both live here rather than in the
            // handler, so an unauthenticated request never reaches App state.
            if let Some(response) = Self::check_auth(&mut auth, &request) {
                if let Err(e) = Self::send_response(conn, &response) {
                    warn!("Failed to send auth response: {}", e);
                    break;
                }
                continue;
            }

            // Create response channel
            let (resp_tx, resp_rx) = mpsc::channel();

            // Send request to main thread
            if let Err(e) = request_tx.send((request.clone(), resp_tx)) {
                error!("Failed to send request to main thread: {}", e);
                let error_resp = ApiResponse::error(request.id, -32603, "Internal error");
                if let Err(e) = Self::send_response(conn, &error_resp) {
                    warn!("Failed to send error response: {}", e);
                }
                continue;
            }

            // Wait for response with timeout
            let response =
                match resp_rx.recv_timeout(std::time::Duration::from_millis(RESPONSE_TIMEOUT_MS)) {
                    Ok(r) => r,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        warn!("Request timed out: {}", request.method);
                        ApiResponse::error(request.id, -32000, "Request timed out")
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        error!("Response channel disconnected");
                        ApiResponse::error(request.id, -32603, "Internal error")
                    }
                };

            // Send response
            if let Err(e) = Self::send_response(conn, &response) {
                warn!("Failed to send response: {}", e);
                break;
            }
        }
    }

    /// Returns a response to send instead of dispatching, when the request is
    /// the handshake or is not permitted yet.
    fn check_auth(auth: &mut ConnectionAuth, request: &ApiRequest) -> Option<ApiResponse> {
        if request.method == AUTHENTICATE_METHOD {
            let supplied = request
                .params
                .get("token")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();

            return Some(if auth.authenticate(supplied) {
                ApiResponse::success(
                    request.id.clone(),
                    serde_json::json!({ "authenticated": true }),
                )
            } else {
                ApiResponse::error(
                    request.id.clone(),
                    ERROR_UNAUTHENTICATED,
                    "Invalid API token",
                )
            });
        }

        if auth.permits(&request.method) {
            return None;
        }

        Some(ApiResponse::error(
            request.id.clone(),
            ERROR_UNAUTHENTICATED,
            format!(
                "Authentication required: call {AUTHENTICATE_METHOD} with the token from the API token file first"
            ),
        ))
    }

    /// Sends a response to the client.
    fn send_response<C: Connection>(conn: &mut C, response: &ApiResponse) -> Result<(), ApiError> {
        let json = serde_json::to_string(response)?;
        debug!("Sending API response: {}", json);
        conn.write_message(&json)
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(method: &str, params: serde_json::Value) -> ApiRequest {
        ApiRequest {
            id: "1".to_string(),
            method: method.to_string(),
            params,
        }
    }

    fn no_params() -> serde_json::Value {
        serde_json::Value::Null
    }

    #[test]
    fn the_platform_default_endpoint_has_a_description() {
        assert!(!ApiEndpoint::PlatformDefault.describe().is_empty());
    }

    #[test]
    fn a_tcp_endpoint_describes_itself_with_a_scheme() {
        let endpoint = ApiEndpoint::Tcp(crate::api::transport::tcp::default_tcp_addr());
        assert!(endpoint.describe().starts_with("tcp://"));
    }

    #[test]
    fn a_local_endpoint_describes_itself_by_name() {
        assert_eq!(
            ApiEndpoint::Local("some-name".to_string()).describe(),
            "some-name"
        );
    }

    #[test]
    fn a_tcp_endpoint_without_a_token_is_refused() {
        let config = ApiServerConfig {
            endpoint: ApiEndpoint::Tcp(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
            token: None,
            token_path: None,
        };
        match ApiServer::start_with(config) {
            Err(ApiError::Protocol(msg)) => assert!(msg.contains("token"), "{msg}"),
            other => panic!("expected refusal, got {:?}", other.err()),
        }
    }

    #[test]
    fn secure_default_mints_a_token() {
        let config = ApiServerConfig::secure_default().expect("config");
        assert!(!config.is_unauthenticated());
        assert!(config.token_path.is_some());
    }

    #[test]
    fn an_unauthenticated_request_is_refused_with_the_auth_error_code() {
        let token = SessionToken::generate().expect("token");
        let mut auth = ConnectionAuth::requiring(token);

        let response =
            ApiServer::check_auth(&mut auth, &request("editor.read_content", no_params()))
                .expect("a refusal");
        let error = response.error.expect("an error");
        assert_eq!(error.code, ERROR_UNAUTHENTICATED);
        assert!(error.message.contains(AUTHENTICATE_METHOD));
    }

    #[test]
    fn ping_is_answered_before_authentication() {
        let token = SessionToken::generate().expect("token");
        let mut auth = ConnectionAuth::requiring(token);
        assert!(ApiServer::check_auth(&mut auth, &request("system.ping", no_params())).is_none());
    }

    #[test]
    fn the_handshake_accepts_the_right_token() {
        let token = SessionToken::generate().expect("token");
        let hex = token.as_hex().to_string();
        let mut auth = ConnectionAuth::requiring(token);

        let response = ApiServer::check_auth(
            &mut auth,
            &request(AUTHENTICATE_METHOD, json!({ "token": hex })),
        )
        .expect("a response");
        assert!(response.error.is_none());
        assert!(auth.is_authenticated());

        // Everything is permitted afterwards.
        assert!(
            ApiServer::check_auth(&mut auth, &request("editor.read_content", no_params()))
                .is_none()
        );
    }

    #[test]
    fn the_handshake_rejects_the_wrong_token() {
        let token = SessionToken::generate().expect("token");
        let mut auth = ConnectionAuth::requiring(token);

        let response = ApiServer::check_auth(
            &mut auth,
            &request(AUTHENTICATE_METHOD, json!({ "token": "nope" })),
        )
        .expect("a response");
        assert_eq!(
            response.error.expect("an error").code,
            ERROR_UNAUTHENTICATED
        );
        assert!(!auth.is_authenticated());
    }

    #[test]
    fn the_handshake_rejects_a_missing_token_field() {
        let token = SessionToken::generate().expect("token");
        let mut auth = ConnectionAuth::requiring(token);

        let response = ApiServer::check_auth(&mut auth, &request(AUTHENTICATE_METHOD, no_params()))
            .expect("a response");
        assert!(response.error.is_some());
        assert!(!auth.is_authenticated());
    }

    #[test]
    fn with_authentication_disabled_every_method_passes_straight_through() {
        let mut auth = ConnectionAuth::disabled();
        assert!(
            ApiServer::check_auth(&mut auth, &request("editor.read_content", no_params()))
                .is_none()
        );
    }
}
