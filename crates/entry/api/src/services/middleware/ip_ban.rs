//! IP-ban enforcement middleware backed by the ban list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_models::api::ApiError;
use systemprompt_users::BannedIpRepository;
use tracing::warn;

use super::client_addr::resolve_client_ip;

pub async fn ip_ban_middleware(
    request: Request,
    next: Next,
    banned_ip_repo: Arc<BannedIpRepository>,
    trusted_proxies: Arc<Vec<IpNet>>,
) -> Response {
    let ip_address = resolve_client_ip(
        request.headers(),
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
        &trusted_proxies,
    )
    .map(|a| a.to_string());

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
