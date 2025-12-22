//! Tab-related REST API handlers.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::extension::rest::{
    state::{ApiState, AppRequest, TabRequest},
    types::{
        ApiError, NewTabResponse, NewTerminalRequest, SwitchTabRequest, TabIndexQuery, TabInfo,
        TabListResponse,
    },
};

/// GET /tabs/terminal - List terminal tabs.
pub async fn list_terminal(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<TabListResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Tab(TabRequest::ListTerminal(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let result = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(TabListResponse {
        tabs: result
            .tabs
            .into_iter()
            .map(|t| TabInfo {
                index: t.index,
                title: t.title,
                modified: t.modified,
            })
            .collect(),
        active: result.active,
    }))
}

/// GET /tabs/editor - List editor tabs.
pub async fn list_editor(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<TabListResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Tab(TabRequest::ListEditor(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let result = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(TabListResponse {
        tabs: result
            .tabs
            .into_iter()
            .map(|t| TabInfo {
                index: t.index,
                title: t.title,
                modified: t.modified,
            })
            .collect(),
        active: result.active,
    }))
}

/// POST /tabs/terminal/new - Create new terminal tab.
pub async fn new_terminal(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<NewTerminalRequest>,
) -> Result<Json<NewTabResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Tab(TabRequest::NewTerminal {
            shell: req.shell,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let index = rx
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to receive response")),
            )
        })?
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiError::new(e))))?;

    Ok(Json(NewTabResponse { index }))
}

/// POST /tabs/terminal/switch - Switch terminal tab.
pub async fn switch_terminal(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SwitchTabRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Tab(TabRequest::SwitchTerminal {
            index: req.index,
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

    Ok(StatusCode::OK)
}

/// DELETE /tabs/terminal/close - Close terminal tab.
pub async fn close_terminal(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<TabIndexQuery>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Tab(TabRequest::CloseTerminal {
            index: query.index,
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

    Ok(StatusCode::OK)
}
