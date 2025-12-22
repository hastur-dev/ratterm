//! REST API HTTP server.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use super::{
    auth::load_or_create_token,
    router::create_router,
    state::{ApiState, AppRequest},
    types::ApiEvent,
};

/// Default port for the REST API server.
pub const DEFAULT_PORT: u16 = 7878;

/// REST API server handle.
pub struct RestApiServer {
    /// Server handle for graceful shutdown.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// API state (shared with handlers).
    state: Arc<ApiState>,
    /// Server address.
    addr: SocketAddr,
}

impl RestApiServer {
    /// Creates and starts a new REST API server.
    ///
    /// # Errors
    /// Returns error if server fails to bind or start.
    pub async fn start(
        request_tx: mpsc::Sender<AppRequest>,
        port: Option<u16>,
    ) -> std::io::Result<Self> {
        let port = port.unwrap_or(DEFAULT_PORT);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));

        // Load or create auth token
        let auth_token = load_or_create_token()?;

        // Create event broadcast channel
        let (event_tx, _) = broadcast::channel(1024);

        // Create shared state
        let state = Arc::new(ApiState::new(request_tx, event_tx, auth_token.clone()));

        // Create router
        let router = create_router(Arc::clone(&state))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .layer(TraceLayer::new_for_http());

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Start server in background task
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_addr = listener.local_addr()?;

        tokio::spawn(async move {
            let server = axum::serve(listener, router).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });

            if let Err(e) = server.await {
                tracing::error!("REST API server error: {}", e);
            }
        });

        tracing::info!("REST API server started on http://{}", actual_addr);
        tracing::info!("API token: {}", auth_token);

        Ok(Self {
            shutdown_tx: Some(shutdown_tx),
            state,
            addr: actual_addr,
        })
    }

    /// Returns the server address.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the API URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Returns the API token.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.state.auth_token
    }

    /// Returns a reference to the API state.
    #[must_use]
    pub fn state(&self) -> &Arc<ApiState> {
        &self.state
    }

    /// Broadcasts an event to all SSE subscribers.
    pub fn broadcast_event(&self, event: ApiEvent) {
        self.state.broadcast_event(event);
    }

    /// Shuts down the server.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            tracing::info!("REST API server shutdown requested");
        }
    }
}

impl Drop for RestApiServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_start() {
        let (tx, _rx) = mpsc::channel(100);

        // Use a random port for testing
        let server = RestApiServer::start(tx, Some(0)).await;
        assert!(server.is_ok());

        if let Ok(server) = server {
            assert!(!server.token().is_empty());
        }
    }
}
