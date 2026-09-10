//! Post-response quota accounting outcomes.
//!
//! The response has already been served by the time accounting runs, so a
//! failed write cannot deny the request. Under `Closed` the request is instead
//! recorded as failed, so uncounted spend is visible in the audit trail rather
//! than only in a log line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::QuotaFaultMode;

use super::super::super::audit::GatewayAudit;
use super::super::super::quota;

pub async fn record_accounting_outcome(
    audit: &GatewayAudit,
    fault_mode: QuotaFaultMode,
    outcome: quota::AccountingOutcome,
) {
    let quota::AccountingOutcome::Faulted { message } = outcome else {
        return;
    };
    if !fault_mode.is_closed() {
        tracing::warn!(
            ai_request_id = %audit.ctx.ai_request_id,
            reason = %message,
            fault_mode = fault_mode.as_str(),
            "Gateway quota accounting failed; spend is not counted against the ceiling"
        );
        return;
    }
    tracing::error!(
        ai_request_id = %audit.ctx.ai_request_id,
        reason = %message,
        fault_mode = fault_mode.as_str(),
        "Gateway quota accounting failed; recording the request as failed"
    );
    if let Err(e) = audit.fail(&message).await {
        tracing::warn!(error = %e, "quota accounting audit fail failed");
    }
}
