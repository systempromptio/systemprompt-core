//! Loopback-secret authentication for proxy requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::forward::ProxyBody;
use crate::proxy::secret;
use crate::proxy::server::ProxyContext;

use super::{owned_response, simple_response};

pub(super) struct RequestLog<'a> {
    pub req_id: &'a str,
    pub method: &'a Method,
    pub path: &'a str,
    pub user_agent: &'a str,
    pub peer: SocketAddr,
}

pub(super) fn reject_non_loopback(log: &RequestLog<'_>, host_hdr: &str) -> Response<ProxyBody> {
    let RequestLog {
        req_id,
        method,
        path,
        peer,
        ..
    } = log;
    tracing::warn!(
        target: "systemprompt_bridge::proxy",
        req_id = %req_id,
        host = %host_hdr,
        peer = %peer,
        "reject: non-loopback host"
    );
    crate::activity::activity_log().append_warn(format!(
        "proxy: {method} {path} → 403 (non-loopback host: {host_hdr}) [{req_id}]"
    ));
    simple_response(StatusCode::FORBIDDEN, "forbidden: non-loopback host\n")
}

pub(super) fn verify_loopback_secret(
    req: &Request<Incoming>,
    ctx: &ProxyContext,
    log: &RequestLog<'_>,
) -> Option<Response<ProxyBody>> {
    let RequestLog {
        req_id,
        method,
        path,
        user_agent,
        peer,
    } = log;
    let presented = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .unwrap_or(v)
                .trim()
                .to_owned()
        })
        .unwrap_or_default();
    if !presented.is_empty() && secret::verify(&presented, ctx.secret.as_ref()) {
        return None;
    }
    let presented_fp = secret::fingerprint(&presented);
    let expected_fp = secret::fingerprint(ctx.secret.as_ref().as_str());
    let secret_path = secret::secret_path()
        .map_or_else(|| "<no config dir>".to_owned(), |p| p.display().to_string());
    if presented.is_empty() {
        tracing::debug!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            peer = %peer,
            method = %method,
            path = %path,
            ua = %user_agent,
            "reject: missing loopback bearer (unauthenticated caller)"
        );
    } else {
        let remediation = secret::reapply_hint();
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
            secret_path = %secret_path,
            remediation = %remediation,
            "reject: stale loopback secret"
        );
        crate::activity::activity_log().append_warn(format!(
            "proxy: {method} {path} → 403 (stale secret; presented_fp={presented_fp} \
             expected_fp={expected_fp}; secret_path={secret_path}; {remediation}) [{req_id}]"
        ));
    }
    let body = if presented.is_empty() {
        no_credential_body(ctx)
    } else {
        mismatch_body(ctx)
    };
    let reason = if presented.is_empty() {
        "no-credential"
    } else {
        "secret-mismatch"
    };
    Some(rejection(body, reason))
}

// Why: the secret fingerprints stay in the logs — putting either in the body
// would let any loopback caller confirm a guessed secret.
fn mismatch_body(ctx: &ProxyContext) -> String {
    format!(
        "forbidden: bad loopback secret\n\
         \n\
         The credential presented does not match the loopback secret of the bridge\n\
         install answering on this port. This is a LOCAL bridge/port mismatch, not an\n\
         expired or wrong gateway API key, and not a region problem. Nothing in your\n\
         gateway credentials needs to change.\n\
         \n\
         this install: {config_dir} (port {port}, pid {pid})\n\
         \n\
         Your client was configured by a different bridge install. If you are running\n\
         two bridges on one machine (for example Windows alongside WSL2), they are\n\
         sharing this loopback port.\n\
         \n\
         remediation: {remediation}\n",
        config_dir = crate::proxy::identity::config_dir_display(),
        port = ctx.port,
        pid = std::process::id(),
        remediation = secret::reapply_hint(),
    )
}

fn no_credential_body(ctx: &ProxyContext) -> String {
    format!(
        "forbidden: no loopback credential presented\n\
         \n\
         This is the {app} loopback proxy. It requires an\n\
         Authorization: Bearer <loopback secret> header on every request.\n\
         \n\
         this install: {config_dir} (port {port})\n\
         \n\
         If your client was configured by a different bridge install, it is talking to\n\
         the wrong proxy.\n",
        app = crate::brand::brand().app_name,
        config_dir = crate::proxy::identity::config_dir_display(),
        port = ctx.port,
    )
}

fn rejection(body: String, reason: &'static str) -> Response<ProxyBody> {
    let mut resp = owned_response(StatusCode::FORBIDDEN, body);
    let headers = resp.headers_mut();
    headers.insert(
        "x-systemprompt-bridge-reason",
        http::HeaderValue::from_static(reason),
    );
    if let Ok(v) = http::HeaderValue::from_str(crate::proxy::identity::install_id().as_str()) {
        headers.insert("x-systemprompt-bridge-install", v);
    }
    if let Ok(v) = http::HeaderValue::from_str(&crate::proxy::identity::config_dir_display()) {
        headers.insert("x-systemprompt-bridge-config-dir", v);
    }
    resp
}
