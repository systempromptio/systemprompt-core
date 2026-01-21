use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde_json::json;

pub async fn handle_health_api() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "service": "oauth"
        })),
    )
}
