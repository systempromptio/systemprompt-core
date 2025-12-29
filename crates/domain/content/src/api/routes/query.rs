use crate::models::SearchRequest;
use crate::services::SearchService;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use systemprompt_core_database::DbPool;
use systemprompt_models::RequestContext;

pub async fn query_handler(
    Extension(_req_ctx): Extension<RequestContext>,
    State(db_pool): State<DbPool>,
    Json(request): Json<SearchRequest>,
) -> Response {
    log_search_start(&request.query);

    let search_service = match SearchService::new(&db_pool) {
        Ok(service) => service,
        Err(e) => return handle_service_error(&e),
    };

    execute_search(&search_service, &request).await
}

fn log_search_start(query: &str) {
    tracing::info!(query = %query, "Searching");
}

fn handle_service_error(e: &crate::ContentError) -> Response {
    tracing::error!(error = %e, "Failed to create search service");
    internal_error(&e.to_string())
}

#[allow(clippy::cognitive_complexity)]
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
