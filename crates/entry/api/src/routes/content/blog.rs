//! Content delivery routes for source-scoped documents.
//!
//! Serves content by source and slug as JSON or Markdown, honouring the
//! configured content-negotiation suffix and advertising the Markdown
//! alternate via a `Link` header when negotiation is enabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::header::LINK;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use systemprompt_content::Content;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::{ApiError, MarkdownFrontmatter, MarkdownResponse};
use systemprompt_runtime::AppContext;
use systemprompt_traits::RepositoryError;

use crate::error::ApiHttpError;
use crate::services::middleware::{AcceptedFormat, AcceptedMediaType};

pub async fn list_content_by_source_handler(
    State(ctx): State<AppContext>,
    Path(source_id): Path<String>,
) -> Result<Json<Vec<Content>>, ApiHttpError> {
    let source_id = SourceId::try_new(source_id).map_err(ApiError::from)?;
    let content_service = &ctx.content_repositories().content;

    let content = content_service
        .list_by_source(&source_id, &LocaleCode::english())
        .await
        .map_err(RepositoryError::from)?;
    Ok(Json(content))
}

pub async fn get_content_handler(
    State(ctx): State<AppContext>,
    Extension(_req_ctx): Extension<RequestContext>,
    accepted_format: Option<Extension<AcceptedFormat>>,
    Path((source_id, slug)): Path<(String, String)>,
) -> Result<Response, ApiHttpError> {
    let source_id = SourceId::try_new(source_id).map_err(ApiError::from)?;
    let content_service = &ctx.content_repositories().content;

    let content = content_service
        .find_by_source_and_slug(&source_id, &slug, &LocaleCode::english())
        .await
        .map_err(RepositoryError::from)?
        .ok_or_else(content_not_found)?;

    let wants_markdown =
        accepted_format.is_some_and(|f| f.0.media_type() == AcceptedMediaType::Markdown);
    if wants_markdown {
        return Ok(content_to_markdown_response(&content).into_response());
    }

    let config = ctx.config();
    if !config.content_negotiation.enabled {
        return Ok(Json(content).into_response());
    }
    let suffix = config
        .content_negotiation
        .markdown_suffix
        .trim_start_matches('.');
    let link_value = format!(
        "</api/v1/content/{}/{}/{}>; rel=\"alternate\"; type=\"text/markdown\"",
        source_id, slug, suffix
    );
    let mut response = Json(&content).into_response();
    if let Ok(header_value) = link_value.parse() {
        response.headers_mut().insert(LINK, header_value);
    }
    Ok(response)
}

pub async fn get_content_markdown_handler(
    State(ctx): State<AppContext>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path((source_id, slug)): Path<(String, String)>,
) -> Result<Response, ApiHttpError> {
    let source_id = SourceId::try_new(source_id).map_err(ApiError::from)?;
    let content_service = &ctx.content_repositories().content;

    let slug = slug.trim_end_matches(".md");

    let content = content_service
        .find_by_source_and_slug(&source_id, slug, &LocaleCode::english())
        .await
        .map_err(RepositoryError::from)?
        .ok_or_else(content_not_found)?;
    Ok(content_to_markdown_response(&content).into_response())
}

fn content_not_found() -> ApiHttpError {
    ApiHttpError::not_found("Content not found")
}

fn content_to_markdown_response(content: &Content) -> MarkdownResponse {
    let tags: Vec<String> = content
        .keywords
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();

    let frontmatter = MarkdownFrontmatter::new(&content.title, &content.slug)
        .with_description(&content.description)
        .with_author(&content.author)
        .with_published_at(content.published_at.format("%Y-%m-%d").to_string())
        .with_tags(tags);

    MarkdownResponse::new(frontmatter, &content.body)
}
