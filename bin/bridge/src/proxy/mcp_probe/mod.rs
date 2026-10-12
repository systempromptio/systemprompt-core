//! Live MCP auth probe through the bridge's loopback proxy.
//!
//! It exercises the full auth chain the host app uses, speaking MCP 2026-07-28
//! first and falling back to the legacy `initialize` handshake.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use crate::mcp_registry::McpRegistry;
use crate::proxy::LoopbackEndpoint;

mod handshake;
mod rpc;
mod types;

pub use handshake::probe_endpoint;
pub use types::{McpAuthState, McpServerAuth, McpTool};


const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
pub(super) const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
pub(super) const LEGACY_PROTOCOL_VERSION: &str = "2025-11-25";
pub(super) const SESSION_HEADER: &str = "mcp-session-id";

#[must_use]
pub async fn probe_all(loopback: &LoopbackEndpoint, registry: &McpRegistry) -> Vec<McpServerAuth> {
    let probed_at_unix = now_unix();

    if registry.is_empty() {
        return vec![McpServerAuth {
            id: String::new(),
            url: String::new(),
            state: McpAuthState::NoServers,
            tools: Vec::new(),
            http_status: None,
            latency_ms: None,
            error: None,
            session_id: None,
            probed_at_unix,
        }];
    }

    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();

    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            return slugs
                .iter()
                .map(|slug| McpServerAuth {
                    id: (*slug).clone(),
                    url: loopback.mcp_url(slug),
                    state: McpAuthState::LocalError,
                    tools: Vec::new(),
                    http_status: None,
                    latency_ms: None,
                    error: Some(format!("probe client build failed: {e}")),
                    session_id: None,
                    probed_at_unix,
                })
                .collect();
        },
    };

    // Why: each probe is a round-trip to that server's vendor through the
    // gateway, and a sync walks every server before it can write the desktop
    // tool policy. In series that was the sum of every vendor's latency (12s
    // for four servers); together it is the slowest one. Order is preserved.
    futures_util::future::join_all(slugs.iter().map(|slug| probe_one(loopback, &client, slug)))
        .await
}

pub fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .no_proxy()
        .build()
}

#[must_use]
pub async fn probe_slug(
    loopback: &LoopbackEndpoint,
    registry: &McpRegistry,
    slug: &str,
) -> Option<McpServerAuth> {
    if !registry.contains_key(slug) {
        return None;
    }
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            return Some(McpServerAuth {
                id: slug.to_owned(),
                url: loopback.mcp_url(slug),
                state: McpAuthState::LocalError,
                tools: Vec::new(),
                http_status: None,
                latency_ms: None,
                error: Some(format!("probe client build failed: {e}")),
                session_id: None,
                probed_at_unix: now_unix(),
            });
        },
    };
    Some(probe_one(loopback, &client, slug).await)
}

async fn probe_one(
    loopback: &LoopbackEndpoint,
    client: &reqwest::Client,
    slug: &str,
) -> McpServerAuth {
    let url = loopback.mcp_url(slug);
    let probed_at_unix = now_unix();
    let bearer = match loopback.bearer() {
        Ok(b) => b,
        Err(e) => {
            return result(
                slug,
                &url,
                probed_at_unix,
                ProbeOutcome {
                    state: McpAuthState::LocalError,
                    http_status: None,
                    latency_ms: None,
                    error: Some(format!("loopback secret unavailable: {e}")),
                },
            );
        },
    };

    probe_endpoint(client, slug, &url, &bearer).await
}

pub(super) struct ProbeOutcome {
    pub state: McpAuthState,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

pub(super) fn result(
    slug: &str,
    url: &str,
    probed_at_unix: u64,
    outcome: ProbeOutcome,
) -> McpServerAuth {
    McpServerAuth {
        id: slug.to_owned(),
        url: url.to_owned(),
        state: outcome.state,
        tools: Vec::new(),
        http_status: outcome.http_status,
        latency_ms: outcome.latency_ms,
        error: outcome.error,
        session_id: None,
        probed_at_unix,
    }
}

pub(super) fn snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.len() <= 200 {
        return trimmed.to_owned();
    }
    let mut end = 200;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &trimmed[..end])
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
