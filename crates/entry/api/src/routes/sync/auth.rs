use axum::body::Body;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;

pub async fn sync_token_middleware(request: Request<Body>, next: Next) -> Response {
    let expected_token = match std::env::var("SYNC_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            tracing::warn!("SYNC_TOKEN environment variable not set");
            return ApiError::unauthorized("SYNC_TOKEN not configured").into_response();
        },
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let provided_token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return ApiError::unauthorized("Missing or invalid Authorization header")
                .into_response();
        },
    };

    if provided_token != expected_token {
        return ApiError::unauthorized("Invalid sync token").into_response();
    }

    next.run(request).await
}
