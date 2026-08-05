//! Content search endpoint over `SearchService`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use systemprompt_content::{SearchRequest, SearchService};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

pub async fn query_handler(
    Extension(_req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
    Json(request): Json<SearchRequest>,
) -> Response {
    log_search_start(&request.query);

    let repositories = ctx.content_repositories();
    let search_service =
        SearchService::new(repositories.search.clone(), repositories.content.clone());

    execute_search(&search_service, &request).await
}

fn log_search_start(query: &str) {
    tracing::info!(query = %query, "Searching");
}

async fn execute_search(service: &SearchService, request: &SearchRequest) -> Response {
    match service.search(request).await {
        Ok(response) => {
            tracing::info!(total = response.total, "Search completed");
            Json(response).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Search error");
            internal_error(&e.to_string())
        },
    }
}

fn internal_error(message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": message})),
    )
        .into_response()
}
