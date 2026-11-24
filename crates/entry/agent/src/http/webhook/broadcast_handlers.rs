//! Webhook broadcast endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use systemprompt_models::api::ApiError;

use systemprompt_extension::router::ApiHttpError;

use super::types::{A2ABroadcastRequest, AgUiBroadcastRequest};
use crate::http::AgentHttpState;

fn user_mismatch() -> ApiHttpError {
    ApiError::forbidden("Authenticated user does not match the request user_id")
        .with_error_key("user_id_mismatch")
        .into()
}

pub async fn broadcast_a2a_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(state): State<AgentHttpState>,
    Json(request): Json<A2ABroadcastRequest>,
) -> Result<Response, ApiHttpError> {
    let authenticated_user_id = &req_ctx.auth.actor.user_id;
    let request_user_id = request.user_id;
    let event_type = request.event.event_type();

    tracing::debug!(event_type = ?event_type, user_id = %request_user_id, auth_user_id = %authenticated_user_id, "Received event");

    if authenticated_user_id != &request_user_id {
        tracing::warn!(auth_user_id = %authenticated_user_id, request_user_id = %request_user_id, "User ID mismatch");
        return Err(user_mismatch());
    }

    let (a2a_count, context_count) = state.events.a2a(&request_user_id, request.event).await;
    let count = a2a_count + context_count;

    tracing::debug!(event_type = ?event_type, count = %count, user_id = %request_user_id, "Event broadcasted to connections");

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count
        })),
    )
        .into_response())
}

pub async fn broadcast_agui_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(state): State<AgentHttpState>,
    Json(request): Json<AgUiBroadcastRequest>,
) -> Result<Response, ApiHttpError> {
    let authenticated_user_id = &req_ctx.auth.actor.user_id;
    let request_user_id = request.user_id;
    let event_type = request.event.event_type();

    tracing::debug!(event_type = ?event_type, user_id = %request_user_id, auth_user_id = %authenticated_user_id, "Received event");

    if authenticated_user_id != &request_user_id {
        tracing::warn!(auth_user_id = %authenticated_user_id, request_user_id = %request_user_id, "User ID mismatch");
        return Err(user_mismatch());
    }

    let (agui_count, context_count) = state.events.agui(&request_user_id, request.event).await;
    let count = agui_count + context_count;

    tracing::debug!(event_type = ?event_type, count = %count, user_id = %request_user_id, "Event broadcasted to connections");

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count
        })),
    )
        .into_response())
}
