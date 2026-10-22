//! Policy routes over `ResolvedCatalog::resolve`: scope routes pin, alias
//! routes rename, the first matching route wins, routes only narrow.

use systemprompt_catalog_fixtures::{claude_model, one_platform_provider, vertex_regions};
use systemprompt_identifiers::{ModelId, ScopeDimension};
use systemprompt_manifest::services::catalog::{EndpointRef, ModelRequest};
use systemprompt_manifest::services::{
    GatewayRoute, ModelGlobs, RouteAlias, RouteDeployment, RouteMatch, SelectionStrategy,
    WeightedDeployment,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};

fn project(value: &str) -> RequestAttribution {
    RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::try_new("project").expect("dimension"),
            value: value.to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: None,
    }
}

#[test]
fn a_scope_route_pins_only_the_requests_it_matches() {
    let catalog = vertex_regions().build();
    let commerce = project("commerce");
    let pinned = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5").with_attribution(&commerce))
        .expect("resolves");
    assert_eq!(
        pinned.route.as_ref().map(|r| r.as_str()),
        Some("project-commerce-eu-only")
    );
    assert_eq!(pinned.access_route_id.as_str(), "project-commerce-eu-only");
    let endpoints: Vec<&str> = pinned.chain.iter().map(|h| h.endpoint.as_str()).collect();
    assert_eq!(endpoints, vec!["claude-eu"]);

    let core = project("core");
    let shared = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5").with_attribution(&core))
        .expect("resolves");
    assert!(
        shared.route.is_none(),
        "an unmatched scope falls through to the model's chain"
    );
    assert_eq!(shared.chain.len(), 2);
    let plain = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert_eq!(plain.chain.len(), 2, "no attribution, no scope route");
}

#[test]
fn an_alias_route_serves_a_second_requested_id_as_the_model() {
    let alias = GatewayRoute {
        alias: Some(RouteAlias {
            requested: ModelId::new("claude-sonnet-latest"),
            model: ModelId::new("claude-sonnet-5"),
        }),
        ..GatewayRoute::new("sonnet-latest")
    };
    let catalog = one_platform_provider().route(alias).build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-latest[1m]"))
        .expect("alias resolves");
    assert_eq!(target.model.id.as_str(), "claude-sonnet-5");
    assert_eq!(target.chain[0].upstream_model, "claude-sonnet-5");
    assert_eq!(
        target.route.as_ref().map(|r| r.as_str()),
        Some("sonnet-latest")
    );
    assert!(
        catalog.model("claude-sonnet-latest").is_none(),
        "an alias is not a second model"
    );
}

#[test]
fn the_first_matching_route_wins_and_a_glob_never_selects_an_endpoint() {
    let first = GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        label: Some("Claude".to_owned()),
        ..GatewayRoute::new("claude-label")
    };
    let second = GatewayRoute {
        models: ModelGlobs(vec!["claude-sonnet-*".to_owned()]),
        deployments: vec![RouteDeployment::Endpoint(EndpointRef::new(
            "vertex",
            "claude-eu",
        ))],
        ..GatewayRoute::new("sonnet-eu")
    };
    let catalog = vertex_regions().route(first).route(second).build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert_eq!(
        target.route.as_ref().map(|r| r.as_str()),
        Some("claude-label")
    );
    assert_eq!(
        target.chain.len(),
        2,
        "a route without deployments keeps the model's chain"
    );
    let route = catalog.route("claude-label").expect("declared");
    assert_eq!(route.audit_label(), "Claude");
}

#[test]
fn a_weighted_route_carries_weights_and_strategy_onto_the_chain() {
    let weighted = GatewayRoute {
        models: ModelGlobs(vec!["claude-sonnet-5".to_owned()]),
        strategy: SelectionStrategy::Weighted,
        deployments: vec![
            RouteDeployment::Weighted(WeightedDeployment {
                endpoint: EndpointRef::new("vertex", "claude-eu"),
                weight: Some(3),
            }),
            RouteDeployment::Endpoint(EndpointRef::new("vertex", "claude-global")),
        ],
        ..GatewayRoute::new("weighted")
    };
    let catalog = vertex_regions().route(weighted).build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert_eq!(target.strategy, SelectionStrategy::Weighted);
    assert_eq!(target.chain[0].endpoint.as_str(), "claude-eu");
    assert_eq!(target.chain[0].effective_weight(), 3);
    assert_eq!(target.chain[1].effective_weight(), 1);
}

#[test]
fn a_route_with_request_predicates_is_skipped_for_a_bare_lookup() {
    let thinking = GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        when: Some(RouteMatch {
            thinking: Some(true),
            ..RouteMatch::default()
        }),
        deployments: vec![RouteDeployment::Endpoint(EndpointRef::new(
            "vertex",
            "claude-eu",
        ))],
        ..GatewayRoute::new("thinking-eu")
    };
    let catalog = vertex_regions().route(thinking).build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert!(target.route.is_none());
}

#[test]
fn context_fallbacks_follow_the_route_to_another_model() {
    let small = claude_model(200_000);
    let fallback = GatewayRoute {
        models: ModelGlobs(vec!["small-sonnet".to_owned()]),
        context_fallbacks: vec![ModelId::new("claude-sonnet-5")],
        ..GatewayRoute::new("small-overflow")
    };
    let catalog = vertex_regions()
        .model("small-sonnet", small)
        .deploy("small-sonnet", ["vertex/claude-global"])
        .route(fallback)
        .build();
    let target = catalog
        .resolve(&ModelRequest::new("small-sonnet"))
        .expect("resolves");
    assert_eq!(target.context_fallbacks.len(), 1);
    assert_eq!(
        target.context_fallbacks[0].model.as_str(),
        "claude-sonnet-5"
    );
}
