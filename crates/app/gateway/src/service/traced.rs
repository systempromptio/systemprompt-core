//! The upstream send under the request's trace context, timed as the
//! upstream-first-byte phase.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::DispatchError;
use super::failover::{FailoverSend, send_with_failover};
use super::stages::ScannedDispatch;
use crate::audit::GatewayRequestContext;
use crate::audit::phases::GatewayPhase;
use crate::protocol::outbound::OutboundOutcome;

pub(super) async fn send_traced(
    scanned: &mut ScannedDispatch,
    send: FailoverSend<'_, '_>,
    ctx: &GatewayRequestContext,
    trace_sampled: Option<bool>,
) -> Result<OutboundOutcome, DispatchError> {
    let admitted_at = std::time::Instant::now();
    let outcome = crate::protocol::outbound::trace_context::with_trace(
        ctx.trace_id.clone(),
        trace_sampled,
        Box::pin(send_with_failover(scanned, send)),
    )
    .await?;
    crate::audit::phases::record(GatewayPhase::UpstreamFirstByte, admitted_at.elapsed());
    Ok(outcome)
}
