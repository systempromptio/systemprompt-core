//! Request forwarding to the gateway: hop-by-hop header stripping, auth
//! injection, and one replay when the upstream socket turns out to be dead.
//!
//! The replay policy and its rationale live in [`replay`].
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
use systemprompt_identifiers::ValidatedUrl;

use crate::proxy::credential::LoopbackCredential;
use crate::proxy::server::ProxyStats;
use crate::proxy::session::SessionContext;
use crate::proxy::token_cache::TokenCache;
use crate::proxy::{keepalive, usage};

mod body;
mod error;
mod headers;
pub mod replay;
mod route;

use body::prepare_upstream_body;
pub use body::{CHAT_COMPLETIONS_PATH, stamp_opencode_session};
pub use error::{ForwardError, ForwardResult, is_client_disconnect};
use headers::{build_upstream_headers, copy_response_headers};
pub use replay::{Replay, describe, replay_policy, should_replay};
use replay::{UpstreamRequest, send_with_replay};
use route::{Route, RouteResolution, resolve_route, same_origin_as};

pub type ProxyBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

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
    pub credential: LoopbackCredential,
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
        credential,
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
            require_hook_credential(&credential, plugin_id.as_str())?;
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
        prepare_upstream_body(body, session_context, &parts.headers, &request_path).await?;

    if let Err(error) = session_context
        .native_sessions()
        .observe(&parts.headers, &buffered_body)
    {
        tracing::warn!(%error, "Native session could not be recorded for binding");
    }
    let mut upstream_headers = build_upstream_headers(
        &parts.headers,
        &upstream_bearer,
        session_context.session_id(),
        gateway_conversation_id.as_ref(),
        &route.extra_headers,
    )?;

    headers::ensure_ingestion_delivery_id(&request_path, &mut upstream_headers)?;
    if hook_plugin.is_some() && request_path == "/api/public/hooks/track" {
        authenticate_hook_track(
            gateway_base,
            &parts.headers,
            &buffered_body,
            &mut upstream_headers,
        )?;
    }

    let upstream_response = send_with_replay(UpstreamRequest {
        client: &client,
        method: &method,
        url: &route.url,
        headers: &upstream_headers,
        body: &buffered_body,
        policy: replay_policy(&request_path, &buffered_body),
    })
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
            } else if same_origin_as(&route.url, gateway_base) {
                token_cache.reject_upstream(&request_path).await;
            } else {
                // Why: only the gateway that minted the credential can say it
                // is bad. A managed MCP upstream elsewhere (or a stale entry
                // for a previous gateway) rejecting it is neither a sign-out
                // nor a reason to re-mint.
                tracing::warn!(
                    upstream = %route.url,
                    gateway = %gateway_base,
                    "401 from a non-gateway upstream; the gateway token is kept"
                );
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

fn authenticate_hook_track(
    gateway_base: &ValidatedUrl,
    request_headers: &http::HeaderMap,
    buffered_body: &[u8],
    upstream_headers: &mut http::HeaderMap,
) -> ForwardResult<()> {
    let host = request_headers
        .get("x-systemprompt-host")
        .and_then(|value| value.to_str().ok());
    if let Err(error) = crate::feedback::hooks::authenticate_forwarded_hook(
        gateway_base.as_str(),
        host,
        upstream_headers,
    ) && !matches!(error, crate::feedback::FeedbackError::EnrollmentRequired)
    {
        return Err(ForwardError::Auth(
            "Device evidence authentication unavailable".to_owned(),
        ));
    }
    // JSON: protocol boundary — the hook body is the host's own wire shape.
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(buffered_body)
        && let (Some(host), Some(session)) = (
            host.and_then(crate::feedback::client_kind),
            value.get("session_id").and_then(serde_json::Value::as_str),
        )
        && let Ok(root) = crate::feedback::metadata_root()
        && let Ok(enrollment) =
            crate::feedback::credentials::Enrollment::load(&root, gateway_base.as_str())
    {
        let outbox = crate::feedback::outbox::Outbox::new(
            enrollment.outbox_path(&root),
            crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
        );
        if let Err(error) = outbox.queue_session(host, session) {
            tracing::debug!(%error, "Hook native session awaits binding");
        }
    }
    Ok(())
}

fn require_hook_credential(credential: &LoopbackCredential, plugin_id: &str) -> ForwardResult<()> {
    match credential {
        LoopbackCredential::Hook(plugin) if plugin.as_str() == plugin_id => Ok(()),
        LoopbackCredential::Hook(_) => Err(ForwardError::Scope {
            presented: "hook token of another plugin",
            route: "this plugin's hook route",
        }),
        LoopbackCredential::Secret => Err(ForwardError::Scope {
            presented: "loopback secret",
            route: "a plugin hook route",
        }),
        LoopbackCredential::Host(_) => Err(ForwardError::Scope {
            presented: "host token",
            route: "a plugin hook route",
        }),
    }
}

fn not_found_response(body: &str) -> ForwardResult<Response<ProxyBody>> {
    let bytes = Bytes::copy_from_slice(body.as_bytes());
    let body: ProxyBody = Full::new(bytes).map_err(|never| match never {}).boxed();
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(body)?)
}

const _: fn() = || {
    const fn assert_send<T: Send>() {}
    assert_send::<ForwardResult<Response<ProxyBody>>>();
};
