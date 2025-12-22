//! Filesystem-related REST API handlers.
//!
//! Note: These handlers operate directly on the filesystem without going through
//! the App thread, since they don't require access to App state.

use std::path::Path;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};

use crate::extension::rest::{
    state::ApiState,
    types::{
        ApiError, BoolResponse, CopyRequest, DirEntry, ListDirResponse, MkdirRequest, PathQuery,
        ReadFileResponse, RenameRequest, SuccessResponse, WriteFileRequest,
    },
};

/// GET /fs/read - Read file content.
pub async fn read_file(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<ReadFileResponse>, (StatusCode, Json<ApiError>)> {
    let content = std::fs::read_to_string(&query.path).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(format!("Failed to read file: {}", e))),
        )
    })?;

    Ok(Json(ReadFileResponse { content }))
}

/// POST /fs/write - Write file content.
pub async fn write_file(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<WriteFileRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    std::fs::write(&req.path, &req.content).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(format!("Failed to write file: {}", e))),
        )
    })?;

    Ok(Json(SuccessResponse { success: true }))
}

/// GET /fs/exists - Check if path exists.
pub async fn exists(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Json<BoolResponse> {
    let path = Path::new(&query.path);
    Json(BoolResponse {
        result: path.exists(),
    })
}

/// GET /fs/is_dir - Check if path is a directory.
pub async fn is_dir(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Json<BoolResponse> {
    let path = Path::new(&query.path);
    Json(BoolResponse {
        result: path.is_dir(),
    })
}

/// GET /fs/is_file - Check if path is a file.
pub async fn is_file(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Json<BoolResponse> {
    let path = Path::new(&query.path);
    Json(BoolResponse {
        result: path.is_file(),
    })
}

/// GET /fs/list_dir - List directory contents.
pub async fn list_dir(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<ListDirResponse>, (StatusCode, Json<ApiError>)> {
    let path = Path::new(&query.path);

    let entries = std::fs::read_dir(path)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(format!("Failed to read directory: {}", e))),
            )
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().to_string();

            Some(DirEntry {
                name,
                is_dir: metadata.is_dir(),
                is_file: metadata.is_file(),
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
            })
        })
        .collect();

    Ok(Json(ListDirResponse { entries }))
}

/// POST /fs/mkdir - Create directory.
pub async fn mkdir(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<MkdirRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    std::fs::create_dir_all(&req.path).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(format!("Failed to create directory: {}", e))),
        )
    })?;

    Ok(Json(SuccessResponse { success: true }))
}

/// DELETE /fs/remove - Remove file or directory.
pub async fn remove(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let path = Path::new(&query.path);

    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(format!("Failed to remove directory: {}", e))),
            )
        })?;
    } else {
        std::fs::remove_file(path).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(format!("Failed to remove file: {}", e))),
            )
        })?;
    }

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /fs/rename - Rename or move file.
pub async fn rename(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    std::fs::rename(&req.from, &req.to).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(format!("Failed to rename: {}", e))),
        )
    })?;

    Ok(Json(SuccessResponse { success: true }))
}

/// POST /fs/copy - Copy file.
pub async fn copy(
    State(_state): State<Arc<ApiState>>,
    Json(req): Json<CopyRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    std::fs::copy(&req.from, &req.to).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(format!("Failed to copy: {}", e))),
        )
    })?;

    Ok(Json(SuccessResponse { success: true }))
}
