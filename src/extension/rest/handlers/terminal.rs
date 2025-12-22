//! Terminal-related REST API handlers.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};

use crate::extension::rest::{
    state::{ApiState, AppRequest, TerminalRequest},
    types::{
        ApiError, BufferQuery, ClearTerminalRequest, CursorPosition, ScrollRequest,
        ScrollbackQuery, ScrollbackResponse, SelectionResponse, SendKeysRequest, SizeQuery,
        SuccessResponse, TerminalBufferResponse, TerminalCursorResponse, TerminalSize,
        TerminalTitleResponse,
    },
};

/// POST /terminal/send_keys - Send keys to terminal.
pub async fn send_keys(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SendKeysRequest>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::SendKeys {
            keys: req.keys,
            tab: req.tab,
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

/// GET /terminal/buffer - Get terminal buffer.
pub async fn get_buffer(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<BufferQuery>,
) -> Result<Json<TerminalBufferResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetBuffer {
            lines: query.lines,
            offset: query.offset,
            tab: query.tab,
            response: tx,
        }))
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

    Ok(Json(TerminalBufferResponse {
        lines: result.lines,
        cursor: result
            .cursor
            .map(|(line, col)| CursorPosition { line, col }),
        size: TerminalSize {
            cols: result.cols,
            rows: result.rows,
        },
    }))
}

/// GET /terminal/size - Get terminal size.
pub async fn get_size(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SizeQuery>,
) -> Result<Json<TerminalSize>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetSize {
            tab: query.tab,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let (cols, rows) = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(TerminalSize { cols, rows }))
}

/// GET /terminal/cursor - Get terminal cursor position.
pub async fn get_cursor(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SizeQuery>,
) -> Result<Json<TerminalCursorResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetCursor {
            tab: query.tab,
            response: tx,
        }))
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

    Ok(Json(TerminalCursorResponse {
        line: result.line,
        col: result.col,
        visible: result.visible,
    }))
}

/// GET /terminal/title - Get terminal title.
pub async fn get_title(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SizeQuery>,
) -> Result<Json<TerminalTitleResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetTitle {
            tab: query.tab,
            response: tx,
        }))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("Failed to send request")),
            )
        })?;

    let title = rx.await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("Failed to receive response")),
        )
    })?;

    Ok(Json(TerminalTitleResponse { title }))
}

/// POST /terminal/clear - Clear terminal.
pub async fn clear(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ClearTerminalRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::Clear {
            tab: req.tab,
            scrollback: req.scrollback.unwrap_or(false),
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

/// GET /terminal/scrollback - Get scrollback buffer.
pub async fn get_scrollback(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ScrollbackQuery>,
) -> Result<Json<ScrollbackResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetScrollback {
            tab: query.tab,
            limit: query.limit,
            offset: query.offset,
            response: tx,
        }))
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

    Ok(Json(ScrollbackResponse {
        lines: result.lines,
        total_lines: result.total_lines,
    }))
}

/// GET /terminal/selection - Get terminal selection.
pub async fn get_selection(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SizeQuery>,
) -> Result<Json<SelectionResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::GetSelection {
            tab: query.tab,
            response: tx,
        }))
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

    Ok(Json(SelectionResponse {
        text: result.text,
        start: result.start.map(|(line, col)| CursorPosition { line, col }),
        end: result.end.map(|(line, col)| CursorPosition { line, col }),
    }))
}

/// POST /terminal/scroll - Scroll terminal.
pub async fn scroll(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ScrollRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .request_tx
        .send(AppRequest::Terminal(TerminalRequest::Scroll {
            tab: req.tab,
            lines: req.lines,
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
