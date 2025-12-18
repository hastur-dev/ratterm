//! API server implementation.
//!
//! Runs in a background thread, accepts connections, and communicates
//! with the main thread via channels.

use crate::api::protocol::{ApiRequest, ApiResponse};
use crate::api::transport::Connection;
use crate::api::{ApiError, RequestSender, request_channel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info, warn};

#[cfg(windows)]
use crate::api::transport::windows::WindowsServer;

#[cfg(unix)]
use crate::api::transport::unix::UnixServer;

/// Maximum time to wait for response (ms).
const RESPONSE_TIMEOUT_MS: u64 = 5000;

/// API server that runs in a background thread.
pub struct ApiServer {
    /// Thread handle for the server.
    thread_handle: Option<JoinHandle<()>>,
    /// Shutdown flag.
    shutdown: Arc<AtomicBool>,
    /// Request sender (to main thread) - kept for potential future use.
    #[allow(dead_code)]
    request_tx: RequestSender,
}

impl ApiServer {
    /// Starts the API server in a background thread.
    #[cfg(windows)]
    pub fn start(pipe_name: Option<&str>) -> Result<(Self, crate::api::RequestReceiver), ApiError> {
        let (request_tx, request_rx) = request_channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let request_tx_clone = request_tx.clone();
        let pipe_name = pipe_name.map(String::from);

        let thread_handle = thread::Builder::new()
            .name("api-server".into())
            .spawn(move || {
                Self::run_windows(pipe_name.as_deref(), request_tx_clone, shutdown_clone);
            })?;

        Ok((
            Self {
                thread_handle: Some(thread_handle),
                shutdown,
                request_tx,
            },
            request_rx,
        ))
    }

    /// Starts the API server in a background thread.
    #[cfg(unix)]
    pub fn start(
        socket_path: Option<std::path::PathBuf>,
    ) -> Result<(Self, crate::api::RequestReceiver), ApiError> {
        let (request_tx, request_rx) = request_channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let request_tx_clone = request_tx.clone();

        let thread_handle = thread::Builder::new()
            .name("api-server".into())
            .spawn(move || {
                Self::run_unix(socket_path, request_tx_clone, shutdown_clone);
            })?;

        Ok((
            Self {
                thread_handle: Some(thread_handle),
                shutdown,
                request_tx,
            },
            request_rx,
        ))
    }

    /// Signals the server to shut down.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Waits for the server thread to finish.
    pub fn join(mut self) {
        self.shutdown();
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Windows server loop.
    #[cfg(windows)]
    fn run_windows(pipe_name: Option<&str>, request_tx: RequestSender, shutdown: Arc<AtomicBool>) {
        let server = WindowsServer::new(pipe_name);
        info!("API server started on pipe: {}", server.pipe_name());

        while !shutdown.load(Ordering::SeqCst) {
            // Accept connection (blocking)
            match server.accept() {
                Ok(mut conn) => {
                    info!("API client connected");
                    Self::handle_connection(&mut conn, &request_tx, &shutdown);
                    info!("API client disconnected");
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

    /// Unix server loop.
    #[cfg(unix)]
    fn run_unix(
        socket_path: Option<std::path::PathBuf>,
        request_tx: RequestSender,
        shutdown: Arc<AtomicBool>,
    ) {
        let server = match UnixServer::new(socket_path) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create API server: {}", e);
                return;
            }
        };

        info!("API server started on socket: {:?}", server.socket_path());

        while !shutdown.load(Ordering::SeqCst) {
            // Accept connection (blocking)
            match server.accept() {
                Ok(mut conn) => {
                    info!("API client connected");
                    Self::handle_connection(&mut conn, &request_tx, &shutdown);
                    info!("API client disconnected");
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
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        // Just test that types compile correctly
        let shutdown = Arc::new(AtomicBool::new(false));
        assert!(!shutdown.load(Ordering::SeqCst));
    }
}
