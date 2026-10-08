//! Content search endpoint over `SearchService`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::{Extension, Json};
use systemprompt_content::{SearchRequest, SearchResponse, SearchService};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;

pub async fn query_handler(
    Extension(_req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiHttpError> {
    tracing::info!(query = %request.query, "Searching");

    let repositories = ctx.content_repositories();
    let search_service =
        SearchService::new(repositories.search.clone(), repositories.content.clone());

    let response = search_service.search(&request).await?;
    tracing::info!(total = response.total, "Search completed");
    Ok(Json(response))
}
