use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use systemprompt_core_analytics::{CreateEngagementEventInput, EngagementRepository};
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;

#[derive(Debug, Deserialize)]
pub struct EngagementBatchInput {
    pub events: Vec<CreateEngagementEventInput>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BatchResponse {
    pub recorded: usize,
}

#[derive(Clone, Debug)]
pub struct EngagementState {
    pub repo: Arc<EngagementRepository>,
}

pub async fn record_engagement(
    State(state): State<EngagementState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(input): Json<CreateEngagementEventInput>,
) -> Result<StatusCode, ApiError> {
    state
        .repo
        .create_engagement(
            req_ctx.session_id().as_str(),
            req_ctx.user_id().as_str(),
            &input,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to record engagement");
            ApiError::internal_error("Failed to record engagement")
        })?;

    Ok(StatusCode::CREATED)
}

pub async fn record_engagement_batch(
    State(state): State<EngagementState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(input): Json<EngagementBatchInput>,
) -> impl IntoResponse {
    let session_id = req_ctx.session_id();
    let user_id = req_ctx.user_id();

    let mut success_count = 0;
    for event in input.events {
        if state
            .repo
            .create_engagement(session_id.as_str(), user_id.as_str(), &event)
            .await
            .is_ok()
        {
            success_count += 1;
        }
    }

    Json(BatchResponse {
        recorded: success_count,
    })
}
