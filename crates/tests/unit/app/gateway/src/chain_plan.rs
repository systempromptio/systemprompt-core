//! Chain planning before dispatch: the selection-strategy attempt order and
//! the context-window pre-check with its fallbacks.

use systemprompt_catalog_fixtures::{CatalogFixture, model, platform, priced};
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_gateway::service::chain_plan::{fit_context_window, order_chain};
use systemprompt_gateway::service::failover::{
    DeploymentLoad, DeploymentState, ProviderBreakers, plan_selection,
};
use systemprompt_identifiers::{ModelId, RouteId};
use systemprompt_manifest::services::catalog::{
    EndpointConfig, ProviderConfig, ResolvedCatalog, ResolvedHop,
};
use systemprompt_manifest::services::{GatewayRoute, SelectionStrategy};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

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
    assert_eq!(
        plan_selection(SelectionStrategy::Ordered, &states, 7),
        vec![0, 2]
    );
}

#[test]
fn weighted_draw_lands_by_cumulative_weight() {
    let states = [state(false, 1, 0), state(false, 3, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 0),
        vec![0, 1]
    );
    for draw in 1..4 {
        assert_eq!(
            plan_selection(SelectionStrategy::Weighted, &states, draw),
            vec![1, 0]
        );
    }
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 4),
        vec![0, 1]
    );
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
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 3),
        vec![0, 1]
    );
}

#[test]
fn least_busy_leads_with_the_fewest_in_flight_and_ties_go_to_chain_order() {
    let states = [state(false, 1, 4), state(false, 1, 1), state(false, 1, 1)];
    assert_eq!(
        plan_selection(SelectionStrategy::LeastBusy, &states, 0),
        vec![1, 0, 2]
    );
    let tripped_idle = [state(false, 1, 9), state(true, 1, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::LeastBusy, &tripped_idle, 0),
        vec![0, 1]
    );
}

fn catalog() -> ResolvedCatalog {
    let mut fixture = CatalogFixture::new();
    for name in [
        "small",
        "large",
        "lb-plan-a",
        "lb-plan-b",
        "lb-plan-c",
        "w-plan-a",
        "w-plan-b",
        "w-plan-solo",
        "unregistered",
    ] {
        fixture = fixture
            .credential(name, platform(name))
            .provider(name, ProviderConfig::new(name))
            .endpoint(
                name,
                "api",
                EndpointConfig::new(WireProtocol::Anthropic, "https://example.test/v1"),
            );
    }
    fixture
        .model(
            "small-m",
            model(&[ClientFamily::Claude], 100, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "large-m",
            model(&[ClientFamily::Claude], 100_000, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "open-m",
            model(&[ClientFamily::Claude], 0, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "plan-m",
            model(&[ClientFamily::Claude], 100_000, priced(1.0, 1.0, 0.0)),
        )
        .deploy("small-m", ["small/api"])
        .deploy("large-m", ["large/api"])
        .deploy("open-m", ["unregistered/api"])
        .deploy(
            "plan-m",
            [
                "lb-plan-a/api",
                "lb-plan-b/api",
                "lb-plan-c/api",
                "w-plan-a/api",
                "w-plan-b/api",
                "w-plan-solo/api",
            ],
        )
        .build()
}

fn hop(catalog: &ResolvedCatalog, model: &str, provider: &str, weight: Option<u32>) -> ResolvedHop {
    let hop = catalog
        .deployments(model)
        .iter()
        .find(|h| h.provider.as_str() == provider)
        .unwrap_or_else(|| panic!("{model} on {provider}"));
    ResolvedHop {
        weight,
        ..hop.clone()
    }
}

fn route_id() -> RouteId {
    RouteId::new("r")
}

#[test]
fn hop_weights_default_to_one() {
    let c = catalog();
    let weights: Vec<u32> = [
        hop(&c, "plan-m", "w-plan-a", Some(3)),
        hop(&c, "plan-m", "w-plan-b", Some(2)),
        hop(&c, "plan-m", "lb-plan-a", None),
    ]
    .iter()
    .map(ResolvedHop::effective_weight)
    .collect();
    assert_eq!(weights, vec![3, 2, 1]);
}

#[test]
fn ordered_chain_is_unchanged_and_not_described() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "small-m", "small", None),
            hop(&c, "large-m", "large", None),
        ],
        SelectionStrategy::Ordered,
        Some(&route_id()),
        &ProviderBreakers::new(),
        &DeploymentLoad::default(),
    );
    assert_eq!(providers_of(&planned), vec!["small", "large"]);
    assert_eq!(planned.descriptor, None);
}

#[test]
fn a_bare_route_serializes_without_strategy_or_fallbacks() {
    let r = GatewayRoute::new("r");
    let yaml = serde_yaml::to_string(&r).expect("serialize");
    assert!(!yaml.contains("strategy") && !yaml.contains("weight") && !yaml.contains("context"));
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

#[test]
fn a_request_that_fits_keeps_its_chain() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(40),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("fits");
    assert_eq!(planned.deployments[0].provider.as_str(), "small");
    assert_eq!(planned.descriptor, None);
}

#[test]
fn an_oversized_request_moves_to_the_context_fallback() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("falls back");
    assert_eq!(planned.deployments.len(), 1);
    assert_eq!(planned.deployments[0].provider.as_str(), "large");
    assert_eq!(
        planned.descriptor.as_deref(),
        Some("context_window:small/api->large/api")
    );
}

#[test]
fn an_oversized_request_with_no_fitting_fallback_is_refused_with_the_numbers() {
    let c = catalog();
    let exceeded = fit_context_window(
        &c,
        &request_of(4_000),
        vec![hop(&c, "small-m", "small", None)],
        Vec::new(),
    )
    .expect_err("refused");
    assert_eq!(exceeded.limit, 100);
    assert!(exceeded.estimate > 100);
    assert!(exceeded.to_string().starts_with("context_window_exceeded"));
}

#[test]
fn failover_deployments_too_small_for_the_request_are_dropped() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        vec![
            hop(&c, "large-m", "large", None),
            hop(&c, "small-m", "small", None),
        ],
        Vec::new(),
    )
    .expect("fits the primary");
    assert_eq!(providers_of(&planned), vec!["large"]);
}

fn providers_of(planned: &systemprompt_gateway::service::chain_plan::PlannedChain) -> Vec<&str> {
    planned
        .deployments
        .iter()
        .map(|v| v.provider.as_str())
        .collect()
}

#[test]
fn an_empty_chain_plans_to_nothing() {
    let planned = order_chain(
        Vec::new(),
        SelectionStrategy::Ordered,
        Some(&route_id()),
        &ProviderBreakers::new(),
        &DeploymentLoad::default(),
    );
    assert!(planned.deployments.is_empty());
    assert_eq!(planned.descriptor, None);
}

#[test]
fn an_idle_least_busy_chain_keeps_chain_order_and_is_described() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "plan-m", "lb-plan-a", None),
            hop(&c, "plan-m", "lb-plan-b", None),
            hop(&c, "plan-m", "lb-plan-c", None),
        ],
        SelectionStrategy::LeastBusy,
        Some(&route_id()),
        &ProviderBreakers::new(),
        &DeploymentLoad::default(),
    );
    assert_eq!(
        providers_of(&planned),
        vec!["lb-plan-a", "lb-plan-b", "lb-plan-c"]
    );
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:least_busy"));
}

#[test]
fn a_weighted_chain_is_a_permutation_of_its_deployments() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "plan-m", "w-plan-a", Some(5)),
            hop(&c, "plan-m", "w-plan-b", Some(1)),
        ],
        SelectionStrategy::Weighted,
        Some(&route_id()),
        &ProviderBreakers::new(),
        &DeploymentLoad::default(),
    );
    let mut providers = providers_of(&planned);
    providers.sort_unstable();
    assert_eq!(providers, vec!["w-plan-a", "w-plan-b"]);
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:weighted"));
}

#[test]
fn a_single_weighted_deployment_is_not_reordered_but_is_described() {
    let c = catalog();
    let planned = order_chain(
        vec![hop(&c, "plan-m", "w-plan-solo", None)],
        SelectionStrategy::Weighted,
        Some(&route_id()),
        &ProviderBreakers::new(),
        &DeploymentLoad::default(),
    );
    assert_eq!(providers_of(&planned), vec!["w-plan-solo"]);
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:weighted"));
}

#[test]
fn a_deployment_with_no_known_window_is_never_refused() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        vec![
            hop(&c, "open-m", "unregistered", None),
            hop(&c, "small-m", "small", None),
        ],
        Vec::new(),
    )
    .expect("an unknown window is not a refusal");
    assert_eq!(providers_of(&planned), vec!["unregistered"]);
}
