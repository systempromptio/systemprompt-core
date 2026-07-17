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
//! `parse_trusted_proxies` drops invalid CIDR entries with a `tracing::warn!`
//! rather than failing bootstrap: a single typo in a profile must not take
//! the whole replica offline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use ipnet::IpNet;

#[must_use]
pub fn parse_trusted_proxies(raw: &[String]) -> Vec<IpNet> {
    raw.iter()
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(net) = trimmed.parse::<IpNet>() {
                return Some(net);
            }
            if let Ok(addr) = trimmed.parse::<IpAddr>() {
                let prefix = match addr {
                    IpAddr::V4(_) => 32,
                    IpAddr::V6(_) => 128,
                };
                if let Ok(net) = IpNet::new(addr, prefix) {
                    return Some(net);
                }
            }
            tracing::warn!(entry = %trimmed, "ignoring invalid trusted_proxies entry");
            None
        })
        .collect()
}

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
