use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::{self, ProxyBody};
use crate::proxy::server::{ProxyContext, ProxyStats};

mod auth;

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
        ctx.client.clone(),
        cfg.gateway_base.as_ref(),
        ctx.token_cache.as_ref(),
        ctx.session.as_ref(),
        Arc::clone(&ctx.stats),
    )
    .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            // Why: monotonic Instant elapsed cannot exceed u64::MAX in practice;
            // saturate to keep the metric monotonic rather than wrap or fail.
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
            crate::activity::activity_log().append(format!(
                "proxy: {method} {path} → {status} ({latency_ms}ms) [{req_id}]"
            ));
            Ok(response)
        },
        Err(e) => {
            // Why: see above — saturating cast keeps the latency metric monotonic.
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
                crate::activity::activity_log().append(format!(
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
                    .append(format!("proxy: {method} {path} → error: {e} [{req_id}]"));
            }
            Ok(simple_response(StatusCode::BAD_GATEWAY, "bad gateway\n"))
        },
    }
}

pub(super) fn record_stats(stats: &ProxyStats, status: u16, latency_ms: u64) {
    stats.forwarded_total.fetch_add(1, Ordering::Relaxed);
    stats
        .last_forwarded_at_unix
        .store(now_unix(), Ordering::Relaxed);
    stats
        .last_status
        .store(u64::from(status), Ordering::Relaxed);
    stats.last_latency_ms.store(latency_ms, Ordering::Relaxed);
}

pub(super) fn simple_response(status: StatusCode, body: &'static str) -> Response<ProxyBody> {
    let full = Full::new(Bytes::from_static(body.as_bytes()))
        .map_err(|never| match never {})
        .boxed();
    let mut resp = Response::new(full);
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain"),
    );
    resp.headers_mut().insert(
        http::header::CONNECTION,
        http::HeaderValue::from_static("close"),
    );
    resp
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) async fn handle_request(
    req: Request<Incoming>,
    ctx: ProxyContext,
    peer: SocketAddr,
) -> Result<Response<ProxyBody>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
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
        return Ok(auth::reject_non_loopback(
            &req_id, &method, &path, &host_hdr, peer,
        ));
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
        return forward_to_gateway(
            req,
            ctx,
            RequestMeta {
                req_id,
                method,
                path,
            },
        )
        .await;
    }

    if let Some(rejection) =
        auth::verify_loopback_secret(&req, &ctx, &req_id, &method, &path, &user_agent, peer)
    {
        return Ok(rejection);
    }

    forward_to_gateway(
        req,
        ctx,
        RequestMeta {
            req_id,
            method,
            path,
        },
    )
    .await
}

fn header_str(req: &Request<Incoming>, name: http::header::HeaderName) -> String {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn is_unauthenticated_path(method: &Method, path: &str) -> bool {
    match (method, path) {
        (&Method::GET | &Method::HEAD, "/healthz") => true,
        (&Method::POST, p) if p == "/otel" || p.starts_with("/otel/") => true,
        _ => false,
    }
}

fn health_response(method: &Method) -> Response<ProxyBody> {
    let body = if method == Method::HEAD { "" } else { "ok\n" };
    simple_response(StatusCode::OK, body)
}

fn mint_req_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

pub(crate) fn sha256_8(s: &str) -> String {
    use sha2::{Digest, Sha256};
    if s.is_empty() {
        return "<empty>".to_string();
    }
    let d = Sha256::digest(s.as_bytes());
    format!("{:08x}", u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
}

fn host_is_loopback(host: &str) -> bool {
    let host_only = host.split(':').next().unwrap_or("");
    matches!(host_only, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}
