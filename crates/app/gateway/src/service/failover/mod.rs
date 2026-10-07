//! Ordered multi-deployment failover for a route whose chain lists
//! `fallbacks`.
//!
//! A route's chain is its primary deployment followed by its ordered
//! fallbacks (or the per-scope chain `by_scope` selected). Each deployment is
//! a provider entry and sits behind that provider's circuit breaker, so a
//! breaker is per deployment. The attempt order is decided before anything is
//! sent: healthy deployments in chain order, or every deployment in chain
//! order when none is healthy, so a request is never left unsent.
//!
//! Each deployment keeps the bounded retry budget for transient 429/503
//! answers. Once that budget is spent, or the deployment answers any other
//! 5xx, or the connection itself fails, the request is re-bound to the next
//! deployment and sent again under the same retry policy. Every hop is counted
//! in `gateway_upstream_failovers_total{from,to,reason}`, lands on the audit
//! row as `served_provider` repriced at the serving deployment's catalog
//! rate, and extends the `failover:a->b->c` segment of `route_match`.
//!
//! Failover never makes a request worse off: a deployment that cannot be
//! bound (credential, adapter, governance requirement, pricing) is skipped,
//! and the client sees the last real upstream verdict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod breakers;
mod chain;
mod decision;

pub use self::breakers::ProviderBreakers;
pub use self::decision::{FailoverReason, failover_reason, is_failover_status, plan_attempts};

use systemprompt_identifiers::AiRequestId;
use systemprompt_manifest::services::ProviderRegistry;

use super::DispatchError;
use super::resolve::ResolvedUpstream;
use super::stages::ScannedDispatch;
use crate::audit::GatewayAudit;
use crate::protocol::outbound::OutboundOutcome;
use crate::protocol::outbound::retry::{current_policy, with_policy};

pub(super) struct FailoverSend<'a, 'r> {
    pub registry: &'r ProviderRegistry,
    pub primary: &'a ResolvedUpstream<'r>,
    pub forward_headers: &'a [(String, String)],
    pub ai_request_id: &'a AiRequestId,
    pub audit: &'a GatewayAudit,
}

pub(super) async fn send_with_failover(
    scanned: &mut ScannedDispatch,
    send: FailoverSend<'_, '_>,
) -> Result<OutboundOutcome, DispatchError> {
    if send.primary.deployments.len() <= 1 {
        return with_policy(
            current_policy(),
            scanned.send(send.primary, send.forward_headers, send.audit),
        )
        .await;
    }
    chain::run(scanned, send).await
}
