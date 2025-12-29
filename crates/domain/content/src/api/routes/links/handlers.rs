use super::types::{
    internal_error, AnalyticsQuery, GenerateLinkRequest, GenerateLinkResponse, ListLinksQuery,
};
use crate::models::{LinkType, TrackClickParams, UtmParams};
use crate::services::link::generation::GenerateLinkParams;
use crate::services::{LinkAnalyticsService, LinkGenerationService};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::{Extension, Json};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{CampaignId, ContentId, LinkId, SessionId};
use systemprompt_models::{Config, RequestContext};
use tracing::error;

pub async fn redirect_handler(
    State(db_pool): State<DbPool>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(short_code): Path<String>,
) -> impl IntoResponse {
    let link_gen_service = match LinkGenerationService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let link = match link_gen_service.get_link_by_short_code(&short_code).await {
        Ok(Some(link)) => link,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Link not found"})),
            )
                .into_response();
        },
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let track_params = TrackClickParams::new(
        link.id.clone(),
        SessionId::new(req_ctx.request.session_id.as_str()),
    )
    .with_user_id(Some(req_ctx.auth.user_id.clone()))
    .with_context_id(Some(req_ctx.execution.context_id.clone()))
    .with_task_id(req_ctx.execution.task_id.clone());

    if let Err(e) = analytics_service.track_click(&track_params).await {
        error!(link_id = %link.id, error = %e, "Failed to track click");
    }

    let target_url = link.get_full_url();
    Redirect::temporary(&target_url).into_response()
}

pub async fn generate_link_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Json(payload): Json<GenerateLinkRequest>,
) -> impl IntoResponse {
    let link_type = match payload.link_type.as_str() {
        "redirect" => LinkType::Redirect,
        "utm" => LinkType::Utm,
        "both" => LinkType::Both,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid link_type. Must be 'redirect', 'utm', or 'both'"})),
            )
                .into_response();
        },
    };

    let utm_params = if payload.utm_source.is_some()
        || payload.utm_medium.is_some()
        || payload.utm_campaign.is_some()
    {
        Some(UtmParams {
            source: payload.utm_source,
            medium: payload.utm_medium,
            campaign: payload.utm_campaign,
            term: payload.utm_term,
            content: payload.utm_content,
        })
    } else {
        None
    };

    let link_gen_service = match LinkGenerationService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let campaign_id = payload.campaign_id.map(CampaignId::new);
    let source_content_id = payload.source_content_id.map(ContentId::new);

    match link_gen_service
        .generate_link(GenerateLinkParams {
            target_url: payload.target_url.clone(),
            link_type,
            campaign_id,
            campaign_name: payload.campaign_name,
            source_content_id,
            source_page: payload.source_page,
            utm_params,
            link_text: payload.link_text,
            link_position: payload.link_position,
            expires_at: payload.expires_at,
        })
        .await
    {
        Ok(link) => {
            let base_url = Config::get()
                .map(|c| c.api_external_url.clone())
                .unwrap_or_default();
            let redirect_url = LinkGenerationService::build_trackable_url(&link, &base_url);
            let full_url = link.get_full_url();

            Json(GenerateLinkResponse {
                link_id: link.id.to_string(),
                short_code: link.short_code,
                redirect_url,
                full_url,
            })
            .into_response()
        },
        Err(e) => internal_error(&e.to_string()).into_response(),
    }
}

pub async fn get_link_performance_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path(link_id): Path<String>,
) -> impl IntoResponse {
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let link_id = LinkId::new(link_id);
    match analytics_service.get_link_performance(&link_id).await {
        Ok(Some(performance)) => Json(performance).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Link not found"})),
        )
            .into_response(),
        Err(e) => internal_error(&e.to_string()).into_response(),
    }
}

pub async fn get_campaign_performance_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path(campaign_id): Path<String>,
) -> impl IntoResponse {
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let campaign_id = CampaignId::new(campaign_id);
    match analytics_service
        .get_campaign_performance(&campaign_id)
        .await
    {
        Ok(Some(performance)) => Json(performance).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Campaign not found"})),
        )
            .into_response(),
        Err(e) => internal_error(&e.to_string()).into_response(),
    }
}

pub async fn get_content_journey_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Query(query): Query<AnalyticsQuery>,
) -> impl IntoResponse {
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    match analytics_service
        .get_content_journey_map(query.limit, query.offset)
        .await
    {
        Ok(journey) => Json(journey).into_response(),
        Err(e) => internal_error(&e.to_string()).into_response(),
    }
}

pub async fn list_links_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Query(query): Query<ListLinksQuery>,
) -> impl IntoResponse {
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    if let Some(campaign_id) = query.campaign_id {
        let campaign_id = CampaignId::new(campaign_id);
        match analytics_service.get_links_by_campaign(&campaign_id).await {
            Ok(links) => Json(links).into_response(),
            Err(e) => internal_error(&e.to_string()).into_response(),
        }
    } else if let Some(source_content_id) = query.source_content_id {
        let source_content_id = ContentId::new(source_content_id);
        match analytics_service
            .get_links_by_source_content(&source_content_id)
            .await
        {
            Ok(links) => Json(links).into_response(),
            Err(e) => internal_error(&e.to_string()).into_response(),
        }
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Must provide either campaign_id or source_content_id"})),
        )
            .into_response()
    }
}

pub async fn get_link_clicks_handler(
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path(link_id): Path<String>,
    Query(query): Query<AnalyticsQuery>,
) -> impl IntoResponse {
    let analytics_service = match LinkAnalyticsService::new(&db_pool) {
        Ok(s) => s,
        Err(e) => return internal_error(&e.to_string()).into_response(),
    };

    let link_id = LinkId::new(link_id);
    match analytics_service
        .get_link_clicks(&link_id, query.limit, query.offset)
        .await
    {
        Ok(clicks) => Json(clicks).into_response(),
        Err(e) => internal_error(&e.to_string()).into_response(),
    }
}
