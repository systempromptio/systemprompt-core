//! The forwarding half of the proxy: taking a request the router has accepted,
//! sending it upstream, and recording what happened — stats and the activity
//! line.
//!
//! Split from `mod.rs`, which keeps request routing and the loopback endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::{self, ProxyBody};
use crate::proxy::server::ProxyContext;

use super::responses::{owned_response, record_stats};

pub(super) struct RequestMeta {
    pub req_id: String,
    pub method: Method,
    pub path: String,
}

pub(super) async fn forward_to_gateway(
    req: Request<Incoming>,
    ctx: ProxyContext,
    meta: RequestMeta,
) -> Result<Response<ProxyBody>, Infallible> {
    let started = Instant::now();
    let cfg = ctx.snapshot();
    let RequestMeta {
        req_id,
        method,
        path,
    } = meta;
    match forward::forward(
        req,
        forward::ForwardDeps {
            client: ctx.client.clone(),
            gateway_base: cfg.gateway_base.as_ref(),
            token_cache: ctx.token_cache.as_ref(),
            session_context: ctx.session.as_ref(),
            stats: Arc::clone(&ctx.stats),
            activity: ctx.deps.activity.clone(),
            mcp_registry: Arc::clone(&ctx.deps.mcp_registry),
            gateway_http: ctx.deps.http.clone(),
            plugin_tokens: Arc::clone(&ctx.deps.plugin_tokens),
        },
    )
    .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, status, latency_ms);
            tracing::info!(
                target: "systemprompt_bridge::proxy",
                req_id = %req_id,
                method = %method,
                path = %path,
                status,
                latency_ms,
                "req out"
            );
            ctx.deps.activity.append_at(
                upstream_level(status),
                format!("proxy: {method} {path} → {status} ({latency_ms}ms) [{req_id}]"),
            );
            Ok(response)
        },
        Err(e) => {
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, StatusCode::BAD_GATEWAY.as_u16(), latency_ms);
            if forward::is_client_disconnect(&e) {
                tracing::warn!(
                    target: "systemprompt_bridge::proxy",
                    req_id = %req_id,
                    method = %method,
                    path = %path,
                    latency_ms,
                    "req out: client disconnected"
                );
                ctx.deps.activity.append_warn(format!(
                    "proxy: {method} {path} → client disconnected [{req_id}]"
                ));
            } else {
                tracing::error!(
                    target: "systemprompt_bridge::proxy",
                    req_id = %req_id,
                    method = %method,
                    path = %path,
                    latency_ms,
                    error = %e,
                    "req out: forward error"
                );
                ctx.deps
                    .activity
                    .append_error(format!("proxy: {method} {path} → error: {e} [{req_id}]"));
            }
            if let Some(challenge) = mcp_auth_challenge(&e, &path, cfg.gateway_base.as_ref()) {
                return Ok(challenge);
            }
            Ok(owned_response(e.status(), e.client_detail()))
        },
    }
}

// Why: a credential failure otherwise maps to 503, which `/mcp` renders as a
// dead server; the RFC 9728 401 challenge offers re-authentication instead.
fn mcp_auth_challenge(
    err: &forward::ForwardError,
    path: &str,
    gateway_base: &systemprompt_identifiers::ValidatedUrl,
) -> Option<Response<ProxyBody>> {
    if !matches!(
        err,
        forward::ForwardError::Auth(_) | forward::ForwardError::AuthTimeout
    ) {
        return None;
    }
    let slug = path.strip_prefix("/mcp/")?.split('/').next()?;
    if slug.is_empty() {
        return None;
    }
    let base = gateway_base.as_str().trim_end_matches('/');
    let metadata = format!("{base}/.well-known/oauth-protected-resource/api/v1/mcp/{slug}/mcp");
    let mut response = owned_response(StatusCode::UNAUTHORIZED, err.client_detail());
    let value = format!("Bearer resource_metadata=\"{metadata}\"");
    let header = match hyper::header::HeaderValue::from_str(&value) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, slug = %slug, "invalid WWW-Authenticate challenge value");
            return None;
        },
    };
    response
        .headers_mut()
        .insert(hyper::header::WWW_AUTHENTICATE, header);
    Some(response)
}

const fn upstream_level(status: u16) -> crate::activity::LogLevel {
    match status {
        500.. => crate::activity::LogLevel::Error,
        400..500 => crate::activity::LogLevel::Warn,
        _ => crate::activity::LogLevel::Info,
    }
}
