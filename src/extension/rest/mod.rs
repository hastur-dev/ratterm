//! REST API module for language-agnostic extension support.
//!
//! This module provides an HTTP REST API that allows extensions written in
//! any language to interact with ratterm. Extensions communicate via HTTP
//! requests and receive events via Server-Sent Events (SSE).
//!
//! ## Overview
//!
//! - **Base URL**: `http://127.0.0.1:7878/api/v1`
//! - **Auth**: Bearer token (stored in `~/.ratterm/api_token`)
//! - **Events**: SSE stream at `/events/stream`
//!
//! ## Example Usage (Python)
//!
//! ```python
//! import requests
//!
//! API_URL = "http://127.0.0.1:7878/api/v1"
//! TOKEN = open("~/.ratterm/api_token").read().strip()
//! HEADERS = {"Authorization": f"Bearer {TOKEN}"}
//!
//! # Get editor content
//! resp = requests.get(f"{API_URL}/editor/content", headers=HEADERS)
//! content = resp.json()["content"]
//!
//! # Show notification
//! requests.post(f"{API_URL}/system/notify",
//!               json={"message": "Hello!"},
//!               headers=HEADERS)
//! ```

pub mod auth;
pub mod handlers;
pub mod router;
pub mod server;
pub mod state;
pub mod types;

pub use server::{DEFAULT_PORT, RestApiServer};
pub use state::{ApiState, AppRequest};
pub use types::ApiEvent;
