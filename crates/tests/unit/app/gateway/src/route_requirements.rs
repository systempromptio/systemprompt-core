//! `enforce_route_requirements` — the gate between a route's declared
//! guarantees and the provider it resolves to.
//!
//! A route can require `european` (data residency) and `no_retain`. These are
//! compliance promises, so the consequential failure is silent: a route
//! declaring European hosting that dispatches to a provider without it sends
//! data somewhere it was promised not to go, and nothing errors.
//!
//! Governance resolves per model first and falls back to the provider, so both
//! directions of that fallback are asserted — a model can be stricter than its
//! provider, and it can be laxer.

use systemprompt_catalog_fixtures::{CatalogFixture, claude_model, platform};
use systemprompt_gateway::service::route_match::enforce_route_requirements;
use systemprompt_identifiers::{AiRequestId, RouteId};
use systemprompt_manifest::services::ai::ModelGovernance;
use systemprompt_manifest::services::catalog::{
    EndpointConfig, ModelEntry, ProviderConfig, ResolvedHop,
};
use systemprompt_manifest::services::{GatewayRoute, RouteRequirements};
use systemprompt_wire::WireProtocol;

fn route(requires: Option<RouteRequirements>) -> GatewayRoute {
    GatewayRoute {
        requires,
        ..GatewayRoute::new("eu-route")
    }
}

fn hop(provider: ModelGovernance, model: Option<ModelGovernance>) -> ResolvedHop {
    let catalog = CatalogFixture::new()
        .credential("acme", platform("acme_api_key"))
        .provider(
            "acme",
            ProviderConfig {
                governance: provider,
                ..ProviderConfig::new("acme")
            },
        )
        .endpoint(
            "acme",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://acme.invalid"),
        )
        .model(
            "model-a",
            ModelEntry {
                governance: model,
                ..claude_model(0)
            },
        )
        .deploy("model-a", ["acme/api"])
        .build();
    catalog.deployments("model-a")[0].clone()
}

const EUROPEAN: ModelGovernance = ModelGovernance {
    european: true,
    no_retain: false,
};
const NOTHING: ModelGovernance = ModelGovernance {
    european: false,
    no_retain: false,
};

fn enforce(route: &GatewayRoute, hop: &ResolvedHop, _model: &str) -> Result<(), String> {
    enforce_route_requirements(
        Some(route),
        &RouteId::new("eu-route"),
        hop,
        &AiRequestId::generate(),
    )
    .map_err(|e| format!("{e:?}"))
}

// Why: a route with no declared requirements constrains nothing. If this
// denied, every ordinary route would stop dispatching.
#[test]
fn a_route_declaring_nothing_is_always_allowed() {
    let provider = hop(NOTHING, None);

    enforce(&route(None), &provider, "model-a").expect("an unconstrained route must dispatch");
}

#[test]
fn a_satisfied_requirement_allows_the_dispatch() {
    let provider = hop(EUROPEAN, None);

    enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: false,
        })),
        &provider,
        "model-a",
    )
    .expect("a provider that satisfies the requirement must dispatch");
}

// Why: this is the gate. Dispatching here sends data to a provider the route
// promised it would not, and the failure is otherwise invisible — the request
// succeeds and the guarantee is simply untrue.
#[test]
fn an_unmet_requirement_denies_and_names_what_was_missing() {
    let provider = hop(NOTHING, None);

    let err = enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: false,
        })),
        &provider,
        "model-a",
    )
    .expect_err("a provider without European hosting must not serve a European route");

    assert!(
        err.contains("european"),
        "the denial should name the unmet requirement: {err}"
    );
    assert!(
        err.contains("eu-route"),
        "the denial should name the route: {err}"
    );
}

#[test]
fn every_unmet_requirement_is_reported_not_just_the_first() {
    let provider = hop(NOTHING, None);

    let err = enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: true,
        })),
        &provider,
        "model-a",
    )
    .expect_err("neither requirement is satisfied");

    assert!(err.contains("european"), "{err}");
    assert!(
        err.contains("no_retain"),
        "an operator fixing one requirement must see the other too: {err}"
    );
}

// Why: a model carries its own governance when it declares one. A provider
// that is broadly European may host a model that is not, and the route must be
// judged on the model actually being called.
#[test]
fn a_model_that_is_laxer_than_its_provider_is_denied() {
    let provider = hop(EUROPEAN, Some(NOTHING));

    let err = enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: false,
        })),
        &provider,
        "model-a",
    )
    .expect_err("the model's own governance must override the provider's");

    assert!(err.contains("european"), "{err}");
}

// Why: the other direction of the same fallback. A model may be stricter than
// the provider hosting it, and the route must be allowed on the model's terms
// rather than refused on the provider's.
#[test]
fn a_model_that_is_stricter_than_its_provider_is_allowed() {
    let provider = hop(NOTHING, Some(EUROPEAN));

    enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: false,
        })),
        &provider,
        "model-a",
    )
    .expect("a model declaring European hosting satisfies the route");
}

// Why: a model with no governance of its own inherits the provider's. Without
// the fallback an undeclared model would be treated as satisfying nothing.
#[test]
fn a_model_without_governance_inherits_the_providers() {
    let provider = hop(EUROPEAN, None);

    enforce(
        &route(Some(RouteRequirements {
            european: true,
            no_retain: false,
        })),
        &provider,
        "model-a",
    )
    .expect("an undeclared model inherits its provider's governance");
}
