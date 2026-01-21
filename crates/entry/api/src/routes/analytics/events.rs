use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

use systemprompt_analytics::{
    AnalyticsEventBatchResponse, AnalyticsEventsRepository, CreateAnalyticsEventBatchInput,
    CreateAnalyticsEventInput,
};
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::ContentId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;

#[derive(Clone, Debug)]
pub struct AnalyticsState {
    pub events_repo: Arc<AnalyticsEventsRepository>,
    pub content_repo: Arc<ContentRepository>,
}

fn extract_slug_from_url(page_url: &str) -> Option<&str> {
    page_url
        .strip_prefix("/blog/")
        .or_else(|| page_url.strip_prefix("/article/"))
        .or_else(|| page_url.strip_prefix("/guide/"))
        .or_else(|| page_url.strip_prefix("/paper/"))
        .or_else(|| page_url.strip_prefix("/docs/"))
        .map(|s| s.split('?').next().unwrap_or(s))
        .map(|s| s.split('#').next().unwrap_or(s))
        .map(|s| s.trim_end_matches('/'))
}

async fn resolve_content_id(
    content_repo: &ContentRepository,
    page_url: &str,
    slug: Option<&str>,
) -> Option<ContentId> {
    let slug_to_use = slug.or_else(|| extract_slug_from_url(page_url))?;

    content_repo
        .get_by_slug(slug_to_use)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, slug = %slug_to_use, "Failed to lookup content by slug");
            e
        })
        .ok()
        .flatten()
        .map(|c| c.id)
}

pub async fn record_event(
    State(state): State<AnalyticsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(mut input): Json<CreateAnalyticsEventInput>,
) -> Result<impl IntoResponse, ApiError> {
    if input.content_id.is_none() {
        input.content_id =
            resolve_content_id(&state.content_repo, &input.page_url, input.slug.as_deref()).await;
    }

    let created = state
        .events_repo
        .create_event(
            req_ctx.session_id().as_str(),
            req_ctx.user_id().as_str(),
            &input,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to record analytics event");
            ApiError::internal_error("Failed to record analytics event")
        })?;

    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn record_events_batch(
    State(state): State<AnalyticsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(mut input): Json<CreateAnalyticsEventBatchInput>,
) -> Result<impl IntoResponse, ApiError> {
    for event in &mut input.events {
        if event.content_id.is_none() {
            event.content_id =
                resolve_content_id(&state.content_repo, &event.page_url, event.slug.as_deref())
                    .await;
        }
    }

    let created = state
        .events_repo
        .create_events_batch(
            req_ctx.session_id().as_str(),
            req_ctx.user_id().as_str(),
            &input.events,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to record analytics events batch");
            ApiError::internal_error("Failed to record analytics events")
        })?;

    Ok((
        StatusCode::CREATED,
        Json(AnalyticsEventBatchResponse {
            recorded: created.len(),
            events: created,
        }),
    ))
}
