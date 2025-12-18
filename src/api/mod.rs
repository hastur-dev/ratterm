//! AI Control API module.
//!
//! Provides an IPC interface for AI agents to control Ratterm.
//! Uses Named Pipes on Windows and Unix domain sockets on Unix.

pub mod handler;
pub mod protocol;
pub mod server;
pub mod transport;

use std::sync::mpsc;
use thiserror::Error;

pub use handler::ApiHandler;
pub use protocol::{ApiRequest, ApiResponse};
pub use server::ApiServer;

/// Maximum requests to process per frame (bounded loop).
pub const MAX_REQUESTS_PER_FRAME: usize = 10;

/// API error types.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Transport/IO error.
    #[error("Transport error: {0}")]
    Transport(#[from] std::io::Error),

    /// Protocol/parsing error.
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Method not found.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters.
    #[error("Invalid params: {0}")]
    InvalidParams(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// No active terminal.
    #[error("No active terminal")]
    NoActiveTerminal,

    /// No active editor.
    #[error("No active editor")]
    NoActiveEditor,

    /// Tab not found.
    #[error("Tab not found: {0}")]
    TabNotFound(usize),

    /// File not found.
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Channel send error.
    #[error("Channel send error")]
    ChannelSend,

    /// Channel receive error.
    #[error("Channel receive error")]
    ChannelRecv,
}

impl ApiError {
    /// Converts error to JSON-RPC error code.
    #[must_use]
    pub fn to_error_code(&self) -> i32 {
        match self {
            ApiError::Protocol(_) | ApiError::Json(_) => -32700, // Parse error
            ApiError::MethodNotFound(_) => -32601,               // Method not found
            ApiError::InvalidParams(_) => -32602,                // Invalid params
            ApiError::Internal(_) => -32603,                     // Internal error
            _ => -32000,                                         // Server error
        }
    }
}

/// Channel types for API communication between threads.
pub type RequestSender = mpsc::Sender<(ApiRequest, ResponseSender)>;
pub type RequestReceiver = mpsc::Receiver<(ApiRequest, ResponseSender)>;
pub type ResponseSender = mpsc::Sender<ApiResponse>;
pub type ResponseReceiver = mpsc::Receiver<ApiResponse>;

/// Creates a new request channel pair.
#[must_use]
pub fn request_channel() -> (RequestSender, RequestReceiver) {
    mpsc::channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            ApiError::MethodNotFound("test".into()).to_error_code(),
            -32601
        );
        assert_eq!(
            ApiError::InvalidParams("test".into()).to_error_code(),
            -32602
        );
        assert_eq!(ApiError::NoActiveTerminal.to_error_code(), -32000);
    }
}
