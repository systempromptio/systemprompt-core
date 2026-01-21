use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use systemprompt_content::ContentService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_models::RequestContext;

pub async fn list_content_by_source_handler(
    State(db_pool): State<DbPool>,
    Path(source_id): Path<String>,
) -> impl IntoResponse {
    let content_service = match ContentService::new(&db_pool) {
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
    State(db_pool): State<DbPool>,
    Extension(_req_ctx): Extension<RequestContext>,
    Path((source_id, slug)): Path<(String, String)>,
) -> impl IntoResponse {
    let content_service = match ContentService::new(&db_pool) {
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
    match content_service
        .get_by_source_and_slug(&source_id, &slug)
        .await
    {
        Ok(Some(content)) => Json(content).into_response(),
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
