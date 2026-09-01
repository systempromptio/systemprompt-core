//! Live MCP auth probe: an `initialize` → `tools/list` round-trip through the
//! bridge's loopback proxy, exercising the full auth chain the host app uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};

use crate::proxy::LoopbackEndpoint;

mod rpc;
mod types;

pub use types::{McpAuthState, McpServerAuth, McpTool};

use rpc::{initialize_body, list_tools};

const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
pub(super) const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
pub(super) const SESSION_HEADER: &str = "mcp-session-id";

#[must_use]
pub async fn probe_all(loopback: &LoopbackEndpoint) -> Vec<McpServerAuth> {
    let registry = crate::mcp_registry::snapshot();
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

    let mut out = Vec::with_capacity(slugs.len());
    for slug in slugs {
        out.push(probe_one(loopback, &client, slug).await);
    }
    out
}

pub fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .no_proxy()
        .build()
}

#[must_use]
pub async fn probe_slug(loopback: &LoopbackEndpoint, slug: &str) -> Option<McpServerAuth> {
    if !crate::mcp_registry::snapshot().contains_key(slug) {
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

pub async fn probe_endpoint(
    client: &reqwest::Client,
    slug: &str,
    url: &str,
    bearer: &str,
) -> McpServerAuth {
    let probed_at_unix = now_unix();
    let started = Instant::now();
    let resp = client
        .post(url)
        .header(AUTHORIZATION, bearer)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .json(&initialize_body())
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            let state = if e.is_timeout() {
                McpAuthState::ProbeTimeout
            } else if e.is_connect() {
                McpAuthState::ProxyUnreachable
            } else {
                McpAuthState::ProtocolError
            };
            return result(
                slug,
                url,
                probed_at_unix,
                ProbeOutcome {
                    state,
                    http_status: None,
                    latency_ms: Some(elapsed_ms(started)),
                    error: Some(e.to_string()),
                },
            );
        },
    };

    let status = resp.status();
    let http = status.as_u16();
    let latency = elapsed_ms(started);

    if !status.is_success() {
        let state = match http {
            403 => McpAuthState::LoopbackMismatch,
            401 => McpAuthState::GatewayUnauthorized,
            404 => McpAuthState::NotRegistered,
            _ => McpAuthState::UpstreamError,
        };
        let body = resp.text().await.unwrap_or_default();
        return result(
            slug,
            url,
            probed_at_unix,
            ProbeOutcome {
                state,
                http_status: Some(http),
                latency_ms: Some(latency),
                error: Some(snippet(&body)),
            },
        );
    }

    let session = resp
        .headers()
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(crate::ids::McpSessionId::new);
    _ = resp.text().await;

    let tools = list_tools(
        client,
        url,
        bearer,
        session.as_ref().map(crate::ids::McpSessionId::as_str),
    )
    .await;

    McpServerAuth {
        id: slug.to_owned(),
        url: url.to_owned(),
        state: McpAuthState::Authenticated,
        tools,
        http_status: Some(http),
        latency_ms: Some(latency),
        error: None,
        session_id: session,
        probed_at_unix,
    }
}

struct ProbeOutcome {
    state: McpAuthState,
    http_status: Option<u16>,
    latency_ms: Option<u64>,
    error: Option<String>,
}

fn result(slug: &str, url: &str, probed_at_unix: u64, outcome: ProbeOutcome) -> McpServerAuth {
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

fn snippet(body: &str) -> String {
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

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
