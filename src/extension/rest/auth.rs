//! Token-based authentication middleware for the REST API.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

use super::state::ApiState;

/// Authentication middleware that validates Bearer tokens.
pub async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check endpoint
    if request.uri().path() == "/api/v1/extensions/health" {
        return Ok(next.run(request).await);
    }

    // Get Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == state.auth_token {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Generates a random API token.
#[must_use]
pub fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Loads or creates an API token from the config directory.
///
/// # Errors
/// Returns error if file operations fail.
pub fn load_or_create_token() -> std::io::Result<String> {
    let config_dir = dirs::home_dir()
        .map(|h| h.join(".ratterm"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home directory"))?;

    // Ensure config directory exists
    std::fs::create_dir_all(&config_dir)?;

    let token_path = config_dir.join("api_token");

    if token_path.exists() {
        // Read existing token
        let token = std::fs::read_to_string(&token_path)?;
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Generate new token
    let token = generate_token();
    std::fs::write(&token_path, &token)?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token1 = generate_token();
        let token2 = generate_token();

        // Tokens should be valid UUIDs
        assert_eq!(token1.len(), 36);
        assert_eq!(token2.len(), 36);

        // Tokens should be unique
        assert_ne!(token1, token2);
    }
}
