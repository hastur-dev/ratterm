//! Layout-related REST API handlers.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::extension::rest::{
    state::{ApiState, AppRequest, LayoutRequest},
    types::{
        ApiError, FocusPaneRequest, LayoutStateResponse, SetSplitRequest, ToggleIdeResponse,
    },
};

/// GET /layout/state - Get layout state.
pub async fn get_state(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<LayoutStateResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Layout(LayoutRequest::GetState(tx)))
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

    Ok(Json(LayoutStateResponse {
        focused: result.focused,
        ide_visible: result.ide_visible,
        split_ratio: result.split_ratio,
    }))
}

/// POST /layout/focus - Set focused pane.
pub async fn set_focus(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<FocusPaneRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Layout(LayoutRequest::SetFocus {
            pane: req.pane,
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

/// POST /layout/toggle_ide - Toggle IDE visibility.
pub async fn toggle_ide(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ToggleIdeResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Layout(LayoutRequest::ToggleIde(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let visible = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(ToggleIdeResponse { visible }))
}

/// PUT /layout/split - Set split ratio.
pub async fn set_split(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SetSplitRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Layout(LayoutRequest::SetSplit {
            ratio: req.ratio,
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
