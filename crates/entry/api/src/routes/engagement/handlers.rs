use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use systemprompt_analytics::{CreateEngagementEventInput, EngagementRepository};
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::ContentId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::ContentRouting;

#[derive(Debug, Deserialize)]
pub struct EngagementBatchInput {
    pub events: Vec<CreateEngagementEventInput>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BatchResponse {
    pub recorded: usize,
}

#[derive(Clone)]
pub struct EngagementState {
    pub repo: Arc<EngagementRepository>,
    pub content_repo: Arc<ContentRepository>,
    pub content_routing: Option<Arc<dyn ContentRouting>>,
}

impl std::fmt::Debug for EngagementState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngagementState")
            .field("content_routing", &self.content_routing.is_some())
            .finish_non_exhaustive()
    }
}

async fn resolve_content_id(
    content_repo: &ContentRepository,
    content_routing: Option<&dyn ContentRouting>,
    page_url: &str,
) -> Option<ContentId> {
    let slug = content_routing.and_then(|r| r.resolve_slug(page_url))?;

    content_repo
        .get_by_slug(&slug)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, slug = %slug, "Failed to lookup content by slug");
            e
        })
        .ok()
        .flatten()
        .map(|c| c.id)
}

pub async fn record_engagement(
    State(state): State<EngagementState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(input): Json<CreateEngagementEventInput>,
) -> Result<StatusCode, ApiError> {
    let content_id = resolve_content_id(
        &state.content_repo,
        state.content_routing.as_deref(),
        &input.page_url,
    )
    .await;

    state
        .repo
        .create_engagement(
            req_ctx.session_id().as_str(),
            req_ctx.user_id().as_str(),
            content_id.as_ref(),
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
        let content_id = resolve_content_id(
            &state.content_repo,
            state.content_routing.as_deref(),
            &event.page_url,
        )
        .await;

        match state
            .repo
            .create_engagement(
                session_id.as_str(),
                user_id.as_str(),
                content_id.as_ref(),
                &event,
            )
            .await
        {
            Ok(_) => success_count += 1,
            Err(e) => {
                tracing::warn!(error = %e, page_url = %event.page_url, "Failed to record batch engagement event");
            },
        }
    }

    Json(BatchResponse {
        recorded: success_count,
    })
}
