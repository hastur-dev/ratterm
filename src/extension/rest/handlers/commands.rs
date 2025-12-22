//! Command-related REST API handlers.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::extension::rest::{
    state::ApiState,
    types::{
        ApiError, CommandIdQuery, CommandInfo, CommandListResponse, ExecuteCommandRequest,
        ExecuteCommandResponse, RegisterCommandRequest, SuccessResponse,
    },
};

/// POST /commands/register - Register a command.
pub async fn register(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<RegisterCommandRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let mut commands = state.commands.write().await;

    commands.register(
        req.id,
        req.name,
        req.description,
        req.callback_url,
        "api".to_string(), // Source is API extension
    );

    Ok(Json(SuccessResponse { success: true }))
}

/// DELETE /commands/unregister - Unregister a command.
pub async fn unregister(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CommandIdQuery>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ApiError>)> {
    let mut commands = state.commands.write().await;

    let success = commands.unregister(&query.id);

    Ok(Json(SuccessResponse { success }))
}

/// GET /commands/list - List all registered commands.
pub async fn list(State(state): State<Arc<ApiState>>) -> Json<CommandListResponse> {
    let commands = state.commands.read().await;

    let command_list = commands
        .list()
        .into_iter()
        .map(|cmd| CommandInfo {
            id: cmd.id.clone(),
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            source: cmd.source.clone(),
        })
        .collect();

    Json(CommandListResponse {
        commands: command_list,
    })
}

/// POST /commands/execute - Execute a command.
pub async fn execute(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ExecuteCommandRequest>,
) -> Result<Json<ExecuteCommandResponse>, (StatusCode, Json<ApiError>)> {
    let commands = state.commands.read().await;

    let cmd = commands.get(&req.id).cloned();
    drop(commands); // Release the lock

    let Some(cmd) = cmd else {
        return Ok(Json(ExecuteCommandResponse {
            success: false,
            result: None,
            error: Some(format!("Command '{}' not found", req.id)),
        }));
    };

    // If command has a callback URL, call it
    if let Some(callback_url) = cmd.callback_url {
        // Make HTTP request to the extension's callback URL
        let client = reqwest::Client::new();

        let body = serde_json::json!({
            "command": cmd.id,
            "args": req.args.unwrap_or_default(),
        });

        match client.post(&callback_url).json(&body).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let result: serde_json::Value =
                        response.json().await.unwrap_or(serde_json::Value::Null);

                    Ok(Json(ExecuteCommandResponse {
                        success: true,
                        result: Some(result),
                        error: None,
                    }))
                } else {
                    Ok(Json(ExecuteCommandResponse {
                        success: false,
                        result: None,
                        error: Some(format!(
                            "Callback returned status: {}",
                            response.status()
                        )),
                    }))
                }
            }
            Err(e) => Ok(Json(ExecuteCommandResponse {
                success: false,
                result: None,
                error: Some(format!("Failed to call callback: {}", e)),
            })),
        }
    } else {
        // No callback URL, command is registered but has no handler
        Ok(Json(ExecuteCommandResponse {
            success: false,
            result: None,
            error: Some("Command has no callback URL".to_string()),
        }))
    }
}
