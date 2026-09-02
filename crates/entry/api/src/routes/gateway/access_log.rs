//! Gateway access-log middleware and the response extension carrying the
//! authenticated identity it logs against.
//!
//! A gateway request produces two records. The `headers` record is written when
//! the response head is ready; for a streamed response that is long before the
//! outcome is known, so a `terminal` record is written once the body has
//! finished — carrying the true status, the full elapsed time, and any upstream
//! error. Without the second record a stream that fails mid-body is logged as
//! the 200 its headers promised.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel};

use crate::services::gateway::audit::GatewayAccessLog;

pub(crate) const PHASE_HEADERS: &str = "headers";
pub(crate) const PHASE_TERMINAL: &str = "terminal";

#[derive(Debug, Clone)]
pub(crate) struct GatewayLogIdentity {
    pub user: UserId,
    pub session: SessionId,
    pub trace: TraceId,
}

fn gateway_log_actor(resp: &Response) -> Option<LogActor> {
    if let Some(identity) = resp.extensions().get::<GatewayLogIdentity>() {
        return Some(LogActor::new(
            identity.user.clone(),
            identity.session.clone(),
            identity.trace.clone(),
        ));
    }
    match LogActor::platform(TraceId::system()) {
        Ok(actor) => Some(actor),
        Err(e) => {
            tracing::warn!(error = %e, "gateway access log skipped: system admin not initialized");
            None
        },
    }
}

pub(super) async fn log_gateway_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    // Why: the router is nested under GATEWAY_BASE, so req.uri() arrives with
    // the prefix already stripped; log the path the client actually requested.
    let path = req
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or_else(
            || {
                format!(
                    "{}{}",
                    systemprompt_models::ApiPaths::GATEWAY_BASE,
                    req.uri().path()
                )
            },
            |orig| orig.path().to_owned(),
        );
    let started = Instant::now();
    let mut req = req;
    req.extensions_mut().insert(GatewayAccessLog {
        method: method.to_string(),
        path: path.clone(),
        started,
    });
    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let metadata = serde_json::json!({
        "kind": "access_log",
        "method": method.to_string(),
        "path": path,
        "status": status,
        "elapsed_ms": elapsed_ms,
        "phase": PHASE_HEADERS,
    });

    let level = level_for(status);

    if status >= 500 {
        tracing::error!(method = %method, path = %path, status, elapsed_ms, "gateway request failed");
    } else if status >= 400 {
        tracing::warn!(method = %method, path = %path, status, elapsed_ms, "gateway request rejected");
    } else {
        tracing::info!(method = %method, path = %path, status, elapsed_ms, "gateway request");
    }

    if let Some(actor) = gateway_log_actor(&resp) {
        let entry = LogEntry::new(
            level,
            "systemprompt_api::gateway",
            format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
            actor,
        )
        .with_metadata(metadata);
        systemprompt_logging::enqueue_background(entry);
    }

    resp
}

const fn level_for(status: u16) -> LogLevel {
    if status >= 500 {
        LogLevel::Error
    } else if status >= 400 {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}

#[derive(Debug)]
pub(crate) struct TerminalOutcome<'a> {
    pub access: &'a GatewayAccessLog,
    pub status: u16,
    pub actor: Option<LogActor>,
    pub error: Option<&'a str>,
}

pub(crate) fn log_gateway_terminal(outcome: TerminalOutcome<'_>) {
    let TerminalOutcome {
        access,
        status,
        actor,
        error,
    } = outcome;
    let elapsed_ms = access.started.elapsed().as_millis() as u64;
    let method = access.method.as_str();
    let path = access.path.as_str();

    if status >= 500 {
        tracing::error!(
            method,
            path,
            status,
            elapsed_ms,
            error,
            "gateway stream failed"
        );
    } else if status >= 400 {
        tracing::warn!(
            method,
            path,
            status,
            elapsed_ms,
            error,
            "gateway stream aborted"
        );
    } else {
        tracing::info!(method, path, status, elapsed_ms, "gateway stream completed");
    }

    let Some(actor) = actor else {
        return;
    };
    let metadata = serde_json::json!({
        "kind": "access_log",
        "method": method,
        "path": path,
        "status": status,
        "elapsed_ms": elapsed_ms,
        "phase": PHASE_TERMINAL,
        "error": error,
    });
    let entry = LogEntry::new(
        level_for(status),
        "systemprompt_api::gateway",
        format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
        actor,
    )
    .with_metadata(metadata);
    systemprompt_logging::enqueue_background(entry);
}
