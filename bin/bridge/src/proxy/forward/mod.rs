//! Request forwarding to the gateway: hop-by-hop header stripping, auth
//! injection, and one replay when the upstream socket turns out to be dead.
//!
//! Why the replay exists: the upstream client keeps idle keep-alive sockets,
//! and on Windows the WSL localhost relay drops an idle one without a FIN or
//! RST. The next request written to it is retransmitted for ~30 s and then
//! aborted by the TCP stack — reqwest reports "error sending request", the
//! gateway never saw a byte, and the caller (Claude desktop reading a
//! dashboard's `ui://` resource) shows the server as unreachable. The request
//! body is buffered anyway, so a request that failed at the connection level
//! is replayed once on a fresh socket. A `tools/call` is the one exception:
//! if the failure came after the bytes left, the server may have executed it,
//! and a tool is not guaranteed idempotent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::{Request, Response, StatusCode};
use systemprompt_identifiers::{GatewayConversationId, ValidatedUrl};
use thiserror::Error;

use crate::proxy::server::ProxyStats;
use crate::proxy::session::{self, SessionContext};
use crate::proxy::token_cache::TokenCache;
use crate::proxy::{keepalive, usage};

mod headers;
mod route;

use headers::{build_upstream_headers, copy_response_headers};
use route::{Route, RouteResolution, resolve_route};

pub type ProxyBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

#[derive(Debug, Error)]
pub enum ForwardError {
    #[error("routing unavailable: {0}")]
    Routing(String),
    #[error("authentication unavailable: {0}")]
    Auth(String),
    #[error("authentication temporarily unavailable, retrying: {0}")]
    AuthRetryable(String),
    #[error("authentication timed out after 10s")]
    AuthTimeout,
    #[error("invalid request method {method}: {source}")]
    BadMethod {
        method: String,
        #[source]
        source: http::method::InvalidMethod,
    },
    #[error("invalid header value: {0}")]
    BadHeader(String),
    #[error("upstream request failed: {}", describe(.0))]
    Upstream(#[from] reqwest::Error),
    #[error("response build failed: {0}")]
    BuildResponse(#[from] http::Error),
    #[error("request body exceeds {BUFFERED_BODY_LIMIT} bytes")]
    BodyTooLarge,
    #[error("request body read failed: {0}")]
    ReadBody(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ForwardError {
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Auth(_) | Self::AuthRetryable(_) | Self::AuthTimeout | Self::Routing(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            },
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BadMethod { .. } | Self::BadHeader(_) => StatusCode::BAD_REQUEST,
            Self::Upstream(_) | Self::BuildResponse(_) | Self::ReadBody(_) => {
                StatusCode::BAD_GATEWAY
            },
        }
    }

    pub fn client_detail(&self) -> String {
        format!("{self}\n")
    }
}

pub type ForwardResult<T> = Result<T, ForwardError>;

pub const REFRESH_THRESHOLD_SECS: u64 = 300;

use systemprompt_models::wire::BUFFERED_BODY_LIMIT_BYTES as BUFFERED_BODY_LIMIT;

pub(crate) struct ForwardDeps<'a> {
    pub client: reqwest::Client,
    pub gateway_base: &'a ValidatedUrl,
    pub token_cache: &'a TokenCache,
    pub session_context: &'a SessionContext,
    pub stats: Arc<ProxyStats>,
    pub activity: crate::activity::ActivityLog,
    pub mcp_registry: Arc<crate::mcp_registry::McpRegistrySlot>,
    pub gateway_http: reqwest::Client,
    pub plugin_tokens: Arc<crate::auth::plugin_oauth::PluginTokenCache>,
}

#[tracing::instrument(
    level = "debug",
    skip(req, deps),
    fields(
        method = %req.method(),
        path = %req.uri().path(),
        session_id = %deps.session_context.session_id(),
        gateway_conversation_id = tracing::field::Empty,
    )
)]
pub(crate) async fn forward(
    req: Request<Incoming>,
    deps: ForwardDeps<'_>,
) -> ForwardResult<Response<ProxyBody>> {
    let ForwardDeps {
        client,
        gateway_base,
        token_cache,
        session_context,
        stats,
        activity,
        mcp_registry,
        gateway_http,
        plugin_tokens,
    } = deps;
    let token = token_cache.current(REFRESH_THRESHOLD_SECS).await?;

    let (parts, body) = req.into_parts();
    let request_path = parts.uri.path().to_owned();

    let mut hook_plugin = None;
    let (route, upstream_bearer) = match resolve_route(&parts.uri, gateway_base, &mcp_registry) {
        RouteResolution::Unavailable(reason) => return Err(ForwardError::Routing(reason)),
        RouteResolution::Gateway(url) => (
            Route {
                url,
                extra_headers: BTreeMap::new(),
            },
            token.token.expose().to_owned(),
        ),
        RouteResolution::Mcp(route) => (route, token.token.expose().to_owned()),
        RouteResolution::Hook { url, plugin_id } => {
            let gw = crate::gateway::GatewayClient::new(gateway_base.clone(), gateway_http);
            let hook = crate::auth::plugin_oauth::mint_or_refresh_plugin_token(
                &plugin_tokens,
                &gw,
                &token.token,
                &plugin_id,
            )
            .await
            .map_err(|e| ForwardError::Auth(format!("hook token mint for {plugin_id}: {e}")))?;
            hook_plugin = Some(plugin_id);
            (
                Route {
                    url,
                    extra_headers: BTreeMap::new(),
                },
                hook.access_token,
            )
        },
        RouteResolution::UnknownMcp(name) => {
            tracing::warn!(server = %name, "unknown managed MCP server requested");
            return not_found_response(&format!("unknown managed MCP server: {name}\n"));
        },
    };

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).map_err(|e| {
        ForwardError::BadMethod {
            method: parts.method.to_string(),
            source: e,
        }
    })?;

    let (buffered_body, gateway_conversation_id) =
        prepare_upstream_body(body, session_context).await?;

    let upstream_headers = build_upstream_headers(
        &parts.headers,
        &upstream_bearer,
        session_context.session_id(),
        gateway_conversation_id.as_ref(),
        &route.extra_headers,
    )?;

    let replayable = replay_policy(&request_path, &buffered_body);
    let upstream_response = send_with_replay(
        &client,
        &method,
        &route.url,
        &upstream_headers,
        &buffered_body,
        replayable,
    )
    .await?;

    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    if status.is_success() {
        if usage::is_messages_path(&request_path) {
            session_context.touch_activity();
        }
        tracing::debug!(upstream_status = status.as_u16(), "upstream forwarded");
    } else {
        tracing::warn!(upstream_status = status.as_u16(), url = %route.url, "upstream non-2xx");
        if status == StatusCode::UNAUTHORIZED {
            if let Some(plugin_id) = hook_plugin.as_ref() {
                plugin_tokens.invalidate(gateway_base.as_str(), plugin_id);
            } else {
                token_cache.reject_upstream(&request_path).await;
            }
        }
    }

    let mut response_builder = Response::builder().status(status);
    if let Some(headers_mut) = response_builder.headers_mut() {
        copy_response_headers(upstream_response.headers(), headers_mut);
    }

    let content_type = upstream_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let tap_enabled = status.is_success() && usage::is_messages_path(&request_path);

    let upstream_stream = upstream_response
        .bytes_stream()
        .map_ok(Frame::data)
        .map_err(std::io::Error::other);
    let wrapped =
        usage::wrap_response_stream(&content_type, tap_enabled, stats, activity, upstream_stream);
    let body: ProxyBody = if content_type.contains("text/event-stream") {
        StreamBody::new(keepalive::SseKeepalive::new(
            Box::pin(wrapped),
            keepalive::SSE_KEEPALIVE_INTERVAL,
        ))
        .boxed()
    } else {
        StreamBody::new(wrapped).boxed()
    };

    Ok(response_builder.body(body)?)
}

fn not_found_response(body: &str) -> ForwardResult<Response<ProxyBody>> {
    let bytes = Bytes::copy_from_slice(body.as_bytes());
    let body: ProxyBody = Full::new(bytes).map_err(|never| match never {}).boxed();
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)?)
}

async fn prepare_upstream_body(
    body: Incoming,
    session_context: &SessionContext,
) -> ForwardResult<(Bytes, Option<GatewayConversationId>)> {
    let buffered = collect_body(body).await?;
    let id = session::derive_gateway_conversation_id(&buffered)
        .map(|hash| session_context.context_for_prefix(hash));
    if let Some(ref c) = id {
        tracing::Span::current().record("gateway_conversation_id", tracing::field::display(c));
    }
    Ok((buffered, id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replay {
    Never,
    OnConnect,
    OnConnectionLoss,
}

// Why: only managed-MCP traffic is replayed. `/v1/messages` is inference —
// a replay could bill twice — and the gateway routes carry their own retry
// semantics. Within MCP, a `tools/call` is replayed only when the connection
// never opened, because nothing can have executed then.
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

fn jsonrpc_method(body: &Bytes) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    value.get("method")?.as_str().map(str::to_owned)
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

async fn send_with_replay(
    client: &reqwest::Client,
    method: &reqwest::Method,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    body: &Bytes,
    policy: Replay,
) -> Result<reqwest::Response, reqwest::Error> {
    let attempt = || {
        client
            .request(method.clone(), url)
            .headers(headers.clone())
            .body(reqwest::Body::from(body.clone()))
            .send()
    };
    match attempt().await {
        Ok(response) => Ok(response),
        Err(first) if should_replay(&first, policy) => {
            tracing::warn!(
                url,
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

async fn collect_body(body: Incoming) -> ForwardResult<Bytes> {
    match http_body_util::Limited::new(body, BUFFERED_BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(e) if e.is::<http_body_util::LengthLimitError>() => Err(ForwardError::BodyTooLarge),
        Err(e) => Err(ForwardError::ReadBody(e)),
    }
}

#[must_use]
pub fn is_client_disconnect(err: &ForwardError) -> bool {
    matches!(
        err,
        ForwardError::Upstream(e)
            if e.is_request() && describe(e).contains("connection closed")
    )
}

const _: fn() = || {
    const fn assert_send<T: Send>() {}
    assert_send::<ForwardResult<Response<ProxyBody>>>();
};
