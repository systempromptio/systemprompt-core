//! Per-request proxy dispatch: path classification and header extraction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::{self, ProxyBody};
use crate::proxy::requests;
use crate::proxy::server::ProxyContext;

mod auth;
mod responses;

use responses::{json_response, owned_response, record_stats, simple_response};

pub(super) struct RequestMeta {
    pub req_id: String,
    pub method: Method,
    pub path: String,
    pub user_agent: String,
}

// Why: the gateway stamps every inference response with this id and keys its
// governance decision on the same value, so it is the only correlator that
// joins a row in our ring to the platform's own verdict.
const UPSTREAM_REQUEST_ID: &str = "x-systemprompt-request-id";

// Why: a user agent is the only attribution a loopback request carries -- there
// is no per-agent credential -- so the leading product token is the best answer
// to "which agent made this call".
pub(super) fn agent_label(user_agent: &str) -> String {
    let token = user_agent.split_whitespace().next().unwrap_or("");
    let name = token.split('/').next().unwrap_or("");
    if name.is_empty() {
        "unknown".to_owned()
    } else {
        name.to_owned()
    }
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
        user_agent,
    } = meta;
    let agent = agent_label(&user_agent);
    let req_id: Arc<str> = Arc::from(req_id.as_str());
    match forward::forward(
        req,
        forward::ForwardDeps {
            client: ctx.client.clone(),
            gateway_base: cfg.gateway_base.as_ref(),
            token_cache: ctx.token_cache.as_ref(),
            session_context: ctx.session.as_ref(),
            stats: Arc::clone(&ctx.stats),
            req_id: Arc::clone(&req_id),
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
            crate::activity::activity_log().append_at(
                upstream_level(status),
                format!("proxy: {method} {path} → {status} ({latency_ms}ms) [{req_id}]"),
            );
            requests::request_log().record(requests::NewRequest {
                req_id: &req_id,
                agent: &agent,
                method: method.as_str(),
                path: &path,
                verdict: requests::LocalVerdict::Forwarded,
                deny_reason: None,
                status: Some(status),
                latency_ms: Some(latency_ms),
                upstream_request_id: response
                    .headers()
                    .get(UPSTREAM_REQUEST_ID)
                    .and_then(|v| v.to_str().ok())
                    .map(ToOwned::to_owned),
            });
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
                crate::activity::activity_log().append_warn(format!(
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
                crate::activity::activity_log()
                    .append_error(format!("proxy: {method} {path} → error: {e} [{req_id}]"));
            }
            requests::request_log().record(requests::NewRequest {
                req_id: &req_id,
                agent: &agent,
                method: method.as_str(),
                path: &path,
                verdict: requests::LocalVerdict::Forwarded,
                deny_reason: Some(e.to_string()),
                status: Some(e.status().as_u16()),
                latency_ms: Some(latency_ms),
                upstream_request_id: None,
            });
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

pub async fn handle_request(
    req: Request<Incoming>,
    ctx: ProxyContext,
    peer: SocketAddr,
) -> Result<Response<ProxyBody>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or("").to_owned();
    let req_id = mint_req_id();
    let host_hdr = header_str(&req, http::header::HOST);
    let user_agent = header_str(&req, http::header::USER_AGENT);
    let content_length = req
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    tracing::info!(
        target: "systemprompt_bridge::proxy",
        req_id = %req_id,
        method = %method,
        path = %path,
        query = %query,
        peer = %peer,
        host = %host_hdr,
        ua = %user_agent,
        content_length,
        "req in"
    );

    if !host_hdr.is_empty() && !host_is_loopback(&host_hdr) {
        let log = auth::RequestLog {
            req_id: &req_id,
            method: &method,
            path: &path,
            user_agent: &user_agent,
            peer,
        };
        return Ok(auth::reject_non_loopback(&log, &host_hdr));
    }

    if is_unauthenticated_path(&method, &path) {
        tracing::debug!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            method = %method,
            path = %path,
            "unauthenticated path"
        );
        if path == "/healthz" {
            return Ok(health_response(&method));
        }
        if path == WHOAMI_PATH {
            return Ok(whoami_response(&ctx));
        }
        return forward_to_gateway(
            req,
            ctx,
            RequestMeta {
                req_id,
                method,
                path,
                user_agent,
            },
        )
        .await;
    }

    let log = auth::RequestLog {
        req_id: &req_id,
        method: &method,
        path: &path,
        user_agent: &user_agent,
        peer,
    };
    if let Some(rejection) = auth::verify_loopback_secret(&req, &ctx, &log) {
        return Ok(rejection);
    }

    forward_to_gateway(
        req,
        ctx,
        RequestMeta {
            req_id,
            method,
            path,
            user_agent,
        },
    )
    .await
}

fn header_str(req: &Request<Incoming>, name: http::header::HeaderName) -> String {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

pub const WHOAMI_PATH: &str = "/__bridge/whoami";

fn is_unauthenticated_path(method: &Method, path: &str) -> bool {
    match (method, path) {
        (&Method::GET | &Method::HEAD, "/healthz") | (&Method::GET, WHOAMI_PATH) => true,
        (&Method::POST, p) if p == "/otel" || p.starts_with("/otel/") => true,
        _ => false,
    }
}

fn health_response(method: &Method) -> Response<ProxyBody> {
    let body = if method == Method::HEAD { "" } else { "ok\n" };
    simple_response(StatusCode::OK, body)
}

// Why: unauthenticated on purpose — the caller asking is by definition one that
// could not authenticate, a sibling bridge deciding whether the port is held by
// itself or by a stranger. It inherits the loopback-host guard above, and the
// payload must carry no secret nor anything derived from one.
fn whoami_response(ctx: &ProxyContext) -> Response<ProxyBody> {
    let who = crate::proxy::identity::WhoAmI::current(ctx.port, ctx.started_at_unix);
    match serde_json::to_string(&who) {
        Ok(body) => json_response(StatusCode::OK, body),
        Err(e) => {
            tracing::warn!(error = %e, "could not serialise the whoami payload");
            simple_response(StatusCode::INTERNAL_SERVER_ERROR, "whoami unavailable\n")
        },
    }
}

fn mint_req_id() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

fn host_is_loopback(host: &str) -> bool {
    let host_only = host.split(':').next().unwrap_or("");
    matches!(host_only, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

const fn upstream_level(status: u16) -> crate::activity::LogLevel {
    match status {
        500.. => crate::activity::LogLevel::Error,
        400..500 => crate::activity::LogLevel::Warn,
        _ => crate::activity::LogLevel::Info,
    }
}
