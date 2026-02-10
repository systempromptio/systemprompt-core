use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use systemprompt_events::EventRouter;
use systemprompt_models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_runtime::{create_request_span, AppContext};

use super::event_loader::load_event_data;
use super::types::WebhookRequest;
use systemprompt_agent::repository::context::ContextRepository;

pub async fn broadcast_context_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(app_context): State<AppContext>,
    Json(request): Json<WebhookRequest>,
) -> Response {
    let start_time = std::time::Instant::now();
    let db = app_context.db_pool();
    let span = create_request_span(&req_ctx);
    if matches!(
        request.event_type.as_str(),
        "task_completed" | "task_created"
    ) {
        span.record_task_id(&systemprompt_identifiers::TaskId::new(
            request.entity_id.clone(),
        ));
    }

    let authenticated_user_id = &req_ctx.auth.user_id;

    if authenticated_user_id.as_str() != request.user_id {
        tracing::error!(jwt_user_id = %authenticated_user_id, payload_user_id = %request.user_id, context_id = %request.context_id, "User mismatch");

        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "User ID mismatch",
                "message": "Authenticated user does not match the request user_id"
            })),
        )
            .into_response();
    }

    let context_id = request.context_id.clone();
    let context_repo = match ContextRepository::new(db) {
        Ok(repo) => repo,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Database error",
                    "message": format!("{e}")
                })),
            )
                .into_response();
        },
    };
    if let Err(e) = context_repo
        .validate_context_ownership(&context_id, authenticated_user_id)
        .await
    {
        tracing::error!(error = %e, context_id = %request.context_id, user_id = %authenticated_user_id, "Context ownership validation failed");

        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Context ownership validation failed",
                "message": format!("User does not own context: {e}")
            })),
        )
            .into_response();
    }

    tracing::debug!(event_type = %request.event_type, entity_id = %request.entity_id, context_id = %request.context_id, user_id = %request.user_id, "Webhook received");

    let webhook_data = match load_event_data(&app_context, &request).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(error = %e, event_type = %request.event_type, entity_id = %request.entity_id, "Failed to load event data");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to load event data",
                    "details": e.to_string()
                })),
            )
                .into_response();
        },
    };

    let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
        name: webhook_data.event_name.clone(),
        value: webhook_data.payload,
    }));

    let (agui_count, context_count) = EventRouter::route_agui(authenticated_user_id, event).await;
    let count = agui_count + context_count;

    tracing::debug!(event_type = %webhook_data.event_name, connection_count = %count, user_id = %request.user_id, duration_ms = %start_time.elapsed().as_millis(), "Webhook processed");

    (
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count,
            "event_type": webhook_data.event_name
        })),
    )
        .into_response()
}
