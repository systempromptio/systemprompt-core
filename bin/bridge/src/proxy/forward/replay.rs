//! One replay of an upstream request whose socket turned out to be dead.
//!
//! The Windows WSL localhost relay drops an idle keep-alive socket without a
//! FIN or RST, so the next request written to it aborts before the gateway
//! sees a byte; such a request is replayed once on a fresh socket. Only
//! managed-MCP traffic is replayed: inference could bill twice and the
//! gateway routes carry their own retry semantics. Within MCP a `tools/call`
//! is replayed only when the connection never opened.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replay {
    Never,
    OnConnect,
    OnConnectionLoss,
}

pub(crate) struct UpstreamRequest<'a> {
    pub client: &'a reqwest::Client,
    pub method: &'a reqwest::Method,
    pub url: &'a str,
    pub headers: &'a reqwest::header::HeaderMap,
    pub body: &'a Bytes,
    pub policy: Replay,
}

#[must_use]
pub fn replay_policy(request_path: &str, body: &Bytes) -> Replay {
    if !request_path.starts_with("/mcp/") {
        return Replay::Never;
    }
    match jsonrpc_method(body).as_deref() {
        Some("tools/call") => Replay::OnConnect,
        _ => Replay::OnConnectionLoss,
    }
}

#[derive(serde::Deserialize)]
struct JsonRpcMethod {
    #[serde(default)]
    method: Option<String>,
}

fn jsonrpc_method(body: &Bytes) -> Option<String> {
    serde_json::from_slice::<JsonRpcMethod>(body).ok()?.method
}

#[must_use]
pub fn should_replay(err: &reqwest::Error, policy: Replay) -> bool {
    if err.is_timeout() || err.is_body() || err.is_decode() || err.status().is_some() {
        return false;
    }
    match policy {
        Replay::Never => false,
        Replay::OnConnect => err.is_connect(),
        Replay::OnConnectionLoss => err.is_connect() || err.is_request(),
    }
}

pub(crate) async fn send_with_replay(
    request: UpstreamRequest<'_>,
) -> Result<reqwest::Response, reqwest::Error> {
    let attempt = || {
        request
            .client
            .request(request.method.clone(), request.url)
            .headers(request.headers.clone())
            .body(reqwest::Body::from(request.body.clone()))
            .send()
    };
    match attempt().await {
        Ok(response) => Ok(response),
        Err(first) if should_replay(&first, request.policy) => {
            tracing::warn!(
                url = request.url,
                error = %describe(&first),
                "upstream socket failed before a response; replaying once on a fresh connection"
            );
            attempt().await
        },
        Err(first) => Err(first),
    }
}

// Why: reqwest's `Display` stops at "error sending request for url (…)" and
// keeps the hyper/io cause — timeout, reset, closed — in `source()`. That
// cause is the only thing that tells a stale socket from a down gateway.
#[must_use]
pub fn describe(err: &reqwest::Error) -> String {
    let mut text = err.to_string();
    let mut cause = std::error::Error::source(err);
    while let Some(inner) = cause {
        let piece = inner.to_string();
        if !text.contains(&piece) {
            text.push_str(": ");
            text.push_str(&piece);
        }
        cause = inner.source();
    }
    text
}
