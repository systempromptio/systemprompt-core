//! Loopback credential checks for proxy requests and the rejection bodies a
//! host UI can classify without parsing prose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};

use crate::proxy::credential::{self, LoopbackCredential, Rejection, RouteClass};
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

pub(super) fn reject_non_loopback(
    ctx: &ProxyContext,
    log: &RequestLog<'_>,
    host_hdr: &str,
) -> Response<ProxyBody> {
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
    ctx.deps.activity.append_warn(format!(
        "proxy: {method} {path} → 403 (non-loopback host: {host_hdr}) [{req_id}]"
    ));
    simple_response(StatusCode::FORBIDDEN, "forbidden: non-loopback host\n")
}

pub(super) fn verify_loopback_credential(
    req: &Request<Incoming>,
    ctx: &ProxyContext,
    log: &RequestLog<'_>,
) -> Result<LoopbackCredential, Box<Response<ProxyBody>>> {
    let presented = presented_bearer(req);
    let route = credential::classify_route(req.uri());
    let rejection = match credential::authenticate(&presented, ctx.secret.as_ref(), &route) {
        Ok(accepted) => return Ok(accepted),
        Err(rejection) => rejection,
    };
    log_rejection(ctx, log, &presented, &route, rejection);
    let body = match rejection {
        Rejection::NoCredential => no_credential_body(ctx),
        Rejection::SecretMismatch => mismatch_body(ctx),
        Rejection::ScopeMismatch => scope_mismatch_body(ctx, &route),
    };
    Err(Box::new(rejection_response(ctx, body, rejection)))
}

fn presented_bearer(req: &Request<Incoming>) -> String {
    req.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .unwrap_or(v)
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

fn log_rejection(
    ctx: &ProxyContext,
    log: &RequestLog<'_>,
    presented: &str,
    route: &RouteClass,
    rejection: Rejection,
) {
    let RequestLog {
        req_id,
        method,
        path,
        user_agent,
        peer,
    } = log;
    let presented_fp = secret::fingerprint(presented);
    match rejection {
        Rejection::NoCredential => tracing::debug!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            peer = %peer,
            method = %method,
            path = %path,
            ua = %user_agent,
            "reject: missing loopback bearer (unauthenticated caller)"
        ),
        Rejection::ScopeMismatch => {
            tracing::warn!(
                target: "systemprompt_bridge::proxy",
                req_id = %req_id,
                peer = %peer,
                method = %method,
                path = %path,
                ua = %user_agent,
                route = ?route,
                presented_fp = %presented_fp,
                "reject: credential does not fit the route"
            );
            ctx.deps.activity.append_warn(format!(
                "proxy: {method} {path} → 401 (credential out of scope for this route; \
                 presented_fp={presented_fp}) [{req_id}]"
            ));
        },
        Rejection::SecretMismatch => {
            let expected_fp = secret::fingerprint(ctx.secret.as_ref().as_str());
            let secret_path = secret::secret_path()
                .map_or_else(|| "<no config dir>".to_owned(), |p| p.display().to_string());
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
            ctx.deps.activity.append_warn(format!(
                "proxy: {method} {path} → 403 (stale secret; presented_fp={presented_fp} \
                 expected_fp={expected_fp}; secret_path={secret_path}; {remediation}) [{req_id}]"
            ));
        },
    }
}

fn scope_mismatch_body(ctx: &ProxyContext, route: &RouteClass) -> String {
    let accepted = match route {
        RouteClass::Hook(None) => {
            "the hook token of the plugin named by plugin_id, and this request names none"
                .to_owned()
        },
        RouteClass::Hook(Some(plugin)) => format!(
            "only the hook token issued for plugin {}; the loopback secret, another \
             plugin's hook token and a host token are refused here",
            plugin.as_str()
        ),
        RouteClass::Otel => "the loopback secret; a host token cannot emit telemetry".to_owned(),
        RouteClass::Inference | RouteClass::Mcp | RouteClass::Other => {
            "the loopback secret".to_owned()
        },
    };
    format!(
        "unauthorized: credential out of scope\n\
         \n\
         This route accepts {accepted}.\n\
         \n\
         this install: {config_dir} (port {port})\n\
         \n\
         remediation: {remediation}\n",
        config_dir = crate::proxy::identity::config_dir_display(),
        port = ctx.port,
        remediation = secret::reapply_hint(),
    )
}

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
         Authorization: Bearer <loopback credential> header on every request.\n\
         \n\
         this install: {config_dir} (port {port})\n\
         \n\
         Most often the client was never enrolled: `sync` writes its MCP connectors\n\
         without writing the provider block or the API key, so it reaches this port\n\
         with no credential at all.\n\
         \n\
         remediation: {remediation}\n\
         \n\
         If your client was configured by a different bridge install, it is talking to\n\
         the wrong proxy.\n",
        app = crate::brand::brand().app_name,
        config_dir = crate::proxy::identity::config_dir_display(),
        port = ctx.port,
        remediation = secret::reapply_hint(),
    )
}

fn rejection_response(
    ctx: &ProxyContext,
    body: String,
    rejection: Rejection,
) -> Response<ProxyBody> {
    let mut resp = owned_response(rejection.status(), body);
    let headers = resp.headers_mut();
    headers.insert(
        "x-systemprompt-bridge-reason",
        http::HeaderValue::from_static(rejection.reason()),
    );
    if let Ok(v) = http::HeaderValue::from_str(ctx.deps.install_id.as_str()) {
        headers.insert("x-systemprompt-bridge-install", v);
    }
    if let Ok(v) = http::HeaderValue::from_str(&crate::proxy::identity::config_dir_display()) {
        headers.insert("x-systemprompt-bridge-config-dir", v);
    }
    resp
}
