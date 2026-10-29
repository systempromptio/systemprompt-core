//! The forwarding half of the proxy: taking a request the router has accepted,
//! sending it upstream, and recording what happened — stats and the activity
//! line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::credential::LoopbackCredential;
use crate::proxy::forward::{self, ProxyBody};
use crate::proxy::server::ProxyContext;

use super::responses::{owned_response, record_stats};

pub(super) struct RequestMeta {
    pub req_id: String,
    pub method: Method,
    pub path: String,
    pub credential: LoopbackCredential,
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
        credential,
    } = meta;
    let log = RequestLog {
        req_id,
        method,
        path,
    };
    if cfg.disabled {
        return Ok(owned_response(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "{} is disabled on this device; enable it to route through the gateway",
                crate::brand::brand().app_name
            ),
        ));
    }
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
            drive_signals: &ctx.deps.drive_signals,
            foreign_models: cfg.foreign_models.as_ref(),
            credential,
        },
    )
    .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, status, latency_ms);
            log_forwarded(&ctx, &log, status, latency_ms);
            Ok(response)
        },
        Err(e) => {
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, StatusCode::BAD_GATEWAY.as_u16(), latency_ms);
            log_forward_failure(&ctx, &log, &e, latency_ms);
            if let Some(challenge) = mcp_auth_challenge(&e, &log.path, cfg.gateway_base.as_ref()) {
                return Ok(challenge);
            }
            Ok(owned_response(e.status(), e.client_detail()))
        },
    }
}

struct RequestLog {
    req_id: String,
    method: Method,
    path: String,
}

fn log_forwarded(ctx: &ProxyContext, log: &RequestLog, status: u16, latency_ms: u64) {
    let RequestLog {
        req_id,
        method,
        path,
    } = log;
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
}

fn log_forward_failure(
    ctx: &ProxyContext,
    log: &RequestLog,
    e: &forward::ForwardError,
    latency_ms: u64,
) {
    let RequestLog {
        req_id,
        method,
        path,
    } = log;
    if forward::is_client_disconnect(e) {
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
    } else if systemprompt_models::ApiPaths::is_otel(path) {
        // Why: Claude Code retries its OTLP exporter on its own schedule; a
        // dropped batch loses telemetry, never a user's request.
        tracing::debug!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            method = %method,
            path = %path,
            latency_ms,
            error = %e,
            "req out: telemetry forward dropped"
        );
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
}

// Why: MCP clients use the RFC 9728 resource-metadata challenge to offer
// re-authentication.
fn mcp_auth_challenge(
    err: &forward::ForwardError,
    path: &str,
    gateway_base: &systemprompt_identifiers::ValidatedUrl,
) -> Option<Response<ProxyBody>> {
    // Why: AuthRetryable is a network blip, not a credential the client can fix by
    // re-authenticating; challenging on it would send clients into a pointless
    // OAuth round trip while the bridge is already retrying.
    if !err.is_credential_failure() {
        return None;
    }
    let slug = systemprompt_models::ApiPaths::mcp_server_slug(path)?;
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
