use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

pub async fn sync_token_middleware(request: Request<Body>, next: Next) -> Response {
    let expected_token = match std::env::var("SYNC_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            tracing::warn!("SYNC_TOKEN environment variable not set");
            return error_response(StatusCode::UNAUTHORIZED, "SYNC_TOKEN not configured");
        },
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let provided_token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            );
        },
    };

    if provided_token != expected_token {
        return error_response(StatusCode::UNAUTHORIZED, "Invalid sync token");
    }

    next.run(request).await
}

fn error_response(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
