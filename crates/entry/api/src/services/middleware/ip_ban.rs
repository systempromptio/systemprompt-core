use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use systemprompt_users::BannedIpRepository;
use systemprompt_models::api::ApiError;
use tracing::warn;

#[derive(Clone, Copy, Debug)]
pub struct IpBanMiddleware;

impl IpBanMiddleware {
    fn extract_ip(request: &Request) -> Option<String> {
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
    }
}

pub async fn ip_ban_middleware(
    request: Request,
    next: Next,
    banned_ip_repo: Arc<BannedIpRepository>,
) -> Response {
    let ip_address = IpBanMiddleware::extract_ip(&request);

    if let Some(ip) = &ip_address {
        match banned_ip_repo.is_banned(ip).await {
            Ok(true) => {
                warn!(ip = %ip, path = %request.uri().path(), "Blocked request from banned IP");
                let api_error = ApiError::forbidden("Access denied");
                let mut response = api_error.into_response();
                response.headers_mut().insert(
                    "X-Blocked-Reason",
                    "ip-banned".parse().expect("valid header value"),
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

pub fn ip_ban_layer(
    banned_ip_repo: Arc<BannedIpRepository>,
) -> axum::middleware::FromFnLayer<
    impl Fn(Request, Next) -> Pin<Box<dyn Future<Output = Response> + Send>> + Clone + Send,
    (),
    Request,
> {
    axum::middleware::from_fn(move |req: Request, next: Next| {
        let repo = banned_ip_repo.clone();
        let fut: Pin<Box<dyn Future<Output = Response> + Send>> =
            Box::pin(async move { ip_ban_middleware(req, next, repo).await });
        fut
    })
}
