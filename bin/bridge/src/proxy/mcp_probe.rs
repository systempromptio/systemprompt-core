//! Live MCP auth probe: an `initialize` → `tools/list` round-trip through the
//! bridge's loopback proxy, exercising the full auth chain the host app uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{Value, json};

const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const SESSION_HEADER: &str = "mcp-session-id";

#[derive(Debug, Clone, Serialize)]
pub struct McpServerAuth {
    pub id: String,
    pub url: String,
    pub state: McpAuthState,
    pub tools: Vec<String>,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub session_id: Option<String>,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
pub enum McpAuthState {
    #[default]
    Unknown,
    NoServers,
    Authenticated,
    /// Proxy 403 — presented loopback secret rejected.
    LoopbackMismatch,
    /// Gateway 401 — injected bridge JWT rejected.
    GatewayUnauthorized,
    /// Proxy 404 — slug not in the proxy's live registry.
    NotRegistered,
    UpstreamError,
    ProxyUnreachable,
    ProtocolError,
}

/// Returns one synthetic `NoServers` entry when the registry is empty.
#[must_use]
pub async fn probe_all() -> Vec<McpServerAuth> {
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
                    url: crate::proxy::mcp_url(slug),
                    state: McpAuthState::ProtocolError,
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
        out.push(probe_one(&client, slug).await);
    }
    out
}

pub fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .no_proxy()
        .build()
}

async fn probe_one(client: &reqwest::Client, slug: &str) -> McpServerAuth {
    let url = crate::proxy::mcp_url(slug);
    let probed_at_unix = now_unix();
    let bearer = match crate::proxy::loopback_bearer() {
        Ok(b) => b,
        Err(e) => {
            return result(
                slug,
                &url,
                probed_at_unix,
                ProbeOutcome {
                    state: McpAuthState::ProtocolError,
                    http_status: None,
                    latency_ms: None,
                    error: Some(format!("loopback secret unavailable: {e}")),
                },
            );
        },
    };

    probe_endpoint(client, slug, &url, &bearer).await
}

/// `url` and `bearer` are injected so callers can drive the probe against any
/// endpoint without touching global proxy state.
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
            let state = if e.is_connect() || e.is_timeout() {
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

    // A tools/list failure is best-effort and must not downgrade the auth verdict.
    let session = resp
        .headers()
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    _ = resp.text().await;

    let tools = list_tools(client, url, bearer, session.as_deref()).await;

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

async fn list_tools(
    client: &reqwest::Client,
    url: &str,
    bearer: &str,
    session: Option<&str>,
) -> Vec<String> {
    let initialized = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    _ = initialized
        .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
        .send()
        .await;

    let req = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    let Ok(resp) = req
        .json(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }))
        .send()
        .await
    else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = resp.text().await.unwrap_or_default();
    parse_tool_names(&content_type, &body)
}

fn with_session(
    builder: reqwest::RequestBuilder,
    session: Option<&str>,
) -> reqwest::RequestBuilder {
    match session {
        Some(s) => builder.header(SESSION_HEADER, s),
        None => builder,
    }
}

fn initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "systemprompt-bridge-probe", "version": env!("CARGO_PKG_VERSION") },
        },
    })
}

fn parse_tool_names(content_type: &str, body: &str) -> Vec<String> {
    let Some(value) = parse_jsonrpc(content_type, body) else {
        return Vec::new();
    };
    value
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_jsonrpc(content_type: &str, body: &str) -> Option<Value> {
    if content_type.contains("text/event-stream") {
        let mut data = String::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.trim_start());
            }
        }
        serde_json::from_str(&data).ok()
    } else {
        serde_json::from_str(body).ok()
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
