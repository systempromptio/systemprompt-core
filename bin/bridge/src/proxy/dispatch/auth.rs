use std::net::SocketAddr;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::ProxyBody;
use crate::proxy::secret;
use crate::proxy::server::ProxyContext;

use super::{sha256_8, simple_response};

pub(super) fn reject_non_loopback(
    req_id: &str,
    method: &Method,
    path: &str,
    host_hdr: &str,
    peer: SocketAddr,
) -> Response<ProxyBody> {
    tracing::warn!(
        target: "systemprompt_bridge::proxy",
        req_id = %req_id,
        host = %host_hdr,
        peer = %peer,
        "reject: non-loopback host"
    );
    crate::activity::activity_log().append(format!(
        "proxy: {method} {path} → 403 (non-loopback host: {host_hdr}) [{req_id}]"
    ));
    simple_response(StatusCode::FORBIDDEN, "forbidden: non-loopback host\n")
}

pub(super) fn verify_loopback_secret(
    req: &Request<Incoming>,
    ctx: &ProxyContext,
    req_id: &str,
    method: &Method,
    path: &str,
    user_agent: &str,
    peer: SocketAddr,
) -> Option<Response<ProxyBody>> {
    let presented = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .unwrap_or(v)
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    if !presented.is_empty() && secret::verify(&presented, ctx.secret.as_ref()) {
        return None;
    }
    let presented_fp = sha256_8(&presented);
    let expected_fp = sha256_8(ctx.secret.as_ref().as_str());
    tracing::warn!(
        target: "systemprompt_bridge::proxy",
        req_id = %req_id,
        peer = %peer,
        method = %method,
        path = %path,
        ua = %user_agent,
        presented_len = presented.len(),
        presented_fp = %presented_fp,
        expected_fp = %expected_fp,
        "reject: bad loopback secret"
    );
    crate::activity::activity_log().append(format!(
        "proxy: {method} {path} → 403 (bad secret; presented_fp={presented_fp} \
         expected_fp={expected_fp}) [{req_id}]"
    ));
    Some(simple_response(
        StatusCode::FORBIDDEN,
        "forbidden: bad loopback secret\n",
    ))
}
