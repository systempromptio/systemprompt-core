//! Inbound A2A notification handling for a context.
//!
//! Validates the JSON-RPC envelope, resolves the owning user, then persists,
//! processes, and broadcasts the notification to the context's live streams.

mod error;
mod handlers;

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;
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
) -> Result<Response, ApiHttpError> {
    let db = app_context.db_pool();

    let ctx_repo = ContextRepository::new(db)?;
    let context_id = ContextId::new(context_id);

    tracing::debug!(context_id = %context_id, method = %notification.method, "Received notification for context");

    let user_id = match resolve_context_user(&ctx_repo, &context_id).await {
        Ok(uid) => uid,
        Err(response) => return Ok(response),
    };

    if notification.jsonrpc != "2.0" {
        tracing::error!(jsonrpc_version = %notification.jsonrpc, "Invalid JSON-RPC version");

        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid JSON-RPC version, must be 2.0"})),
        )
            .into_response());
    }

    let agent_id = notification
        .params
        .get("agentId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();

    let notification_id = persist_notification(
        Arc::clone(db),
        context_id.as_str(),
        &agent_id,
        &notification,
    )
    .await?;
    tracing::debug!(notification_id = %notification_id, context_id = %context_id, "Persisted notification");

    process_notification(app_context.clone(), &notification).await?;

    broadcast_and_mark(db, &context_id, &user_id, &notification, notification_id).await;

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "received",
            "notification_id": notification_id
        })),
    )
        .into_response())
}

async fn resolve_context_user(
    ctx_repo: &ContextRepository,
    context_id: &ContextId,
) -> Result<UserId, Response> {
    match ctx_repo.find_user_id_for_context(context_id).await {
        Ok(Some(uid)) => Ok(uid),
        Ok(None) => {
            tracing::error!(context_id = %context_id, "Context not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Context not found",
                    "context_id": context_id.as_str()
                })),
            )
                .into_response())
        },
        Err(e) => {
            tracing::error!(error = %e, context_id = %context_id, "Context not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Context not found",
                    "context_id": context_id.as_str()
                })),
            )
                .into_response())
        },
    }
}

async fn broadcast_and_mark(
    db: &DbPool,
    context_id: &ContextId,
    user_id: &UserId,
    notification: &A2aNotification,
    notification_id: i32,
) {
    let broadcast_count = broadcast_notification(context_id.as_str(), user_id, notification).await;
    tracing::debug!(broadcast_count = %broadcast_count, context_id = %context_id, "Broadcasted notification to streams");

    if let Err(e) = mark_notification_broadcasted(Arc::clone(db), notification_id).await {
        tracing::error!(error = %e, notification_id = %notification_id, "Failed to mark notification as broadcasted");
    }
}
