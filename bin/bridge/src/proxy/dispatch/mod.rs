//! Per-request proxy dispatch: path classification and header extraction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::ProxyBody;
use crate::proxy::server::ProxyContext;

mod auth;
mod forwarding;
mod responses;

use forwarding::{RequestMeta, forward_to_gateway};
use responses::{json_response, owned_response, simple_response};

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
        return Ok(auth::reject_non_loopback(&ctx, &log, &host_hdr));
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
    let who = crate::proxy::identity::WhoAmI::current(
        ctx.port,
        ctx.started_at_unix,
        &ctx.deps.install_id,
    );
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
