//! Cancellation guard for a dispatch that has opened its audit row.
//!
//! `open_audit` writes the `ai_requests` row as `pending`, and from then on
//! exactly one party must close it: an error path calls `audit.fail`, a
//! buffered reply spawns its completion task, a stream hands the audit to the
//! tap. None of those run when the dispatch future is *cancelled* — hyper
//! drops it the moment the client hangs up — so a client that disconnected
//! while the upstream was still thinking (a reasoning model sitting seconds
//! before its first byte) left the row `pending` forever. The guard is armed
//! after the row opens, disarmed once ownership has moved on, and its `Drop`
//! is the only path left when the future is dropped mid-flight.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::super::audit::GatewayAudit;
use super::super::stream_tap::log_terminal;

pub const ABANDONED_REASON: &str = "client disconnected before upstream responded";

const CLIENT_CLOSED_REQUEST: u16 = 499;

/// The armed/disarmed state of an `AbandonGuard`, kept separate so the
/// once-only firing rule is testable without a database-backed audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arming {
    armed: bool,
}

impl Arming {
    pub const fn armed() -> Self {
        Self { armed: true }
    }

    pub const fn disarm(&mut self) {
        self.armed = false;
    }

    pub const fn take(&mut self) -> bool {
        let fires = self.armed;
        self.armed = false;
        fires
    }
}

pub(super) struct AbandonGuard {
    audit: Arc<GatewayAudit>,
    arming: Arming,
}

impl AbandonGuard {
    pub(super) const fn arm(audit: Arc<GatewayAudit>) -> Self {
        Self {
            audit,
            arming: Arming::armed(),
        }
    }

    pub(super) const fn disarm(&mut self) {
        self.arming.disarm();
    }
}

impl Drop for AbandonGuard {
    fn drop(&mut self) {
        if !self.arming.take() {
            return;
        }
        let audit = Arc::clone(&self.audit);
        audit.mark_upstream_end();
        log_terminal(&audit, CLIENT_CLOSED_REQUEST, Some(ABANDONED_REASON));
        tokio::spawn(async move {
            if let Err(e) = audit.fail(ABANDONED_REASON).await {
                tracing::warn!(error = %e, "abandoned dispatch audit fail failed");
            }
        });
    }
}
