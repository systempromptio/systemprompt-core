use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_core_events::EventRouter;
use systemprompt_identifiers::UserId;
use systemprompt_models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_runtime::AppContext;

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

async fn persist_notification(
    db: systemprompt_core_database::DbPool,
    context_id: &str,
    agent_id: &str,
    notification: &A2aNotification,
) -> Result<i32, anyhow::Error> {
    let pool = db.pool_arc()?;
    let notification_data =
        serde_json::to_value(notification).map_err(|e| anyhow::anyhow!("{}", e))?;

    let result = sqlx::query!(
        r#"INSERT INTO context_notifications (context_id, agent_id, notification_type, notification_data)
        VALUES ($1, $2, $3, $4)
        RETURNING id"#,
        context_id,
        agent_id,
        notification.method,
        notification_data
    )
    .fetch_one(pool.as_ref())
    .await?;

    Ok(result.id)
}

async fn process_notification(
    app_context: AppContext,
    notification: &A2aNotification,
) -> Result<(), anyhow::Error> {
    let db = app_context.db_pool();
    let pool = db.pool_arc()?;

    match notification.method.as_str() {
        "notifications/taskStatusUpdate" => {
            let task_id = notification
                .params
                .get("taskId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing taskId in notification"))?;

            let status = notification
                .params
                .get("status")
                .ok_or_else(|| anyhow::anyhow!("Missing status in notification"))?;

            let state = status
                .get("state")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing state in status"))?;

            let timestamp = status
                .get("timestamp")
                .and_then(systemprompt_core_database::parse_database_datetime)
                .unwrap_or_else(Utc::now);

            if state == "completed" {
                sqlx::query!(
                    r#"UPDATE agent_tasks SET
                        status = 'completed',
                        updated_at = $1,
                        completed_at = CURRENT_TIMESTAMP,
                        started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
                        execution_time_ms = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - COALESCE(started_at, CURRENT_TIMESTAMP))) * 1000
                    WHERE task_id = $2"#,
                    timestamp,
                    task_id
                )
                .execute(pool.as_ref())
                .await?;
            } else {
                sqlx::query!(
                    "UPDATE agent_tasks SET status = $1, updated_at = $2 WHERE task_id = $3",
                    state,
                    timestamp,
                    task_id
                )
                .execute(pool.as_ref())
                .await?;
            }

            Ok(())
        },
        _ => Ok(()),
    }
}

async fn broadcast_notification(
    context_id: &str,
    user_id: &UserId,
    notification: &A2aNotification,
) -> Result<usize, anyhow::Error> {
    let mut total_broadcasts = 0;

    match notification.method.as_str() {
        "notifications/taskStatusUpdate" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "task_status_changed".to_string(),
                value: json!({
                    "contextId": context_id,
                    "taskId": notification.params.get("taskId"),
                    "status": notification.params.get("status"),
                    "task": notification.params.get("task"),
                }),
            }));

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
            total_broadcasts += agui + ctx;
        },
        "notifications/artifactCreated" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "artifact".to_string(),
                value: json!({
                    "artifact": notification.params.get("artifact"),
                    "taskId": notification.params.get("taskId"),
                    "contextId": context_id,
                }),
            }));

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
            total_broadcasts += agui + ctx;
        },
        "notifications/messageAdded" => {
            let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
                name: "message_added".to_string(),
                value: json!({
                    "contextId": context_id,
                    "messageId": notification.params.get("messageId"),
                    "message": notification.params.get("message"),
                }),
            }));

            let (agui, ctx) = EventRouter::route_agui(user_id, event).await;
            total_broadcasts += agui + ctx;
        },
        _ => {},
    }

    Ok(total_broadcasts)
}

async fn mark_notification_broadcasted(
    db: systemprompt_core_database::DbPool,
    notification_id: i32,
) -> Result<(), anyhow::Error> {
    let pool = db.pool_arc()?;
    sqlx::query!(
        "UPDATE context_notifications SET broadcasted = true WHERE id = $1",
        notification_id
    )
    .execute(pool.as_ref())
    .await?;

    Ok(())
}
