//! Provider failover for a route that names a `fallback_provider`.
//!
//! The primary upstream keeps its bounded retry budget for transient 429/503
//! answers. Once that budget is spent, or the provider answers any other 5xx,
//! or the connection itself fails, the request is re-bound to the fallback
//! provider and sent once more under the same retry policy. A per-provider
//! circuit breaker records those outcomes so a provider that keeps failing is
//! skipped outright — the fallback is tried first and the retry budget is not
//! spent on a dead upstream. Every failover is counted in
//! `gateway_upstream_failovers_total{from,to,reason}` and lands on the audit
//! row as `served_provider`, repriced at the serving provider's catalog rate.
//!
//! Failover never makes a request worse off: if the fallback cannot be bound
//! (credential, adapter, governance requirement, pricing), the primary's own
//! error is what the client sees, exactly as without a fallback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod breakers;
mod decision;

pub use self::breakers::ProviderBreakers;
pub use self::decision::{
    AttemptPlan, FailoverReason, failover_reason, is_failover_status, plan_attempts,
};

use systemprompt_database::resilience::Probe;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::services::ProviderRegistry;

use self::breakers::{acquire, breaker_settings, settle};
use super::DispatchError;
use super::pricing::failover_pricing;
use super::resolve::{ResolvedUpstream, resolve_fallback_upstream};
use super::stages::{ScannedDispatch, audit_upstream_failure};
use crate::services::gateway::audit::GatewayAudit;
use crate::services::gateway::protocol::outbound::OutboundOutcome;
use crate::services::gateway::protocol::outbound::retry::{current_policy, with_policy};

const FAILOVERS_TOTAL: &str = "gateway_upstream_failovers_total";

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
    let FailoverSend {
        registry,
        primary,
        forward_headers,
        ai_request_id,
        audit,
    } = send;
    let Some(fallback_name) = primary.route.fallback_provider.as_ref() else {
        return with_policy(
            current_policy(),
            scanned.send(primary, forward_headers, audit),
        )
        .await;
    };
    let attempt = Attempt {
        registry,
        primary,
        forward_headers,
        ai_request_id,
        audit,
        request_model: scanned.request_model().to_owned(),
        primary_name: primary.provider.name.as_str().to_owned(),
        fallback_name: fallback_name.as_str().to_owned(),
    };
    let breakers = ProviderBreakers::global();
    let primary_breaker = breakers.for_provider(
        &attempt.primary_name,
        &breaker_settings(&attempt.primary_name),
    );
    let fallback_breaker = breakers.for_provider(
        &attempt.fallback_name,
        &breaker_settings(&attempt.fallback_name),
    );
    let primary_probe = acquire(&primary_breaker);
    let fallback_probe = acquire(&fallback_breaker);
    match plan_attempts(true, primary_probe.is_none(), fallback_probe.is_none()) {
        AttemptPlan::PrimaryOnly => attempt.primary_only(scanned, primary_probe).await,
        AttemptPlan::FallbackOnly => attempt.fallback_first(scanned, fallback_probe).await,
        AttemptPlan::PrimaryThenFallback => {
            attempt
                .primary_then_fallback(scanned, primary_probe, fallback_probe)
                .await
        },
    }
}

struct Attempt<'a, 'r> {
    registry: &'r ProviderRegistry,
    primary: &'a ResolvedUpstream<'r>,
    forward_headers: &'a [(String, String)],
    ai_request_id: &'a AiRequestId,
    audit: &'a GatewayAudit,
    request_model: String,
    primary_name: String,
    fallback_name: String,
}

impl<'r> Attempt<'_, 'r> {
    async fn send_primary(&self, scanned: &ScannedDispatch) -> anyhow::Result<OutboundOutcome> {
        with_policy(
            current_policy(),
            scanned.send_attempt(self.primary, self.forward_headers, self.audit),
        )
        .await
    }

    async fn failed(&self, provider: &str, error: anyhow::Error) -> DispatchError {
        self.audit.mark_upstream_end();
        audit_upstream_failure(self.audit, provider, &self.request_model, &error).await;
        DispatchError::Recorded(error)
    }

    async fn primary_only(
        &self,
        scanned: &ScannedDispatch,
        probe: Option<Probe<'_>>,
    ) -> Result<OutboundOutcome, DispatchError> {
        match self.send_primary(scanned).await {
            Ok(outcome) => {
                settle(probe, true);
                Ok(outcome)
            },
            Err(error) => {
                settle(probe, failover_reason(&error).is_none());
                Err(self.failed(&self.primary_name, error).await)
            },
        }
    }

    async fn fallback_first(
        &self,
        scanned: &mut ScannedDispatch,
        probe: Option<Probe<'_>>,
    ) -> Result<OutboundOutcome, DispatchError> {
        match self
            .fallback(scanned, probe, FailoverReason::CircuitOpen)
            .await
        {
            Some(Ok(outcome)) => Ok(outcome),
            Some(Err(error)) => Err(self.failed(&self.fallback_name, error).await),
            None => self.primary_only(scanned, None).await,
        }
    }

    async fn primary_then_fallback(
        &self,
        scanned: &mut ScannedDispatch,
        primary_probe: Option<Probe<'_>>,
        fallback_probe: Option<Probe<'_>>,
    ) -> Result<OutboundOutcome, DispatchError> {
        let error = match self.send_primary(scanned).await {
            Ok(outcome) => {
                settle(primary_probe, true);
                return Ok(outcome);
            },
            Err(error) => error,
        };
        let reason = failover_reason(&error);
        settle(primary_probe, reason.is_none());
        let Some(reason) = reason else {
            return Err(self.failed(&self.primary_name, error).await);
        };
        match self.fallback(scanned, fallback_probe, reason).await {
            Some(Ok(outcome)) => Ok(outcome),
            Some(Err(fallback_error)) => {
                tracing::warn!(
                    ai_request_id = %self.ai_request_id,
                    primary = %self.primary_name,
                    fallback = %self.fallback_name,
                    primary_error = %error,
                    "Gateway failover attempt failed as well"
                );
                Err(self.failed(&self.fallback_name, fallback_error).await)
            },
            None => Err(self.failed(&self.primary_name, error).await),
        }
    }

    // Why: `None` means the fallback could not even be bound, and the caller
    // falls back to the primary's own verdict — a misconfigured fallback must
    // never turn a provider outage into a gateway-internal error.
    async fn fallback(
        &self,
        scanned: &mut ScannedDispatch,
        probe: Option<Probe<'_>>,
        reason: FailoverReason,
    ) -> Option<anyhow::Result<OutboundOutcome>> {
        let upstream = self.bind_fallback(scanned).await?;
        tracing::warn!(
            ai_request_id = %self.ai_request_id,
            from = %self.primary_name,
            to = %self.fallback_name,
            reason = %reason.label(),
            "Gateway failing over to fallback provider"
        );
        metrics::counter!(
            FAILOVERS_TOTAL,
            "from" => self.primary_name.clone(),
            "to" => self.fallback_name.clone(),
            "reason" => reason.label(),
        )
        .increment(1);
        let outcome = with_policy(
            current_policy(),
            scanned.send_attempt(&upstream, self.forward_headers, self.audit),
        )
        .await;
        let healthy = match &outcome {
            Ok(_) => true,
            Err(error) => failover_reason(error).is_none(),
        };
        settle(probe, healthy);
        Some(outcome)
    }

    async fn bind_fallback(&self, scanned: &mut ScannedDispatch) -> Option<ResolvedUpstream<'r>> {
        let bound = resolve_fallback_upstream(
            self.registry,
            self.primary,
            &self.request_model,
            self.ai_request_id,
        )
        .await;
        let upstream = match bound {
            Ok(upstream) => upstream?,
            Err(error) => return self.skip("fallback provider could not be bound", &error),
        };
        let pricing = match failover_pricing(&upstream, &self.request_model) {
            Ok(pricing) => pricing,
            Err(error) => {
                return self.skip("fallback provider has no pricing for the model", &error);
            },
        };
        if let Err(error) = scanned.rebind(&upstream, self.audit).await {
            return self.skip("request could not be rebuilt for the fallback wire", &error);
        }
        self.audit.reprice(pricing);
        self.audit
            .set_served_provider(upstream.provider.name.as_str())
            .await;
        if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
            self.audit.set_route_match(descriptor).await;
        }
        Some(upstream)
    }

    fn skip<T>(&self, what: &str, error: &dyn std::fmt::Display) -> Option<T> {
        tracing::warn!(
            ai_request_id = %self.ai_request_id,
            primary = %self.primary_name,
            fallback = %self.fallback_name,
            skipped = what,
            error = %error,
            "Gateway failover skipped"
        );
        None
    }
}
