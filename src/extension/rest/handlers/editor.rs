//! Editor-related REST API handlers.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};

use crate::extension::rest::{
    state::{ApiState, AppRequest, EditorRequest},
    types::{
        ApiError, CurrentFileResponse, CursorPosition, EditorContentResponse, InsertTextRequest,
        OpenFileRequest, SaveFileRequest, SaveFileResponse, SetContentRequest, SetCursorRequest,
    },
};

/// GET /editor/content - Get editor content.
pub async fn get_content(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EditorContentResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::GetContent(tx)))
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

    Ok(Json(EditorContentResponse {
        content: result.content,
        path: result.path,
        modified: result.modified,
        cursor: CursorPosition {
            line: result.cursor_line,
            col: result.cursor_col,
        },
    }))
}

/// PUT /editor/content - Set editor content.
pub async fn set_content(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SetContentRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::SetContent {
            content: req.content,
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

/// POST /editor/open - Open a file.
pub async fn open_file(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<OpenFileRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::OpenFile {
            path: req.path,
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

/// POST /editor/save - Save current file.
pub async fn save_file(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SaveFileRequest>,
) -> Result<Json<SaveFileResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::Save {
            path: req.path,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let path = rx
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to receive response")),
            )
        })?
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiError::new(e))))?;

    Ok(Json(SaveFileResponse { path }))
}

/// GET /editor/cursor - Get cursor position.
pub async fn get_cursor(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CursorPosition>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::GetCursor(tx)))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let (line, col) = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(CursorPosition { line, col }))
}

/// PUT /editor/cursor - Set cursor position.
pub async fn set_cursor(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SetCursorRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::SetCursor {
            line: req.line,
            col: req.col,
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

/// POST /editor/insert - Insert text at position.
pub async fn insert_text(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<InsertTextRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::InsertText {
            text: req.text,
            line: req.line,
            col: req.col,
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

/// GET /editor/file - Get current file path.
pub async fn get_file(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CurrentFileResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Editor(EditorRequest::GetFile(tx)))
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

    Ok(Json(CurrentFileResponse { path }))
}
