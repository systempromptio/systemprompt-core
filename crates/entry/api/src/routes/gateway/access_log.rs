//! Gateway access-log middleware and the response extension carrying the
//! authenticated identity it logs against.
//!
//! The middleware writes the `headers` record of a gateway request; the
//! `terminal` record, the polling-route policy and the record shape live in
//! [`systemprompt_gateway::audit::access_log`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use systemprompt_gateway::audit::access_log::{
    GatewayAccessLog, LOG_TARGET, PHASE_HEADERS, level_for, persists_access_record,
};
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LoggingRuntime};

#[derive(Debug, Clone)]
pub(crate) struct GatewayLogIdentity {
    pub user: UserId,
    pub session: SessionId,
    pub trace: TraceId,
}

fn gateway_log_actor(logging: &LoggingRuntime, resp: &Response) -> LogActor {
    resp.extensions().get::<GatewayLogIdentity>().map_or_else(
        || logging.platform_actor(TraceId::system()),
        |identity| {
            LogActor::new(
                identity.user.clone(),
                identity.session.clone(),
                identity.trace.clone(),
            )
        },
    )
}

pub(super) async fn log_gateway_request(
    State(logging): State<LoggingRuntime>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    // Why: Axum strips the mount prefix from `req.uri()` inside a nested router.
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
    let persist = persists_access_record(&path, status);

    if status >= 500 {
        tracing::error!(method = %method, path = %path, status, elapsed_ms, "gateway request failed");
    } else if status >= 400 {
        tracing::warn!(method = %method, path = %path, status, elapsed_ms, "gateway request rejected");
    } else if persist {
        tracing::info!(method = %method, path = %path, status, elapsed_ms, "gateway request");
    } else {
        tracing::debug!(method = %method, path = %path, status, elapsed_ms, "gateway poll");
    }

    if !persist {
        return resp;
    }

    let entry = LogEntry::new(
        level,
        LOG_TARGET,
        format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
        gateway_log_actor(&logging, &resp),
    )
    .with_metadata(metadata);
    logging.enqueue(entry);

    resp
}
