use axum::extract::{Path, State};
use axum::http::header::LINK;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use systemprompt_content::{Content, ContentService};
use systemprompt_identifiers::SourceId;
use systemprompt_models::api::{MarkdownFrontmatter, MarkdownResponse};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use crate::services::middleware::{AcceptedFormat, AcceptedMediaType};

pub async fn list_content_by_source_handler(
    State(ctx): State<AppContext>,
    Path(source_id): Path<String>,
) -> impl IntoResponse {
    let content_service = match ContentService::new(ctx.db_pool()) {
        Ok(svc) => svc,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    let source_id = SourceId::new(source_id);
    match content_service.list_by_source(&source_id).await {
        Ok(content) => Json(content).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_content_handler(
    State(ctx): State<AppContext>,
    Extension(_req_ctx): Extension<RequestContext>,
    accepted_format: Option<Extension<AcceptedFormat>>,
    Path((source_id, slug)): Path<(String, String)>,
) -> Response {
    let content_service = match ContentService::new(ctx.db_pool()) {
        Ok(svc) => svc,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    let source_id_typed = SourceId::new(source_id.clone());
    match content_service
        .get_by_source_and_slug(&source_id_typed, &slug)
        .await
    {
        Ok(Some(content)) => {
            let wants_markdown = accepted_format
                .map(|f| f.0.media_type() == AcceptedMediaType::Markdown)
                .unwrap_or(false);

            if wants_markdown {
                content_to_markdown_response(&content).into_response()
            } else {
                let config = ctx.config();
                if config.content_negotiation.enabled {
                    let suffix = config.content_negotiation.markdown_suffix.as_str();
                    let link_value = format!(
                        "</api/v1/content/{}/{}{}>; rel=\"alternate\"; type=\"text/markdown\"",
                        source_id, slug, suffix
                    );
                    let mut response = Json(&content).into_response();
                    if let Ok(header_value) = link_value.parse() {
                        response.headers_mut().insert(LINK, header_value);
                    }
                    response
                } else {
                    Json(content).into_response()
                }
            }
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Content not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_content_markdown_handler(
    State(ctx): State<AppContext>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path((source_id, slug)): Path<(String, String)>,
) -> impl IntoResponse {
    let content_service = match ContentService::new(ctx.db_pool()) {
        Ok(svc) => svc,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        },
    };

    let slug = slug.trim_end_matches(".md");
    let source_id = SourceId::new(source_id);

    match content_service
        .get_by_source_and_slug(&source_id, slug)
        .await
    {
        Ok(Some(content)) => content_to_markdown_response(&content).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Content not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn content_to_markdown_response(content: &Content) -> MarkdownResponse {
    let tags: Vec<String> = content
        .keywords
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let frontmatter = MarkdownFrontmatter::new(&content.title, &content.slug)
        .with_description(&content.description)
        .with_author(&content.author)
        .with_published_at(content.published_at.format("%Y-%m-%d").to_string())
        .with_tags(tags);

    MarkdownResponse::new(frontmatter, &content.body)
}
