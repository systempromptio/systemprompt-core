//! Blocking on a held call until a human answers it.
//!
//! The wait is a poll rather than a `LISTEN`/`NOTIFY` or an in-process
//! `Notify`: the MCP server that parks the call and the admin console that
//! resolves it are separate processes, so no in-process primitive can carry
//! the wake-up, and a dedicated listener connection per held call costs more
//! than a query every [`POLL_INTERVAL`]. The bound on total polling is
//! `hold_seconds`, not the approval's lifetime — an unanswered call is handed
//! back to the client as an MRTR round and re-enters the wait on retry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use super::repository::{ApprovalRepository, ApprovalRequest, ApprovalStatus};

/// How often the waiter re-reads the row. Fast enough that an approval feels
/// immediate to whoever clicked it, slow enough that a held call costs ~2
/// queries a second.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How a wait ended.
#[derive(Debug, Clone)]
pub enum ApprovalOutcome {
    /// A human approved it. Carries the row so the caller can stamp the
    /// approver into the audit.
    Approved(Box<ApprovalRequest>),
    /// A human refused it.
    Denied(Box<ApprovalRequest>),
    /// Nobody answered within `expires_at`; the call is abandoned.
    Expired(Box<ApprovalRequest>),
    /// Nobody answered within this round's `hold_seconds`, but the approval is
    /// still open. The caller should hand the wait back to the client and
    /// re-enter on retry.
    StillPending(Box<ApprovalRequest>),
}

/// Polls one approval until it resolves, `hold` elapses, or it expires.
///
/// A transient read error does not end the wait: the approval is the durable
/// state, and failing the call on a blip would turn a recoverable database
/// hiccup into a refused tool call. Errors are logged and retried until the
/// hold budget runs out, at which point the call falls back to `StillPending`
/// and the client retries.
pub async fn wait_for_decision(
    repo: &ApprovalRepository,
    call_id: &str,
    hold: Duration,
) -> ApprovalOutcome {
    let deadline = tokio::time::Instant::now() + hold;
    let mut last_seen: Option<ApprovalRequest> = None;

    loop {
        match repo.find(call_id).await {
            Ok(Some(request)) => {
                match request.status {
                    ApprovalStatus::Approved => {
                        return ApprovalOutcome::Approved(Box::new(request));
                    },
                    ApprovalStatus::Denied => {
                        return ApprovalOutcome::Denied(Box::new(request));
                    },
                    ApprovalStatus::Expired => {
                        return ApprovalOutcome::Expired(Box::new(request));
                    },
                    ApprovalStatus::Pending => {
                        // The sweep job may not have run; the deadline on the
                        // row is what actually decides, not the status column.
                        if request.expires_at <= chrono::Utc::now() {
                            return ApprovalOutcome::Expired(Box::new(request));
                        }
                        last_seen = Some(request);
                    },
                }
            },
            Ok(None) => {
                tracing::error!(
                    call_id,
                    "approval row vanished while a call was waiting on it; \
                     treating the call as denied"
                );
                return last_seen.map_or_else(
                    || ApprovalOutcome::Expired(Box::new(missing_placeholder(call_id))),
                    |r| ApprovalOutcome::Denied(Box::new(r)),
                );
            },
            Err(err) => {
                tracing::warn!(
                    call_id,
                    error = %err,
                    "could not read the approval row; retrying within the hold budget"
                );
            },
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            return last_seen.map_or_else(
                || ApprovalOutcome::Expired(Box::new(missing_placeholder(call_id))),
                |r| ApprovalOutcome::StillPending(Box::new(r)),
            );
        }
        tokio::time::sleep(POLL_INTERVAL.min(deadline - now)).await;
    }
}

// Why: the outcome enum carries the row so callers can stamp an approver, but
// the two failure paths above have no row to carry. A synthetic expired row
// keeps those callers total rather than making every match arm handle a
// second layer of `Option`.
fn missing_placeholder(call_id: &str) -> ApprovalRequest {
    let now = chrono::Utc::now();
    ApprovalRequest {
        call_id: call_id.to_owned(),
        tool_name: String::new(),
        server_name: String::new(),
        arguments: serde_json::Value::Null,
        args_digest: String::new(),
        requested_by: String::new(),
        session_id: None,
        trace_id: None,
        rule: String::new(),
        status: ApprovalStatus::Expired,
        approver_id: None,
        approver_username: None,
        decided_at: Some(now),
        decision_note: Some("approval record unavailable".to_owned()),
        expires_at: now,
        created_at: now,
    }
}
