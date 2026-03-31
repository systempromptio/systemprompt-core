use axum::body::Body;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;
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

    let provided_bytes = provided_token.as_bytes();
    let expected_bytes = expected_token.as_bytes();
    let length_matches = provided_bytes.len() == expected_bytes.len();
    if !length_matches || !bool::from(provided_bytes.ct_eq(expected_bytes)) {
        return ApiError::unauthorized("Invalid sync token").into_response();
    }

    next.run(request).await
}
