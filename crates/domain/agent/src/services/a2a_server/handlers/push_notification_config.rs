use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use std::sync::Arc;

use crate::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    SetTaskPushNotificationConfigRequest,
};
use crate::repository::content::PushNotificationConfigRepository;
use crate::services::a2a_server::handlers::AgentHandlerState;

pub async fn handle_set_push_notification_config(
    State(state): State<Arc<AgentHandlerState>>,
    request: SetTaskPushNotificationConfigRequest,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(task_id = %request.task_id, "Setting push notification config");

    let repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to create repository");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to create repository",
                        "data": format!("{e}")
                    }
                })),
            ));
        },
    };

    let task_id = systemprompt_identifiers::TaskId::new(&request.task_id);
    match repo.add_config(&task_id, &request.config).await {
        Ok(config_id) => {
            tracing::info!(config_id = %config_id, task_id = %request.task_id, "Successfully added config");

            Ok((
                StatusCode::OK,
                Json(json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "success": true,
                        "configId": config_id,
                        "message": "Push notification config added successfully"
                    }
                })),
            ))
        },
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to add config");

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to add push notification config",
                        "data": format!("{e}")
                    }
                })),
            ))
        },
    }
}

pub async fn handle_get_push_notification_config(
    State(state): State<Arc<AgentHandlerState>>,
    request: GetTaskPushNotificationConfigRequest,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(task_id = %request.task_id, "Getting push notification config");

    let repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to create repository");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to create repository",
                        "data": format!("{e}")
                    }
                })),
            ));
        },
    };

    let task_id = systemprompt_identifiers::TaskId::new(&request.task_id);
    match repo.list_configs(&task_id).await {
        Ok(configs) => Ok((
            StatusCode::OK,
            Json(json!({
                "jsonrpc": "2.0",
                "result": {
                    "configs": configs
                }
            })),
        )),
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to get configs");

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to get push notification configs",
                        "data": format!("{e}")
                    }
                })),
            ))
        },
    }
}

pub async fn handle_list_push_notification_configs(
    State(state): State<Arc<AgentHandlerState>>,
    task_id: String,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(task_id = %task_id, "Listing push notification configs");

    let repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::error!(task_id = %task_id, error = %e, "Failed to create repository");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to create repository",
                        "data": format!("{e}")
                    }
                })),
            ));
        },
    };
    let task_id_typed = systemprompt_identifiers::TaskId::new(&task_id);

    match repo.list_configs(&task_id_typed).await {
        Ok(configs) => {
            let total = configs.len() as u32;

            Ok((
                StatusCode::OK,
                Json(json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "configs": configs,
                        "total": total
                    }
                })),
            ))
        },
        Err(e) => {
            tracing::error!(task_id = %task_id, error = %e, "Failed to list configs");

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to list push notification configs",
                        "data": format!("{e}")
                    }
                })),
            ))
        },
    }
}

pub async fn handle_delete_push_notification_config(
    State(state): State<Arc<AgentHandlerState>>,
    request: DeleteTaskPushNotificationConfigRequest,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(task_id = %request.task_id, "Deleting push notification config");

    let repo = match PushNotificationConfigRepository::new(state.db_pool.clone()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to create repository");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to create repository",
                        "data": format!("{e}")
                    }
                })),
            ));
        },
    };
    let task_id = systemprompt_identifiers::TaskId::new(&request.task_id);

    match repo.delete_all_for_task(&task_id).await {
        Ok(count) => {
            tracing::info!(task_id = %request.task_id, deleted = count, "Successfully deleted configs");

            Ok((
                StatusCode::OK,
                Json(json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "success": true,
                        "deleted": count,
                        "message": format!("Deleted {} push notification config(s)", count)
                    }
                })),
            ))
        },
        Err(e) => {
            tracing::error!(task_id = %request.task_id, error = %e, "Failed to delete configs");

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": "Failed to delete push notification configs",
                        "data": format!("{e}")
                    }
                })),
            ))
        },
    }
}
