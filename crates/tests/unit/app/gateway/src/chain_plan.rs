//! Chain planning before dispatch: the selection-strategy attempt order and
//! the context-window pre-check with its fallbacks.

use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_gateway::service::chain_plan::{fit_context_window, order_chain};
use systemprompt_gateway::service::failover::{DeploymentState, plan_selection};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::{GatewayRoute, ProviderEntry, ProviderRegistry, SelectionStrategy};

const fn state(tripped: bool, weight: u32, in_flight: u64) -> DeploymentState {
    DeploymentState {
        tripped,
        weight,
        in_flight,
    }
}

#[test]
fn ordered_keeps_chain_order_and_skips_tripped() {
    let states = [state(false, 1, 0), state(true, 1, 0), state(false, 1, 0)];
    assert_eq!(plan_selection(SelectionStrategy::Ordered, &states, 7), vec![0, 2]);
}

#[test]
fn weighted_draw_lands_by_cumulative_weight() {
    let states = [state(false, 1, 0), state(false, 3, 0)];
    assert_eq!(plan_selection(SelectionStrategy::Weighted, &states, 0), vec![0, 1]);
    for draw in 1..4 {
        assert_eq!(plan_selection(SelectionStrategy::Weighted, &states, draw), vec![1, 0]);
    }
    assert_eq!(plan_selection(SelectionStrategy::Weighted, &states, 4), vec![0, 1]);
}

#[test]
fn weighted_never_draws_a_tripped_deployment_and_tries_it_last() {
    let states = [state(true, 100, 0), state(false, 1, 0), state(false, 2, 0)];
    for draw in 0..10 {
        let order = plan_selection(SelectionStrategy::Weighted, &states, draw);
        assert_ne!(order[0], 0, "draw {draw}");
        assert_eq!(order.last(), Some(&0));
        assert_eq!(order.len(), 3);
    }
}

#[test]
fn weighted_with_every_deployment_tripped_still_sends() {
    let states = [state(true, 1, 0), state(true, 1, 0)];
    assert_eq!(plan_selection(SelectionStrategy::Weighted, &states, 3), vec![0, 1]);
}

#[test]
fn least_busy_leads_with_the_fewest_in_flight_and_ties_go_to_chain_order() {
    let states = [state(false, 1, 4), state(false, 1, 1), state(false, 1, 1)];
    assert_eq!(plan_selection(SelectionStrategy::LeastBusy, &states, 0), vec![1, 0, 2]);
    let tripped_idle = [state(false, 1, 9), state(true, 1, 0)];
    assert_eq!(plan_selection(SelectionStrategy::LeastBusy, &tripped_idle, 0), vec![0, 1]);
}

fn route(yaml: &str) -> GatewayRoute {
    serde_yaml::from_str(yaml).expect("route fixture")
}

#[test]
fn route_strategy_and_weights_reach_every_view() {
    let r = route(
        "model_pattern: m\nprovider: a\nweight: 3\nstrategy: weighted\nfallbacks:\n  - {provider: b, weight: 2}\n  - {provider: c}\n",
    );
    let views = r.chain_views(&systemprompt_manifest::services::ChainSelection::Route);
    let weights: Vec<u32> = views.iter().map(GatewayRoute::effective_weight).collect();
    assert_eq!(weights, vec![3, 2, 1]);
    assert!(views.iter().all(|v| v.strategy == SelectionStrategy::Weighted));
}

#[test]
fn ordered_chain_is_unchanged_and_not_described() {
    let r = route("model_pattern: m\nprovider: a\nfallbacks:\n  - {provider: b}\n");
    let planned = order_chain(
        r.chain_views(&systemprompt_manifest::services::ChainSelection::Route),
        "m",
    );
    let providers: Vec<&str> = planned.deployments.iter().map(|v| v.provider.as_str()).collect();
    assert_eq!(providers, vec!["a", "b"]);
    assert_eq!(planned.descriptor, None);
}

#[test]
fn a_bare_route_without_new_keys_serializes_without_them() {
    let r = route("model_pattern: m\nprovider: a\n");
    let yaml = serde_yaml::to_string(&r).expect("serialize");
    assert!(!yaml.contains("strategy") && !yaml.contains("weight") && !yaml.contains("context"));
}

fn provider(name: &str, context_window: u32) -> ProviderEntry {
    serde_yaml::from_str(&format!(
        "name: {name}\nwire: anthropic\nsurface: anthropic\nendpoint: https://example.test/v1\napi_key_secret: {name}\nmodels:\n  - id: {name}-m\n    upstream_model: m\n    pricing:\n      input_per_million: 1.0\n      output_per_million: 1.0\n    limits:\n      context_window: {context_window}\n      max_output_tokens: 1000\n"
    ))
    .expect("provider fixture")
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![provider("small", 100), provider("large", 100_000)],
    }
}

fn request_of(chars: usize) -> CanonicalRequest {
    CanonicalRequest {
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("x".repeat(chars))],
        }],
        ..CanonicalRequest::new(ModelId::new("m"), Vec::new(), 64)
    }
}

fn chain(r: &GatewayRoute) -> (Vec<GatewayRoute>, Vec<GatewayRoute>) {
    let selection = systemprompt_manifest::services::ChainSelection::Route;
    (r.chain_views(&selection), r.context_fallback_views(&selection))
}

#[test]
fn a_request_that_fits_keeps_its_chain() {
    let r = route("model_pattern: m\nprovider: small\ncontext_fallbacks:\n  - {provider: large}\n");
    let (deployments, fallbacks) = chain(&r);
    let planned = fit_context_window(&registry(), &request_of(40), deployments, fallbacks)
        .expect("fits");
    assert_eq!(planned.deployments[0].provider.as_str(), "small");
    assert_eq!(planned.descriptor, None);
}

#[test]
fn an_oversized_request_moves_to_the_context_fallback() {
    let r = route("model_pattern: m\nprovider: small\ncontext_fallbacks:\n  - {provider: large}\n");
    let (deployments, fallbacks) = chain(&r);
    let planned = fit_context_window(&registry(), &request_of(4_000), deployments, fallbacks)
        .expect("falls back");
    assert_eq!(planned.deployments.len(), 1);
    assert_eq!(planned.deployments[0].provider.as_str(), "large");
    assert_eq!(planned.descriptor.as_deref(), Some("context_window:small->large"));
}

#[test]
fn an_oversized_request_with_no_fitting_fallback_is_refused_with_the_numbers() {
    let r = route("model_pattern: m\nprovider: small\n");
    let (deployments, fallbacks) = chain(&r);
    let exceeded = fit_context_window(&registry(), &request_of(4_000), deployments, fallbacks)
        .expect_err("refused");
    assert_eq!(exceeded.limit, 100);
    assert!(exceeded.estimate > 100);
    assert!(exceeded.to_string().starts_with("context_window_exceeded"));
}

#[test]
fn failover_deployments_too_small_for_the_request_are_dropped() {
    let r = route("model_pattern: m\nprovider: large\nfallbacks:\n  - {provider: small}\n");
    let (deployments, fallbacks) = chain(&r);
    let planned = fit_context_window(&registry(), &request_of(4_000), deployments, fallbacks)
        .expect("fits the primary");
    let providers: Vec<&str> = planned.deployments.iter().map(|v| v.provider.as_str()).collect();
    assert_eq!(providers, vec!["large"]);
}
