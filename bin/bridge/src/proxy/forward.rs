//! Request forwarding to the gateway: hop-by-hop header stripping and auth
//! injection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::{HeaderMap, Request, Response, StatusCode};
use systemprompt_identifiers::{
    GatewayConversationId, SessionId, ValidatedUrl, headers as sp_headers,
};
use thiserror::Error;

use crate::mcp_registry;
use crate::proxy::server::ProxyStats;
use crate::proxy::session::{self, SessionContext};
use crate::proxy::token_cache::TokenCache;
use crate::proxy::usage;

const HOP_BY_HOP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "content-length",
    "authorization",
    "x-api-key",
];

pub type ProxyBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

#[derive(Debug, Error)]
pub enum ForwardError {
    #[error("authentication unavailable: {0}")]
    Auth(String),
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
    #[error("upstream request failed: {0}")]
    Upstream(#[from] reqwest::Error),
    #[error("response build failed: {0}")]
    BuildResponse(#[from] http::Error),
    #[error("request body exceeds {BUFFERED_BODY_LIMIT} bytes")]
    BodyTooLarge,
    #[error("request body read failed: {0}")]
    ReadBody(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ForwardError {
    /// An auth failure is the bridge's own inability to obtain a credential,
    /// not a broken upstream, so it must not masquerade as `502`. `503`
    /// tells the caller the condition is local and retryable.
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Auth(_) | Self::AuthTimeout => StatusCode::SERVICE_UNAVAILABLE,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BadMethod { .. } | Self::BadHeader(_) => StatusCode::BAD_REQUEST,
            Self::Upstream(_) | Self::BuildResponse(_) | Self::ReadBody(_) => {
                StatusCode::BAD_GATEWAY
            },
        }
    }

    /// Names the real cause: a plugin hook author reading `bad gateway` cannot
    /// discover that the host simply has no credential store.
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
    } = deps;
    let token = token_cache.current(REFRESH_THRESHOLD_SECS).await?;

    let (parts, body) = req.into_parts();
    let request_path = parts.uri.path().to_owned();

    let (route, upstream_bearer, is_hook) = match resolve_route(&parts.uri, gateway_base) {
        RouteResolution::Gateway(url) => (
            Route {
                url,
                extra_headers: BTreeMap::new(),
            },
            token.token.expose().to_owned(),
            false,
        ),
        RouteResolution::Mcp(route) => (route, token.token.expose().to_owned(), false),
        RouteResolution::Hook { url, plugin_id } => {
            let gw = crate::gateway::GatewayClient::new(gateway_base.clone());
            let hook = crate::auth::plugin_oauth::mint_or_refresh_plugin_token(
                &gw,
                &token.token,
                &plugin_id,
            )
            .await
            .map_err(|e| ForwardError::Auth(format!("hook token mint for {plugin_id}: {e}")))?;
            (
                Route {
                    url,
                    extra_headers: BTreeMap::new(),
                },
                hook.access_token,
                true,
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

    let (upstream_body, gateway_conversation_id) =
        prepare_upstream_body(body, session_context).await?;

    let upstream_headers = build_upstream_headers(
        &parts.headers,
        &upstream_bearer,
        session_context.session_id(),
        gateway_conversation_id.as_ref(),
        &route.extra_headers,
    )?;

    let upstream_response = client
        .request(method, &route.url)
        .headers(upstream_headers)
        .body(upstream_body)
        .send()
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
        if status == StatusCode::UNAUTHORIZED && !is_hook {
            token_cache.invalidate().await;
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
    let wrapped = usage::wrap_response_stream(&content_type, tap_enabled, stats, upstream_stream);
    let body: ProxyBody = StreamBody::new(wrapped).boxed();

    Ok(response_builder.body(body)?)
}

struct Route {
    url: String,
    extra_headers: BTreeMap<String, String>,
}

enum RouteResolution {
    Gateway(String),
    Mcp(Route),
    UnknownMcp(String),
    Hook {
        url: String,
        plugin_id: systemprompt_identifiers::PluginId,
    },
}

fn resolve_route(uri: &http::Uri, gateway_base: &ValidatedUrl) -> RouteResolution {
    if let Some(name) = parse_mcp_path(uri.path()) {
        let registry = mcp_registry::snapshot();
        return registry.get(name).map_or_else(
            || RouteResolution::UnknownMcp(name.to_owned()),
            |entry| {
                RouteResolution::Mcp(Route {
                    url: entry.url.as_str().to_owned(),
                    extra_headers: entry.headers.clone(),
                })
            },
        );
    }
    if uri.path().starts_with("/api/public/hooks/")
        && let Some(plugin_id) = parse_hook_plugin_id(uri)
    {
        return RouteResolution::Hook {
            url: build_gateway_url(gateway_base, uri),
            plugin_id,
        };
    }
    RouteResolution::Gateway(build_gateway_url(gateway_base, uri))
}

fn parse_mcp_path(path: &str) -> Option<&str> {
    let stripped = path.strip_prefix("/mcp/")?;
    let name = stripped.split('/').next()?;
    if name.is_empty() { None } else { Some(name) }
}

fn parse_hook_plugin_id(uri: &http::Uri) -> Option<systemprompt_identifiers::PluginId> {
    uri.query()?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == "plugin_id" && !v.is_empty()).then(|| systemprompt_identifiers::PluginId::new(v))
    })
}

fn build_gateway_url(gateway_base: &ValidatedUrl, uri: &http::Uri) -> String {
    let path_and_query = uri.path_and_query().map_or("/", |p| p.as_str());
    let separator = if path_and_query.starts_with('/') {
        ""
    } else {
        "/"
    };
    let rewritten = rewrite_otel_to_v1(path_and_query);
    let path_and_query = rewritten.as_deref().unwrap_or(path_and_query);
    format!(
        "{base}{separator}{path_and_query}",
        base = gateway_base.as_str().trim_end_matches('/'),
    )
}

// Why: OTLP exporters POST `/otel` without the `/v1` prefix the gateway router
// is nested under.
fn rewrite_otel_to_v1(path_and_query: &str) -> Option<String> {
    let (path, suffix) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(p, q)| (p, Some(q)));
    if path != "/otel" && !path.starts_with("/otel/") {
        return None;
    }
    Some(suffix.map_or_else(|| format!("/v1{path}"), |q| format!("/v1{path}?{q}")))
}

fn not_found_response(body: &str) -> ForwardResult<Response<ProxyBody>> {
    let bytes = Bytes::copy_from_slice(body.as_bytes());
    let body: ProxyBody = Full::new(bytes).map_err(|never| match never {}).boxed();
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)?)
}

fn build_upstream_headers(
    src: &HeaderMap,
    bearer: &str,
    session_id: &SessionId,
    gateway_conversation_id: Option<&GatewayConversationId>,
    extra: &BTreeMap<String, String>,
) -> ForwardResult<HeaderMap> {
    let mut headers = HeaderMap::with_capacity(src.len() + 4 + extra.len());
    copy_request_headers(src, &mut headers);

    let bearer = reqwest::header::HeaderValue::try_from(format!("Bearer {bearer}"))
        .map_err(|e| ForwardError::BadHeader(format!("authorization: {e}")))?;
    headers.insert(reqwest::header::AUTHORIZATION, bearer);
    headers.insert(
        reqwest::header::HeaderName::from_static("x-systemprompt-bridge"),
        reqwest::header::HeaderValue::from_static("1"),
    );
    let session_value = reqwest::header::HeaderValue::try_from(session_id.as_str())
        .map_err(|e| ForwardError::BadHeader(format!("{}: {e}", sp_headers::SESSION_ID)))?;
    headers.insert(
        reqwest::header::HeaderName::from_static(sp_headers::SESSION_ID),
        session_value,
    );
    if let Some(id) = gateway_conversation_id {
        let value = reqwest::header::HeaderValue::try_from(id.as_str()).map_err(|e| {
            ForwardError::BadHeader(format!("{}: {e}", sp_headers::GATEWAY_CONVERSATION_ID))
        })?;
        headers.insert(
            reqwest::header::HeaderName::from_static(sp_headers::GATEWAY_CONVERSATION_ID),
            value,
        );
    }

    for (k, v) in extra {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        let value = reqwest::header::HeaderValue::try_from(v)
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        headers.insert(name, value);
    }

    Ok(headers)
}

async fn prepare_upstream_body(
    body: Incoming,
    session_context: &SessionContext,
) -> ForwardResult<(reqwest::Body, Option<GatewayConversationId>)> {
    let buffered = collect_body(body).await?;
    let id = session::derive_gateway_conversation_id(&buffered)
        .map(|hash| session_context.context_for_prefix(hash));
    if let Some(ref c) = id {
        tracing::Span::current().record("gateway_conversation_id", tracing::field::display(c));
    }
    Ok((reqwest::Body::from(buffered), id))
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

fn copy_request_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.append(name, value);
    }
}

fn copy_response_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            hyper::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            hyper::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.insert(name, value);
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

#[must_use]
pub fn is_client_disconnect(err: &ForwardError) -> bool {
    matches!(
        err,
        ForwardError::Upstream(e)
            if e.is_request() && e.to_string().contains("connection closed")
    )
}

const _: fn() = || {
    const fn assert_send<T: Send>() {}
    assert_send::<ForwardResult<Response<ProxyBody>>>();
};
