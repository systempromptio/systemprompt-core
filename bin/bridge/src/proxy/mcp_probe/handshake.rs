//! The probe's handshake: MCP 2026-07-28 `server/discover`, the legacy
//! `initialize` fallback, and the `tools/list` that proves the auth chain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use super::rpc::{
    self, Lifecycle, discover_body, discover_supports_current, initialize_body, list_tools,
};
use super::{
    McpAuthState, McpServerAuth, ProbeOutcome, SESSION_HEADER, elapsed_ms, now_unix, result,
    snippet,
};
use crate::ids::McpSessionId;

struct Target<'a> {
    client: &'a reqwest::Client,
    slug: &'a str,
    url: &'a str,
    bearer: &'a str,
    probed_at_unix: u64,
    started: Instant,
}

impl Target<'_> {
    fn post(&self) -> reqwest::RequestBuilder {
        rpc::post(self.client, self.url, self.bearer)
    }

    fn fail(&self, outcome: ProbeOutcome) -> McpServerAuth {
        result(self.slug, self.url, self.probed_at_unix, outcome)
    }
}

struct Handshake {
    http: u16,
    latency: u64,
    session: Option<McpSessionId>,
    stateless: bool,
}

pub async fn probe_endpoint(
    client: &reqwest::Client,
    slug: &str,
    url: &str,
    bearer: &str,
) -> McpServerAuth {
    let target = Target {
        client,
        slug,
        url,
        bearer,
        probed_at_unix: now_unix(),
        started: Instant::now(),
    };
    let resp = match rpc::stateless(target.post(), "server/discover")
        .json(&discover_body())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return target.fail(transport_failure(&e, target.started)),
    };

    let status = resp.status();
    if status.is_client_error() && !matches!(status.as_u16(), 401 | 403 | 404 | 409) {
        drop(resp);
        return probe_legacy(&target).await;
    }
    if !status.is_success() {
        return target.fail(rejected(resp, target.started).await);
    }

    let http = status.as_u16();
    let content_type = rpc::content_type(&resp);
    let body = match resp.text().await {
        Ok(body) => body,
        Err(e) => {
            return target.fail(ProbeOutcome {
                state: McpAuthState::ProtocolError,
                http_status: Some(http),
                latency_ms: Some(elapsed_ms(target.started)),
                error: Some(format!("server/discover body unreadable: {e}")),
            });
        },
    };
    if !discover_supports_current(&content_type, &body) {
        return probe_legacy(&target).await;
    }

    let handshake = Handshake {
        http,
        latency: elapsed_ms(target.started),
        session: None,
        stateless: true,
    };
    finish(&target, handshake).await
}

async fn probe_legacy(target: &Target<'_>) -> McpServerAuth {
    let resp = match target.post().json(&initialize_body()).send().await {
        Ok(r) => r,
        Err(e) => return target.fail(transport_failure(&e, target.started)),
    };
    if !resp.status().is_success() {
        return target.fail(rejected(resp, target.started).await);
    }
    let handshake = Handshake {
        http: resp.status().as_u16(),
        latency: elapsed_ms(target.started),
        session: resp
            .headers()
            .get(SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| McpSessionId::try_new(v).ok()),
        stateless: false,
    };
    drop(resp);
    finish(target, handshake).await
}

async fn finish(target: &Target<'_>, handshake: Handshake) -> McpServerAuth {
    let lifecycle = if handshake.stateless {
        Lifecycle::Stateless
    } else {
        Lifecycle::Session(handshake.session.as_ref().map(McpSessionId::as_str))
    };
    match list_tools(target.client, target.url, target.bearer, lifecycle).await {
        Ok(tools) => McpServerAuth {
            id: target.slug.to_owned(),
            url: target.url.to_owned(),
            state: McpAuthState::Authenticated,
            tools,
            http_status: Some(handshake.http),
            latency_ms: Some(handshake.latency),
            error: None,
            session_id: handshake.session,
            probed_at_unix: target.probed_at_unix,
        },
        Err(error) => target.fail(ProbeOutcome {
            state: McpAuthState::ProtocolError,
            http_status: Some(handshake.http),
            latency_ms: Some(handshake.latency),
            error: Some(error.to_string()),
        }),
    }
}

fn transport_failure(e: &reqwest::Error, started: Instant) -> ProbeOutcome {
    let state = if e.is_timeout() {
        McpAuthState::ProbeTimeout
    } else if e.is_connect() {
        McpAuthState::ProxyUnreachable
    } else {
        McpAuthState::ProtocolError
    };
    ProbeOutcome {
        state,
        http_status: None,
        latency_ms: Some(elapsed_ms(started)),
        error: Some(e.to_string()),
    }
}

async fn rejected(resp: reqwest::Response, started: Instant) -> ProbeOutcome {
    let http = resp.status().as_u16();
    let latency = elapsed_ms(started);
    let state = match http {
        403 => McpAuthState::LoopbackMismatch,
        401 => McpAuthState::GatewayUnauthorized,
        404 => McpAuthState::NotRegistered,
        409 => McpAuthState::ProviderNotConnected,
        _ => McpAuthState::UpstreamError,
    };
    let error = match resp.text().await {
        Ok(body) => snippet(&body),
        Err(e) => format!("response body unreadable: {e}"),
    };
    ProbeOutcome {
        state,
        http_status: Some(http),
        latency_ms: Some(latency),
        error: Some(error),
    }
}
