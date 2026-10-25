//! Client-address resolution that does not blindly trust hop headers.
//!
//! [`resolve_client_ip`] is the single helper every middleware that
//! cares about the originating client (rate-limiter, IP banlist,
//! bot-scoring, abuse heuristics) must use. The contract:
//!
//! 1. If the immediate socket peer (`ConnectInfo<SocketAddr>`) is not contained
//!    in `trusted_proxies`, return the peer address. Hop headers are ignored
//!    entirely — they are untrusted in this case.
//! 2. If the peer is trusted, walk `X-Forwarded-For` right-to-left and take the
//!    first hop that is itself outside `trusted_proxies`. That hop is the
//!    closest entity our proxy chain still sees, and the earliest one a client
//!    could have spoofed.
//! 3. If the chain is empty or every hop is trusted, fall back to the peer
//!    address.
//!
//! `X-Real-IP`, `Fly-Client-IP`, and `CF-Connecting-IP` are honoured only
//! under rule 2's trust gate; otherwise they are ignored.
//!
//! The trusted-proxy CIDR set is parsed and validated when the profile
//! loads (`ServerConfig::trusted_proxies` deserialises to `Vec<IpNet>`);
//! an invalid entry fails boot rather than being silently dropped, so this
//! resolver only ever sees a validated set.
//!
//! A request arriving through a proxy the server does not trust has its
//! forwarded client-IP headers discarded and the peer's own address used
//! instead. That is almost always a misconfiguration: something is adding
//! `X-Forwarded-For`, so a proxy is in the path, but its address is absent
//! from `server.trusted_proxies`, and every client behind it then resolves to
//! one address and shares one rate-limit bucket, one ban entry and one set of
//! abuse heuristics. The warning is not gated on the peer being in a private
//! range: a reverse proxy on a host network has a public address. Whether the
//! peer is public or private has
//! no bearing on whether the operator wants to know.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::{Future, ready};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use ipnet::IpNet;
use systemprompt_manifest::Config;

fn is_trusted(addr: IpAddr, trusted: &[IpNet]) -> bool {
    trusted.iter().any(|net| net.contains(&addr))
}

#[must_use]
pub fn resolve_client_ip(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
    trusted: &[IpNet],
) -> Option<IpAddr> {
    let peer_ip = connect_info.map(|c| c.0.ip())?;

    if !is_trusted(peer_ip, trusted) {
        return Some(peer_ip);
    }

    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        let hops: Vec<&str> = xff
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        for hop in hops.iter().rev() {
            if let Ok(addr) = hop.parse::<IpAddr>()
                && !is_trusted(addr, trusted)
            {
                return Some(addr);
            }
        }
    }

    for header in ["x-real-ip", "fly-client-ip", "cf-connecting-ip"] {
        if let Some(raw) = headers.get(header).and_then(|v| v.to_str().ok())
            && let Ok(addr) = raw.trim().parse::<IpAddr>()
            && !is_trusted(addr, trusted)
        {
            return Some(addr);
        }
    }

    Some(peer_ip)
}

#[must_use]
pub fn forwarded_headers_ignored(headers: &HeaderMap, peer_ip: IpAddr, trusted: &[IpNet]) -> bool {
    !is_trusted(peer_ip, trusted) && headers.contains_key("x-forwarded-for")
}

#[must_use]
pub fn client_ip_from_request(request: &axum::extract::Request, config: &Config) -> Option<IpAddr> {
    resolve_client_ip(
        request.headers(),
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
        &config.trusted_proxies,
    )
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the router carries no Config extension to resolve trusted proxies from")]
pub struct ClientIpConfigMissing;

impl IntoResponse for ClientIpConfigMissing {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(error = %self, "client ip extractor is not wired");
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub Option<IpAddr>);

impl<S: Sync> FromRequestParts<S> for ClientIp {
    type Rejection = ClientIpConfigMissing;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, ClientIpConfigMissing>> + Send {
        let resolved = parts
            .extensions
            .get::<Arc<Config>>()
            .ok_or(ClientIpConfigMissing)
            .map(|config| {
                Self(resolve_client_ip(
                    &parts.headers,
                    parts.extensions.get::<ConnectInfo<SocketAddr>>(),
                    &config.trusted_proxies,
                ))
            });
        ready(resolved)
    }
}
