//! System-related REST API handlers.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::extension::rest::{
    state::{ApiState, AppRequest, SystemRequest},
    types::{
        ApiError, ConfigQuery, ConfigResponse, CwdResponse, ExtensionListResponse, HealthResponse,
        NotifyRequest, SetStatusRequest, SetThemeRequest, StatusResponse, SuccessResponse,
        ThemeListResponse, ThemeResponse, VersionResponse,
    },
};

/// GET /system/version - Get ratterm version.
pub async fn get_version(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<VersionResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::GetVersion(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let version = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(VersionResponse { version }))
}

/// GET /system/status - Get status message.
pub async fn get_status(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::GetStatus(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let message = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(StatusResponse { message }))
}

/// PUT /system/status - Set status message.
pub async fn set_status(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SetStatusRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::SetStatus {
            message: req.message,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(StatusCode::OK)
}

/// GET /system/cwd - Get current working directory.
pub async fn get_cwd(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CwdResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::GetCwd(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let path = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(CwdResponse { path }))
}

/// GET /system/config - Get config value.
pub async fn get_config(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ConfigQuery>,
) -> Result<Json<ConfigResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::GetConfig {
            key: query.key,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let value = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(ConfigResponse { value }))
}

/// GET /system/theme - Get current theme name.
pub async fn get_theme(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ThemeResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::GetTheme(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let name = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(ThemeResponse { name }))
}

/// PUT /system/theme - Set theme by name.
pub async fn set_theme(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SetThemeRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::SetTheme {
            name: req.name,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    rx.await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to receive response")),
            )
        })?
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiError::new(e))))?;

    Ok(Json(SuccessResponse { success: true }))
}

/// GET /system/themes - List available themes.
pub async fn list_themes(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ThemeListResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::ListThemes(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let (themes, current) = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(ThemeListResponse { themes, current }))
}

/// POST /system/notify - Show notification.
pub async fn notify(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<NotifyRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::System(SystemRequest::Notify {
            message: req.message,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(SuccessResponse { success: true }))
}

/// GET /extensions/list - List loaded extensions.
pub async fn list_extensions(
    State(_state): State<Arc<ApiState>>,
) -> Json<ExtensionListResponse> {
    // TODO: Get actual extension list from ExtensionProcessManager
    Json(ExtensionListResponse {
        extensions: vec![],
    })
}

/// GET /extensions/health - Health check endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Placeholder for reload extension
pub async fn reload_extension(
    State(_state): State<Arc<ApiState>>,
    Json(_req): Json<crate::extension::rest::types::ReloadExtensionRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    // TODO: Implement extension reloading
    Ok(Json(SuccessResponse { success: true }))
}
