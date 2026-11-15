//! Engagement-event ingestion handlers with conversion marking.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use systemprompt_analytics::CreateEngagementEventInput;
use systemprompt_extension::router::ApiHttpError;
use systemprompt_identifiers::SessionId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_traits::SessionStore;

use crate::http::EngagementState;

const CONVERSION_EVENT_TYPES: &[&str] = &[
    "github_click",
    "evaluate_click",
    "demo_click",
    "demo_site_click",
    "pricing_click",
];

#[derive(Debug, Deserialize)]
pub struct EngagementBatchInput {
    pub events: Vec<CreateEngagementEventInput>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BatchResponse {
    pub recorded: usize,
}

async fn mark_converted_if_applicable(
    sessions: &dyn SessionStore,
    session_id: &SessionId,
    event_type: &str,
) {
    if !CONVERSION_EVENT_TYPES.contains(&event_type) {
        return;
    }
    if let Err(e) = sessions.mark_converted(session_id).await {
        tracing::warn!(error = %e, session_id = %session_id.as_str(), "Failed to mark session converted");
    }
}

pub(crate) async fn record_engagement(
    State(state): State<EngagementState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(input): Json<CreateEngagementEventInput>,
) -> Result<StatusCode, ApiHttpError> {
    let content_id = state.content.resolve(&input.page_url, None).await;

    state
        .repositories
        .engagement
        .create_engagement(
            req_ctx.session_id(),
            req_ctx.user_id(),
            content_id.as_ref(),
            &input,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to record engagement");
            ApiError::internal("Failed to record engagement", e)
        })?;

    mark_converted_if_applicable(
        &*state.repositories.session_store,
        req_ctx.session_id(),
        &input.event_type,
    )
    .await;

    Ok(StatusCode::CREATED)
}

pub(crate) async fn record_engagement_batch(
    State(state): State<EngagementState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(input): Json<EngagementBatchInput>,
) -> impl IntoResponse {
    let session_id = req_ctx.session_id();
    let user_id = req_ctx.user_id();

    let mut success_count = 0;
    for event in input.events {
        let content_id = state.content.resolve(&event.page_url, None).await;

        match state
            .repositories
            .engagement
            .create_engagement(session_id, user_id, content_id.as_ref(), &event)
            .await
        {
            Ok(_) => {
                success_count += 1;
                mark_converted_if_applicable(
                    &*state.repositories.session_store,
                    session_id,
                    &event.event_type,
                )
                .await;
            },
            Err(e) => {
                tracing::warn!(error = %e, page_url = %event.page_url, "Failed to record batch engagement event");
            },
        }
    }

    Json(BatchResponse {
        recorded: success_count,
    })
}
