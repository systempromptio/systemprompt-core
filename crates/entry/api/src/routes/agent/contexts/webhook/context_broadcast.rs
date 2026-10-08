//! Context-scoped webhook broadcast with authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use systemprompt_models::api::ApiError;
use systemprompt_models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_runtime::AppContext;

use super::event_loader::load_event_data;
use super::types::WebhookRequest;
use crate::error::ApiHttpError;

async fn authorize_broadcast(
    repos: &systemprompt_agent::repository::A2ARepositories,
    req_ctx: &systemprompt_models::RequestContext,
    request: &WebhookRequest,
) -> Result<(), ApiHttpError> {
    let authenticated_user_id = &req_ctx.auth.actor.user_id;

    if *authenticated_user_id != request.user_id {
        return Err(ApiHttpError::forbidden(
            "Authenticated user does not match the request user_id",
        ));
    }

    match repos
        .contexts
        .validate_context_ownership(&request.context_id, authenticated_user_id)
        .await
    {
        Ok(()) => Ok(()),
        Err(e) if e.is_not_found() => Err(ApiHttpError::from(
            ApiError::forbidden("User does not own the context").with_source(e),
        )),
        Err(e) => Err(ApiHttpError::from(e)),
    }
}

pub async fn broadcast_context_event(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(app_context): State<AppContext>,
    Json(request): Json<WebhookRequest>,
) -> Result<Response, ApiHttpError> {
    let start_time = std::time::Instant::now();

    authorize_broadcast(app_context.a2a_repositories(), &req_ctx, &request).await?;

    tracing::debug!(event_type = %request.event_type, entity_id = %request.entity_id, context_id = %request.context_id, user_id = %request.user_id, "Webhook received");

    let webhook_data = load_event_data(&app_context, &request).await?;

    let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
        name: webhook_data.event_name.clone(),
        value: webhook_data.payload,
    }));

    let (agui_count, context_count) = app_context
        .event_router()
        .route_agui(&req_ctx.auth.actor.user_id, event)
        .await
        .into_local_logged();
    let count = agui_count + context_count;

    tracing::debug!(event_type = %webhook_data.event_name, connection_count = %count, user_id = %request.user_id, duration_ms = %start_time.elapsed().as_millis(), "Webhook processed");

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "broadcasted",
            "connection_count": count,
            "event_type": webhook_data.event_name
        })),
    )
        .into_response())
}
