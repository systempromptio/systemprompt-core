mod handlers;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use handlers::{
    broadcast_notification, mark_notification_broadcasted, persist_notification,
    process_notification,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct A2aNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

pub async fn handle_context_notification(
    Path(context_id): Path<String>,
    State(app_context): State<AppContext>,
    Json(notification): Json<A2aNotification>,
) -> Response {
    let db = app_context.db_pool();

    let pool = match db.pool_arc() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        },
    };

    tracing::debug!(context_id = %context_id, method = %notification.method, "Received notification for context");

    let user_id = match sqlx::query_scalar::<_, String>(
        "SELECT user_id FROM user_contexts WHERE context_id = $1",
    )
    .bind(&context_id)
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(uid)) => UserId::new(uid),
        Ok(None) => {
            tracing::error!(context_id = %context_id, "Context not found");
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Context not found",
                    "context_id": context_id
                })),
            )
                .into_response();
        },
        Err(e) => {
            tracing::error!(error = %e, context_id = %context_id, "Context not found");
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Context not found",
                    "context_id": context_id
                })),
            )
                .into_response();
        },
    };

    if notification.jsonrpc != "2.0" {
        tracing::error!(jsonrpc_version = %notification.jsonrpc, "Invalid JSON-RPC version");

        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid JSON-RPC version, must be 2.0"})),
        )
            .into_response();
    }

    let agent_id = notification
        .params
        .get("agentId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    match persist_notification(db.clone(), &context_id, &agent_id, &notification).await {
        Ok(notification_id) => {
            tracing::debug!(notification_id = %notification_id, context_id = %context_id, "Persisted notification");

            match process_notification(app_context.clone(), &notification).await {
                Ok(()) => {
                    match broadcast_notification(&context_id, &user_id, &notification).await {
                        Ok(broadcast_count) => {
                            tracing::debug!(broadcast_count = %broadcast_count, context_id = %context_id, "Broadcasted notification to streams");

                            if let Err(e) =
                                mark_notification_broadcasted(db.clone(), notification_id).await
                            {
                                tracing::error!(error = %e, notification_id = %notification_id, "Failed to mark notification as broadcasted");
                            }
                        },
                        Err(e) => {
                            tracing::error!(error = %e, notification_id = %notification_id, "Failed to broadcast notification");
                        },
                    }

                    (
                        StatusCode::OK,
                        Json(json!({
                            "status": "received",
                            "notification_id": notification_id
                        })),
                    )
                        .into_response()
                },
                Err(e) => {
                    tracing::error!(error = %e, notification_id = %notification_id, "Failed to process notification");

                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to process notification",
                            "details": e.to_string()
                        })),
                    )
                        .into_response()
                },
            }
        },
        Err(e) => {
            tracing::error!(error = %e, context_id = %context_id, "Failed to persist notification");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to persist notification",
                    "details": e.to_string()
                })),
            )
                .into_response()
        },
    }
}
