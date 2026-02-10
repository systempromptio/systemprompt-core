use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

use systemprompt_analytics::{
    AnalyticsEventBatchResponse, AnalyticsEventType, AnalyticsEventsRepository,
    CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput, CreateEngagementEventInput,
    EngagementOptionalMetrics, EngagementRepository,
};
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::ContentId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::ContentRouting;

#[derive(Clone)]
pub struct AnalyticsState {
    pub events: Arc<AnalyticsEventsRepository>,
    pub content: Arc<ContentRepository>,
    pub engagement: Arc<EngagementRepository>,
    pub content_routing: Option<Arc<dyn ContentRouting>>,
}

impl std::fmt::Debug for AnalyticsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsState")
            .field("content_routing", &self.content_routing.is_some())
            .finish_non_exhaustive()
    }
}

async fn resolve_content_id(
    content_repo: &ContentRepository,
    content_routing: Option<&dyn ContentRouting>,
    page_url: &str,
    slug: Option<&str>,
) -> Option<ContentId> {
    let resolved_slug;
    let slug_to_use = if let Some(s) = slug {
        s
    } else {
        resolved_slug = content_routing.and_then(|r| r.resolve_slug(page_url))?;
        &resolved_slug
    };

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
        input.content_id = resolve_content_id(
            &state.content,
            state.content_routing.as_deref(),
            &input.page_url,
            input.slug.as_deref(),
        )
        .await;
    }

    let created = state
        .events
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

    if input.event_type == AnalyticsEventType::PageExit {
        fan_out_engagement(&state, &req_ctx, &input).await;
    }

    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn record_events_batch(
    State(state): State<AnalyticsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(mut input): Json<CreateAnalyticsEventBatchInput>,
) -> Result<impl IntoResponse, ApiError> {
    for event in &mut input.events {
        if event.content_id.is_none() {
            event.content_id = resolve_content_id(
                &state.content,
                state.content_routing.as_deref(),
                &event.page_url,
                event.slug.as_deref(),
            )
            .await;
        }
    }

    let created = state
        .events
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

    for event in &input.events {
        if event.event_type == AnalyticsEventType::PageExit {
            fan_out_engagement(&state, &req_ctx, event).await;
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(AnalyticsEventBatchResponse {
            recorded: created.len(),
            events: created,
        }),
    ))
}

async fn fan_out_engagement(
    state: &AnalyticsState,
    req_ctx: &RequestContext,
    input: &CreateAnalyticsEventInput,
) {
    let Some(ref data) = input.data else { return };

    let get_i32 =
        |key: &str| -> Option<i32> { data.get(key).and_then(|v| v.as_i64()).map(|v| v as i32) };
    let get_f32 =
        |key: &str| -> Option<f32> { data.get(key).and_then(|v| v.as_f64()).map(|v| v as f32) };
    let get_bool = |key: &str| -> Option<bool> { data.get(key).and_then(|v| v.as_bool()) };
    let get_string =
        |key: &str| -> Option<String> { data.get(key).and_then(|v| v.as_str()).map(String::from) };

    let time_on_page = get_i32("time_on_page_ms").unwrap_or(0);
    if time_on_page == 0 {
        return;
    }

    let engagement_input = CreateEngagementEventInput {
        page_url: input.page_url.clone(),
        event_type: input.event_type.as_str().to_string(),
        time_on_page_ms: time_on_page,
        max_scroll_depth: get_i32("max_scroll_depth").unwrap_or(0),
        click_count: get_i32("click_count").unwrap_or(0),
        optional_metrics: EngagementOptionalMetrics {
            time_to_first_interaction_ms: get_i32("time_to_first_interaction_ms"),
            time_to_first_scroll_ms: get_i32("time_to_first_scroll_ms"),
            scroll_velocity_avg: get_f32("scroll_velocity_avg"),
            scroll_direction_changes: get_i32("scroll_direction_changes"),
            mouse_move_distance_px: get_i32("mouse_move_distance_px"),
            keyboard_events: get_i32("keyboard_events"),
            copy_events: get_i32("copy_events"),
            focus_time_ms: get_i32("focus_time_ms"),
            blur_count: get_i32("blur_count"),
            tab_switches: get_i32("tab_switches"),
            visible_time_ms: get_i32("visible_time_ms"),
            hidden_time_ms: get_i32("hidden_time_ms"),
            is_rage_click: get_bool("is_rage_click"),
            is_dead_click: get_bool("is_dead_click"),
            reading_pattern: get_string("reading_pattern"),
        },
    };

    let content_id = resolve_content_id(
        &state.content,
        state.content_routing.as_deref(),
        &input.page_url,
        input.slug.as_deref(),
    )
    .await;

    if let Err(e) = state
        .engagement
        .create_engagement(
            req_ctx.session_id().as_str(),
            req_ctx.user_id().as_str(),
            content_id.as_ref(),
            &engagement_input,
        )
        .await
    {
        tracing::warn!(error = %e, "Failed to fan out engagement data from page_exit event");
    }
}
