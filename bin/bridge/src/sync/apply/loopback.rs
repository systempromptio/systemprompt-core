//! Rewriting manifest MCP URLs that point at a loopback address onto the
//! gateway host, and the manifest copy that carries the rewritten servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;
use url::{Host, Url};

use crate::gateway::manifest::{ManagedMcpServer, SignedManifest};

pub(super) fn rewrite_loopback_urls(
    servers: &[ManagedMcpServer],
    gateway: &ValidatedUrl,
) -> Vec<ManagedMcpServer> {
    let Ok(gateway_url) = Url::parse(gateway.as_str()) else {
        return servers.to_vec();
    };
    let (Some(raw_gw_host), gw_scheme) = (gateway_url.host_str(), gateway_url.scheme()) else {
        return servers.to_vec();
    };
    // Why: Cowork's non-HTTPS MCP validator accepts 127.0.0.1 but rejects literal
    // localhost.
    let gw_host = if raw_gw_host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        raw_gw_host
    };
    let gw_port = gateway_url.port();
    servers
        .iter()
        .map(|s| rewrite_loopback_server(s, gw_scheme, gw_host, gw_port))
        .collect()
}

fn rewrite_loopback_server(
    server: &ManagedMcpServer,
    gw_scheme: &str,
    gw_host: &str,
    gw_port: Option<u16>,
) -> ManagedMcpServer {
    let url_str = server.url.as_str();
    let Ok(mut parsed) = Url::parse(url_str) else {
        return server.clone();
    };
    let is_loopback = match parsed.host() {
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(addr)) => addr.is_loopback(),
        Some(Host::Ipv6(addr)) => addr.is_loopback(),
        None => false,
    };
    if !is_loopback {
        return server.clone();
    }
    if parsed.set_scheme(gw_scheme).is_err() {
        return server.clone();
    }
    if parsed.set_host(Some(gw_host)).is_err() {
        return server.clone();
    }
    if parsed.set_port(gw_port).is_err() {
        return server.clone();
    }
    let rebuilt = parsed.to_string();
    match ValidatedUrl::try_new(&rebuilt) {
        Ok(url) => {
            tracing::info!(
                target: "bridge::sync",
                original = %url_str,
                rewritten = %rebuilt,
                "rewrote loopback MCP URL to gateway host"
            );
            let mut next = server.clone();
            next.url = url;
            next
        },
        Err(e) => {
            tracing::warn!(
                target: "bridge::sync",
                original = %url_str,
                rewritten = %rebuilt,
                error = %e,
                "loopback rewrite produced invalid URL; keeping original"
            );
            server.clone()
        },
    }
}

pub(super) fn manifest_with_servers(
    base: &SignedManifest,
    servers: Vec<ManagedMcpServer>,
) -> SignedManifest {
    let mut next = base.clone();
    next.managed_mcp_servers = servers;
    next
}
