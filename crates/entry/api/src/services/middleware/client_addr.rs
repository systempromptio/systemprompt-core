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
//! `X-Real-IP` and `CF-Connecting-IP` are honoured only under rule 2's
//! trust gate; otherwise they are ignored.
//!
//! The trusted-proxy CIDR set is parsed and validated when the profile
//! loads (`ServerConfig::trusted_proxies` deserialises to `Vec<IpNet>`);
//! an invalid entry fails boot rather than being silently dropped, so this
//! resolver only ever sees a validated set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::future::{Future, ready};
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use ipnet::IpNet;

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

    for header in ["x-real-ip", "cf-connecting-ip"] {
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
pub fn resolve_client_ip_from_config(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
) -> Option<IpAddr> {
    let trusted = systemprompt_models::Config::get()
        .map(|c| c.trusted_proxies.clone())
        .unwrap_or_default();
    resolve_client_ip(headers, connect_info, &trusted)
}

#[must_use]
pub fn client_ip_from_request(request: &axum::extract::Request) -> Option<IpAddr> {
    resolve_client_ip_from_config(
        request.headers(),
        request.extensions().get::<ConnectInfo<SocketAddr>>(),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub Option<IpAddr>);

impl<S: Sync> FromRequestParts<S> for ClientIp {
    type Rejection = Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Infallible>> + Send {
        let resolved = resolve_client_ip_from_config(
            &parts.headers,
            parts.extensions.get::<ConnectInfo<SocketAddr>>(),
        );
        ready(Ok(Self(resolved)))
    }
}
