//! IP-ban enforcement middleware backed by the ban list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
//!
//! The gate is fail-closed: a request is served only once the ban list has
//! affirmatively reported the caller's address as clean. An unresolvable
//! address or an unreachable ban list denies the request rather than admitting
//! it, so a database outage cannot silently disable IP banning.
//!
//! The static content router is out of scope: it is merged after this layer is
//! applied, so public pages and assets are served without a ban-list lookup. A
//! database fault must not take the public site down, and static content
//! carries no privileged data worth banning an address from.
//!
//! Liveness and readiness probes are exempt. They are unauthenticated, carry no
//! caller identity worth banning, and orchestrators recycle pods on a failed
//! probe — denying them during an outage would convert a transient database
//! fault into a rolling restart. `/metrics` needs no exemption here: it is
//! served on its own listener, which this middleware never wraps.

use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_users::BannedIpRepository;
use tracing::warn;

use super::client_addr::resolve_client_ip;

fn is_probe_path(path: &str) -> bool {
    path == "/health"
        || path == ApiPaths::LIVEZ
        || path == ApiPaths::READYZ
        || path == ApiPaths::HEALTH
}

fn deny(reason: &'static str) -> Response {
    let mut response = ApiError::forbidden("Access denied").into_response();
    response
        .headers_mut()
        .insert("X-Blocked-Reason", http::HeaderValue::from_static(reason));
    response
}

pub async fn ip_ban_middleware(
    request: Request,
    next: Next,
    banned_ip_repo: Arc<BannedIpRepository>,
    trusted_proxies: Arc<Vec<IpNet>>,
) -> Response {
    if is_probe_path(request.uri().path()) {
        return next.run(request).await;
    }

    let ip_address = resolve_client_ip(
        request.headers(),
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
        &trusted_proxies,
    )
    .map(|a| a.to_string());

    let Some(ip) = &ip_address else {
        warn!(path = %request.uri().path(), "Denied request with an unresolvable client address");
        return deny("ip-unresolvable");
    };

    match banned_ip_repo.is_banned(ip).await {
        Ok(true) => {
            warn!(ip = %ip, path = %request.uri().path(), "Blocked request from banned IP");
            deny("ip-banned")
        },
        Ok(false) => next.run(request).await,
        Err(e) => {
            tracing::error!(error = %e, ip = %ip, "Ban list unreachable; denying request");
            deny("ip-ban-unavailable")
        },
    }
}
