//! Inbound A2A notification handling for a context.
//!
//! Validates the JSON-RPC envelope and that the caller owns the context; a
//! status update must name a task the caller owns inside that context. Only
//! then is the notification persisted, applied and broadcast to the caller's
//! live streams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod handlers;


use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;
use crate::routes::agent::parse_context_id;
use handlers::{
    apply_status_update, broadcast_notification, mark_notification_broadcasted,
    persist_notification, resolve_status_update,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct A2aNotification {
    pub jsonrpc: String,
    pub method: String,
    // JSON: A2A JSON-RPC notification `params` — shape varies by `method`.
    pub params: serde_json::Value,
}

pub async fn handle_context_notification(
    Extension(req_ctx): Extension<RequestContext>,
    Path(context_id): Path<String>,
    State(app_context): State<AppContext>,
    Json(notification): Json<A2aNotification>,
) -> Result<Response, ApiHttpError> {
    let repos = app_context.a2a_repositories();
    let context_id = parse_context_id(&context_id)?;
    let user_id = req_ctx.user_id().clone();

    tracing::debug!(context_id = %context_id, method = %notification.method, "Received notification for context");

    if notification.jsonrpc != "2.0" {
        tracing::error!(jsonrpc_version = %notification.jsonrpc, "Invalid JSON-RPC version");

        return Err(ApiHttpError::bad_request(
            "Invalid JSON-RPC version, must be 2.0",
        ));
    }

    repos
        .contexts
        .validate_context_ownership(&context_id, &user_id)
        .await?;

    let status_update =
        resolve_status_update(&repos.tasks, &context_id, &user_id, &notification).await?;

    let agent_id = notification
        .params
        .get("agentId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();

    let notification_id = persist_notification(
        &repos.context_notifications,
        &context_id,
        &agent_id,
        &notification,
    )
    .await?;
    tracing::debug!(notification_id = %notification_id, context_id = %context_id, "Persisted notification");

    if let Some(update) = status_update {
        apply_status_update(&repos.tasks, &update).await?;
    }

    broadcast_and_mark(
        &app_context,
        &context_id,
        &user_id,
        &notification,
        notification_id,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "received",
            "notification_id": notification_id
        })),
    )
        .into_response())
}

async fn broadcast_and_mark(
    app_context: &AppContext,
    context_id: &ContextId,
    user_id: &UserId,
    notification: &A2aNotification,
    notification_id: i32,
) {
    let broadcast_count = broadcast_notification(
        app_context.event_router(),
        context_id.as_str(),
        user_id,
        notification,
    )
    .await;
    tracing::debug!(broadcast_count = %broadcast_count, context_id = %context_id, "Broadcasted notification to streams");

    let notifications_repo = &app_context.a2a_repositories().context_notifications;
    if let Err(e) = mark_notification_broadcasted(notifications_repo, notification_id).await {
        tracing::error!(error = %e, notification_id = %notification_id, "Failed to mark notification as broadcasted");
    }
}
