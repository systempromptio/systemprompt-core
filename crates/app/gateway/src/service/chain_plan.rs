//! Planning a resolved chain before anything is sent: the deployment
//! selection strategy and the context-window pre-check.
//!
//! [`order_chain`] reorders the chain's hops so the deployment the route's
//! `strategy` selects leads; failover then walks the reordered chain. Every
//! selection is counted in `gateway_deployment_selected_total{route,provider,
//! strategy}`, and a non-`ordered` strategy adds `strategy:<name>` to
//! `route_match`. Breaker and load state are keyed per endpoint.
//!
//! [`fit_context_window`] estimates the request's input tokens with the
//! provider-neutral text estimate and compares it with the leading
//! deployment's catalog context window (the hop's model `limits`). A request
//! that fits keeps its chain,
//! minus any later deployment whose known window it would not fit. One that
//! does not is sent down the chain's `context_fallbacks` that fit it (counted
//! in `gateway_context_fallbacks_total{from,to}`, audited as
//! `context_window:a->b`), or refused with [`ContextWindowExceeded`] before
//! any upstream call. On a hop whose wire forwards the caller's bytes as-is
//! ([`SentBody`]), the estimate is the larger of the text estimate and the
//! byte estimate of the forwarded body's text, binary media excluded, so
//! content the canonical model does not carry is still counted. A model with no
//! declared window is never refused. The requested `max_tokens` is not checked
//! here: the outbound codecs clamp it to the served model's output ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RouteId;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop};
use systemprompt_manifest::services::{SelectionStrategy, estimate_input_tokens};

use super::ContextWindowExceeded;
use super::failover::{
    DeploymentLoad, DeploymentState, ProviderBreakers, breaker_key, plan_selection,
};
use crate::protocol::canonical::CanonicalRequest;
// JSON: protocol boundary — the forwarded body is dynamic wire JSON.
use crate::quota::input_tokens_for_bytes;
use serde_json::Value;
use systemprompt_wire::{WireProtocol, inspect};

const DEPLOYMENT_SELECTED_TOTAL: &str = "gateway_deployment_selected_total";
const CONTEXT_FALLBACKS_TOTAL: &str = "gateway_context_fallbacks_total";

/// The chain to dispatch and the `route_match` segment planning produced.
#[derive(Debug)]
pub struct PlannedChain {
    pub deployments: Vec<ResolvedHop>,
    pub descriptor: Option<String>,
}

#[must_use]
pub fn order_chain(
    deployments: Vec<ResolvedHop>,
    strategy: SelectionStrategy,
    route: Option<&RouteId>,
    breakers: &ProviderBreakers,
    load: &DeploymentLoad,
) -> PlannedChain {
    let deployments = if deployments.len() > 1 && !strategy.is_ordered() {
        let states: Vec<DeploymentState> = deployments
            .iter()
            .map(|hop| deployment_state(hop, breakers, load))
            .collect();
        let order = plan_selection(strategy, &states, rand::random());
        let mut slots: Vec<Option<ResolvedHop>> = deployments.into_iter().map(Some).collect();
        order
            .into_iter()
            .filter_map(|i| slots.get_mut(i).and_then(Option::take))
            .collect()
    } else {
        deployments
    };
    if let Some(selected) = deployments.first() {
        metrics::counter!(
            DEPLOYMENT_SELECTED_TOTAL,
            "route" => route.map_or_else(String::new, ToString::to_string),
            "provider" => selected.provider.as_str().to_owned(),
            "strategy" => strategy.as_str(),
        )
        .increment(1);
    }
    PlannedChain {
        deployments,
        descriptor: (strategy != SelectionStrategy::Ordered)
            .then(|| format!("strategy:{strategy}")),
    }
}

fn deployment_state(
    hop: &ResolvedHop,
    breakers: &ProviderBreakers,
    load: &DeploymentLoad,
) -> DeploymentState {
    let key = breaker_key(hop);
    let tripped = breakers.for_endpoint(&key, &hop.resilience).is_open();
    DeploymentState {
        tripped,
        weight: hop.effective_weight(),
        in_flight: load.in_flight(&key),
    }
}

/// The size of a body forwarded unchanged on `passthrough_wire`.
///
/// `text_bytes` is the decoded length of the body's text strings and keys;
/// encoded binary media (`inspect::is_binary_source`) counts as zero and is
/// left to the upstream to price.
#[derive(Debug, Clone, Copy)]
pub struct SentBody {
    passthrough_wire: Option<WireProtocol>,
    text_bytes: usize,
}

impl SentBody {
    #[must_use]
    pub fn measure(passthrough_wire: Option<WireProtocol>, raw: &[u8]) -> Self {
        let text_bytes = passthrough_wire.map_or(0, |_| {
            serde_json::from_slice::<Value>(raw)
                .map_or(raw.len(), |root| inspect::text_bytes(&root))
        });
        Self {
            passthrough_wire,
            text_bytes,
        }
    }
}

struct InputEstimate {
    canonical: u32,
    passthrough: Option<(WireProtocol, u32)>,
}

impl InputEstimate {
    fn new(request: &CanonicalRequest, sent: SentBody) -> Self {
        Self {
            canonical: estimate_input_tokens(request),
            passthrough: sent
                .passthrough_wire
                .map(|wire| (wire, input_tokens_for_bytes(sent.text_bytes))),
        }
    }

    fn for_hop(&self, hop: &ResolvedHop) -> u32 {
        match self.passthrough {
            Some((wire, bytes)) if wire == hop.wire => self.canonical.max(bytes),
            _ => self.canonical,
        }
    }
}

pub fn fit_context_window(
    catalog: &ResolvedCatalog,
    request: &CanonicalRequest,
    sent: SentBody,
    deployments: Vec<ResolvedHop>,
    context_fallbacks: Vec<ResolvedHop>,
) -> Result<PlannedChain, ContextWindowExceeded> {
    let estimates = InputEstimate::new(request, sent);
    let model = request.model.as_str();
    let fits = |hop: &ResolvedHop| {
        context_window(catalog, hop).is_none_or(|(limit, _)| estimates.for_hop(hop) <= limit)
    };
    let overflow = deployments
        .first()
        .and_then(|primary| {
            context_window(catalog, primary).map(|window| (window, estimates.for_hop(primary)))
        })
        .filter(|((limit, _), estimate)| estimate > limit);
    let Some(((limit, served), estimate)) = overflow else {
        let deployments = deployments
            .into_iter()
            .enumerate()
            .filter(|(i, hop)| *i == 0 || fits(hop))
            .map(|(_, hop)| hop)
            .collect();
        return Ok(PlannedChain {
            deployments,
            descriptor: None,
        });
    };
    let fallbacks: Vec<ResolvedHop> = context_fallbacks
        .into_iter()
        .filter(|hop| {
            context_window(catalog, hop).is_some_and(|(window, _)| estimates.for_hop(hop) <= window)
        })
        .collect();
    let (Some(from), Some(to)) = (deployments.first(), fallbacks.first()) else {
        return Err(ContextWindowExceeded {
            estimate,
            limit,
            model: served,
        });
    };
    let from = from.endpoint_ref().to_string();
    let to = to.endpoint_ref().to_string();
    tracing::info!(
        model = %model,
        estimate,
        limit,
        from = %from,
        to = %to,
        "Gateway request exceeds the deployment's context window; using a context fallback"
    );
    metrics::counter!(CONTEXT_FALLBACKS_TOTAL, "from" => from.clone(), "to" => to.clone())
        .increment(1);
    Ok(PlannedChain {
        deployments: fallbacks,
        descriptor: Some(format!("context_window:{from}->{to}")),
    })
}

pub(super) fn join_segments(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(a), Some(b)) => Some(format!("{a};{b}")),
        (a, b) => a.or(b),
    }
}

fn context_window(catalog: &ResolvedCatalog, hop: &ResolvedHop) -> Option<(u32, String)> {
    let limit = catalog.model(hop.model.as_str())?.limits.context_window;
    (limit > 0).then(|| (limit, hop.upstream_model.clone()))
}
