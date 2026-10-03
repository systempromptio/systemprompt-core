//! Access-log records for gateway requests.
//!
//! A gateway request produces two records. The `headers` record is written by
//! the HTTP middleware when the response head is ready; for a streamed
//! response that is long before the outcome is known, so a `terminal` record
//! is written by [`log_gateway_terminal`] once the body has finished —
//! carrying the true status, the full elapsed time, and any upstream error.
//! Without the second record a stream that fails mid-body is logged as the 200
//! its headers promised.
//!
//! Timer-driven bridge routes are the exception. A bridge polls `profile`,
//! `profile/usage` and `heartbeat` on a fixed interval and checks `latest` and
//! `manifest` on its own schedule, so a successful hit on one of those is
//! evidence of nothing: persisting every one of them made those five routes
//! nine of every ten rows in a production `logs` table (392k of 439k in three
//! weeks) while the ten thousand inference calls the table exists to record sat
//! underneath. Successes on a polling route are emitted to the tracing
//! subscriber at debug and never reach the database
//! ([`persists_access_record`]); failures on the same routes still persist,
//! because a 401 heartbeat or a 502 update check is the signal an operator
//! looks for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_logging::{LogActor, LogEntry, LogLevel};

pub const PHASE_HEADERS: &str = "headers";
pub const PHASE_TERMINAL: &str = "terminal";
pub const LOG_TARGET: &str = "systemprompt_api::gateway";

const POLLING_ROUTES: [&str; 5] = [
    "/v1/bridge/profile",
    "/v1/bridge/profile/usage",
    "/v1/bridge/heartbeat",
    "/v1/bridge/latest",
    "/v1/bridge/manifest",
];

pub fn persists_access_record(path: &str, status: u16) -> bool {
    status >= 400 || !POLLING_ROUTES.contains(&path)
}

/// Method, path, and start instant captured by the gateway access-log
/// middleware, carried so terminal outcomes can be logged against the same
/// request line after the response body has finished streaming.
#[derive(Debug, Clone)]
pub struct GatewayAccessLog {
    pub method: String,
    pub path: String,
    pub started: Instant,
}

pub const fn level_for(status: u16) -> LogLevel {
    if status >= 500 {
        LogLevel::Error
    } else if status >= 400 {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}


#[derive(Debug)]
pub struct TerminalOutcome<'a> {
    pub access: &'a GatewayAccessLog,
    pub status: u16,
    pub actor: Option<LogActor>,
    pub error: Option<&'a str>,
}

pub fn log_gateway_terminal(outcome: TerminalOutcome<'_>) {
    let TerminalOutcome {
        access,
        status,
        actor,
        error,
    } = outcome;
    let elapsed_ms = access.started.elapsed().as_millis() as u64;
    let method = access.method.as_str();
    let path = access.path.as_str();
    let persist = persists_access_record(path, status);

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
    } else if persist {
        tracing::info!(method, path, status, elapsed_ms, "gateway stream completed");
    } else {
        tracing::debug!(method, path, status, elapsed_ms, "gateway poll completed");
    }

    if !persist {
        return;
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
        LOG_TARGET,
        format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
        actor,
    )
    .with_metadata(metadata);
    systemprompt_logging::enqueue_background(entry);
}
