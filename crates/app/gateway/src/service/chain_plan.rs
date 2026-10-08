//! Planning a resolved chain before anything is sent: the deployment
//! selection strategy and the context-window pre-check.
//!
//! [`order_chain`] reorders the chain's views so the deployment the chain's
//! `strategy` selects leads; failover then walks the reordered chain. Every
//! selection is counted in `gateway_deployment_selected_total{route,provider,
//! strategy}`, and a non-`ordered` strategy adds `strategy:<name>` to
//! `route_match`.
//!
//! [`fit_context_window`] estimates the request's input tokens with the
//! provider-neutral text estimate and compares it with the leading
//! deployment's catalog context window. A request that fits keeps its chain,
//! minus any later deployment whose known window it would not fit. One that
//! does not is sent down the chain's `context_fallbacks` that fit it (counted
//! in `gateway_context_fallbacks_total{from,to}`, audited as
//! `context_window:a->b`), or refused with [`ContextWindowExceeded`] before
//! any upstream call. A model with no declared window is never refused. The
//! requested `max_tokens` is not checked here: the outbound codecs clamp it
//! to the served model's output ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::{
    GatewayRoute, ProviderRegistry, SelectionStrategy, estimate_input_tokens,
};

use super::ContextWindowExceeded;
use super::failover::{
    DeploymentLoad, DeploymentState, ProviderBreakers, breaker_settings, plan_selection,
};
use crate::protocol::canonical::CanonicalRequest;

const DEPLOYMENT_SELECTED_TOTAL: &str = "gateway_deployment_selected_total";
const CONTEXT_FALLBACKS_TOTAL: &str = "gateway_context_fallbacks_total";

/// The chain to dispatch and the `route_match` segment planning produced.
#[derive(Debug)]
pub struct PlannedChain {
    pub deployments: Vec<GatewayRoute>,
    pub descriptor: Option<String>,
}

#[must_use]
pub fn order_chain(deployments: Vec<GatewayRoute>, request_model: &str) -> PlannedChain {
    let Some(strategy) = deployments.first().map(|d| d.strategy) else {
        return PlannedChain {
            deployments,
            descriptor: None,
        };
    };
    let deployments = if deployments.len() > 1 && !strategy.is_ordered() {
        let states: Vec<DeploymentState> = deployments
            .iter()
            .map(|view| deployment_state(view, request_model))
            .collect();
        let order = plan_selection(strategy, &states, rand::random());
        let mut slots: Vec<Option<GatewayRoute>> = deployments.into_iter().map(Some).collect();
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
            "route" => selected.effective_id().to_string(),
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

fn deployment_state(view: &GatewayRoute, request_model: &str) -> DeploymentState {
    let provider = view.provider.as_str();
    let tripped = ProviderBreakers::global()
        .for_provider(provider, &breaker_settings(provider))
        .is_open();
    let key = DeploymentLoad::key(provider, view.effective_upstream_model(request_model));
    DeploymentState {
        tripped,
        weight: view.effective_weight(),
        in_flight: DeploymentLoad::global().in_flight(&key),
    }
}

pub fn fit_context_window(
    registry: &ProviderRegistry,
    request: &CanonicalRequest,
    deployments: Vec<GatewayRoute>,
    context_fallbacks: Vec<GatewayRoute>,
) -> Result<PlannedChain, ContextWindowExceeded> {
    let estimate = estimate_input_tokens(request);
    let model = request.model.as_str();
    let fits = |view: &GatewayRoute| {
        context_window(registry, view, model).is_none_or(|(limit, _)| estimate <= limit)
    };
    let overflow = deployments
        .first()
        .and_then(|primary| context_window(registry, primary, model))
        .filter(|(limit, _)| estimate > *limit);
    let Some((limit, served)) = overflow else {
        let deployments = deployments
            .into_iter()
            .enumerate()
            .filter(|(i, view)| *i == 0 || fits(view))
            .map(|(_, view)| view)
            .collect();
        return Ok(PlannedChain {
            deployments,
            descriptor: None,
        });
    };
    let fallbacks: Vec<GatewayRoute> = context_fallbacks
        .into_iter()
        .filter(|view| {
            context_window(registry, view, model).is_some_and(|(window, _)| estimate <= window)
        })
        .collect();
    let (Some(from), Some(to)) = (deployments.first(), fallbacks.first()) else {
        return Err(ContextWindowExceeded {
            estimate,
            limit,
            model: served,
        });
    };
    let from = from.provider.as_str().to_owned();
    let to = to.provider.as_str().to_owned();
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

fn context_window(
    registry: &ProviderRegistry,
    view: &GatewayRoute,
    requested: &str,
) -> Option<(u32, String)> {
    let provider = view.resolve(registry)?;
    let model = view.upstream_model.as_deref().unwrap_or(requested);
    let served = provider.find_served_model(model)?;
    let limit = served.limits.context_window;
    (limit > 0).then(|| (limit, model.to_owned()))
}
