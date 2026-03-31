use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use systemprompt_models::api::ApiError;
use systemprompt_users::BannedIpRepository;
use tracing::warn;

fn extract_client_ip(request: &Request) -> Option<String> {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        })
        .or_else(|| {
            request
                .headers()
                .get("cf-connecting-ip")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        })
        .or_else(|| {
            request
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|ci| ci.0.ip().to_string())
        })
}

pub async fn ip_ban_middleware(
    request: Request,
    next: Next,
    banned_ip_repo: Arc<BannedIpRepository>,
) -> Response {
    let ip_address = extract_client_ip(&request);

    if let Some(ip) = &ip_address {
        match banned_ip_repo.is_banned(ip).await {
            Ok(true) => {
                warn!(ip = %ip, path = %request.uri().path(), "Blocked request from banned IP");
                let api_error = ApiError::forbidden("Access denied");
                let mut response = api_error.into_response();
                response.headers_mut().insert(
                    "X-Blocked-Reason",
                    http::HeaderValue::from_static("ip-banned"),
                );
                return response;
            },
            Ok(false) => {},
            Err(e) => {
                tracing::error!(error = %e, ip = %ip, "Failed to check IP ban status");
            },
        }
    }

    next.run(request).await
}
