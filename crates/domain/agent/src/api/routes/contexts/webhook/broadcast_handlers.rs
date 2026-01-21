use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use systemprompt_events::EventRouter;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use super::types::{A2ABroadcastRequest, AgUiBroadcastRequest};

pub async fn broadcast_a2a_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(_app_context): State<AppContext>,
    Json(request): Json<A2ABroadcastRequest>,
) -> Response {
    let authenticated_user_id = &req_ctx.auth.user_id;
    let request_user_id = UserId::new(&request.user_id);
    let event_type = request.event.event_type();

    tracing::debug!(event_type = ?event_type, user_id = %request_user_id, auth_user_id = %authenticated_user_id, "Received event");

    if authenticated_user_id != &request_user_id {
        tracing::warn!(auth_user_id = %authenticated_user_id, request_user_id = %request_user_id, "User ID mismatch");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "User ID mismatch",
                "message": "Authenticated user does not match the request user_id"
            })),
        )
            .into_response();
    }

    let (a2a_count, context_count) = EventRouter::route_a2a(&request_user_id, request.event).await;
    let count = a2a_count + context_count;

    tracing::debug!(event_type = ?event_type, count = %count, user_id = %request.user_id, "Event broadcasted to connections");

    (
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count
        })),
    )
        .into_response()
}

pub async fn broadcast_agui_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(_app_context): State<AppContext>,
    Json(request): Json<AgUiBroadcastRequest>,
) -> Response {
    let authenticated_user_id = &req_ctx.auth.user_id;
    let request_user_id = UserId::new(&request.user_id);
    let event_type = request.event.event_type();

    tracing::debug!(event_type = ?event_type, user_id = %request_user_id, auth_user_id = %authenticated_user_id, "Received event");

    if authenticated_user_id != &request_user_id {
        tracing::warn!(auth_user_id = %authenticated_user_id, request_user_id = %request_user_id, "User ID mismatch");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "User ID mismatch",
                "message": "Authenticated user does not match the request user_id"
            })),
        )
            .into_response();
    }

    let (agui_count, context_count) =
        EventRouter::route_agui(&request_user_id, request.event).await;
    let count = agui_count + context_count;

    tracing::debug!(event_type = ?event_type, count = %count, user_id = %request.user_id, "Event broadcasted to connections");

    (
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count
        })),
    )
        .into_response()
}
