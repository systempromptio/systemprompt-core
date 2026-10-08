//! The failover loop over a route's deployment chain: per-hop bind, send,
//! breaker settlement and the hop's metric, audit and `route_match` updates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::resilience::Probe;

use super::FailoverSend;
use super::breakers::{ProviderBreakers, acquire, breaker_settings, settle};
use super::decision::{FailoverReason, failover_reason, plan_attempts};
use crate::protocol::outbound::OutboundOutcome;
use crate::protocol::outbound::retry::{current_policy, with_policy};
use crate::service::pricing::failover_pricing;
use crate::service::resolve::{DeploymentHop, ResolvedUpstream, resolve_deployment_upstream};
use crate::service::stages::{ScannedDispatch, audit_upstream_failure};
use crate::service::{DispatchError, GatewayError};

const FAILOVERS_TOTAL: &str = "gateway_upstream_failovers_total";

pub(super) async fn run(
    scanned: &mut ScannedDispatch,
    send: FailoverSend<'_, '_>,
) -> Result<OutboundOutcome, DispatchError> {
    let names: Vec<String> = send
        .primary
        .deployments
        .iter()
        .map(|view| view.provider.as_str().to_owned())
        .collect();
    let breakers = ProviderBreakers::global();
    let handles: Vec<_> = names
        .iter()
        .map(|name| breakers.for_provider(name, &breaker_settings(name)))
        .collect();
    let mut probes: Vec<Option<Probe<'_>>> = handles.iter().map(|b| acquire(b)).collect();
    let tripped: Vec<bool> = probes.iter().map(Option::is_none).collect();
    let mut order = plan_attempts(&tripped);
    if !order.contains(&0) {
        order.push(0);
    }
    let mut chain = Chain {
        request_model: scanned.request_model().to_owned(),
        hops: vec![names[0].clone()],
        names,
        send,
        bound: Some(0),
        last: None,
    };
    for (position, index) in order.iter().copied().enumerate() {
        let forced = index == 0 && position > 0 && tripped[0];
        if forced && chain.last.is_some() {
            break;
        }
        let probe = probes.get_mut(index).and_then(Option::take);
        match chain.attempt(scanned, index, probe).await {
            Step::Served(outcome) => return Ok(outcome),
            Step::Fatal(provider, error) => return Err(chain.failed(&provider, error).await),
            Step::Next => {},
        }
    }
    match chain.last.take() {
        Some((provider, error)) => Err(chain.failed(&provider, error).await),
        None => Err(DispatchError::Recorded(GatewayError::internal(
            "gateway failover",
            "no deployment of the route could be bound",
        ))),
    }
}

enum Step {
    Served(OutboundOutcome),
    Fatal(String, GatewayError),
    Next,
}

struct Chain<'a, 'r> {
    send: FailoverSend<'a, 'r>,
    request_model: String,
    names: Vec<String>,
    hops: Vec<String>,
    bound: Option<usize>,
    last: Option<(String, GatewayError)>,
}

impl<'r> Chain<'_, 'r> {
    async fn attempt(
        &mut self,
        scanned: &mut ScannedDispatch,
        index: usize,
        probe: Option<Probe<'_>>,
    ) -> Step {
        let name = self.names[index].clone();
        let hop = index != 0 || self.last.is_some();
        let rebound = if index == 0 && self.bound == Some(0) {
            None
        } else {
            match self.bind(scanned, index).await {
                Some(upstream) => Some(upstream),
                None => return Step::Next,
            }
        };
        if hop {
            self.record_hop(&name);
        }
        let upstream = rebound.as_ref().unwrap_or(self.send.primary);
        let _in_flight = super::enter_deployment(upstream, &self.request_model);
        let outcome = with_policy(
            current_policy(),
            scanned.send_attempt(upstream, self.send.forward_headers, self.send.audit),
        )
        .await;
        match outcome {
            Ok(outcome) => {
                settle(probe, true);
                Step::Served(outcome)
            },
            Err(error) => {
                let reason = failover_reason(&error);
                settle(probe, reason.is_none());
                if reason.is_none() {
                    return Step::Fatal(name, error);
                }
                if let Some((previous, previous_error)) = self.last.as_ref() {
                    tracing::warn!(
                        ai_request_id = %self.send.ai_request_id,
                        previous = %previous,
                        deployment = %name,
                        previous_error = %previous_error,
                        "Gateway failover attempt failed as well"
                    );
                }
                self.last = Some((name, error));
                Step::Next
            },
        }
    }

    fn record_hop(&self, to: &str) {
        let (from, reason) = match self.last.as_ref() {
            Some((from, error)) => (
                from.clone(),
                failover_reason(error).unwrap_or(FailoverReason::Transport),
            ),
            None => (self.names[0].clone(), FailoverReason::CircuitOpen),
        };
        tracing::warn!(
            ai_request_id = %self.send.ai_request_id,
            from = %from,
            to = %to,
            reason = %reason.label(),
            "Gateway failing over to the next deployment"
        );
        metrics::counter!(
            FAILOVERS_TOTAL,
            "from" => from,
            "to" => to.to_owned(),
            "reason" => reason.label(),
        )
        .increment(1);
    }

    async fn failed(&self, provider: &str, error: GatewayError) -> DispatchError {
        self.send.audit.mark_upstream_end();
        audit_upstream_failure(self.send.audit, provider, &self.request_model, &error).await;
        DispatchError::Recorded(error)
    }

    async fn bind(
        &mut self,
        scanned: &mut ScannedDispatch,
        index: usize,
    ) -> Option<ResolvedUpstream<'r>> {
        let mut hops = self.hops.clone();
        hops.push(self.names[index].clone());
        let bound = resolve_deployment_upstream(
            self.send.registry,
            self.send.primary,
            DeploymentHop { index, hops: &hops },
            &self.request_model,
            self.send.ai_request_id,
        )
        .await;
        let upstream = match bound {
            Ok(upstream) => upstream?,
            Err(error) => return self.skip(index, "deployment could not be bound", &error),
        };
        let pricing = match failover_pricing(&upstream, &self.request_model) {
            Ok(pricing) => pricing,
            Err(error) => {
                return self.skip(index, "deployment has no pricing for the model", &error);
            },
        };
        if let Err(error) = scanned.rebind(&upstream, self.send.audit).await {
            self.bound = None;
            return self.skip(
                index,
                "request could not be rebuilt for the deployment wire",
                &error,
            );
        }
        self.bound = Some(index);
        self.hops = hops;
        let audit = self.send.audit;
        audit.reprice(pricing);
        audit
            .set_served_provider(upstream.provider.name.as_str())
            .await;
        if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
            audit.set_route_match(descriptor).await;
        }
        Some(upstream)
    }

    fn skip<T>(&self, index: usize, what: &str, error: &dyn std::fmt::Display) -> Option<T> {
        tracing::warn!(
            ai_request_id = %self.send.ai_request_id,
            primary = %self.names[0],
            deployment = %self.names[index],
            skipped = what,
            error = %error,
            "Gateway failover skipped"
        );
        None
    }
}
