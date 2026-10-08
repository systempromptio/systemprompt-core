use std::collections::BTreeMap;

use systemprompt_identifiers::{ProviderId, ScopeDimension};
use systemprompt_manifest::services::{
    ChainSelection, GatewayProfileError, GatewayRoute, ModelGovernance, ProviderRegistry,
    RouteDeployment, RouteScopeChains, ScopeChain, UnmappedScope,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_wire::WireProtocol;

use crate::profile_gateway::{
    enabled_gateway, priced_model, priced_provider, requires_no_retain, route_to, token_rates,
};

fn registry() -> ProviderRegistry {
    let claude = |name: &str, input: f64| {
        priced_provider(
            name,
            WireProtocol::Anthropic,
            vec![priced_model(
                "claude-opus-5",
                token_rates(input, input * 5.0),
            )],
        )
    };
    ProviderRegistry {
        providers: vec![
            claude("anthropic", 5.0),
            claude("vertex-eu-w4", 6.0),
            claude("vertex-eu-w1", 6.0),
            claude("vertex-acme-eu-w4", 6.0),
            claude("vertex-acme-eu-w1", 6.0),
            priced_provider(
                "gemini",
                WireProtocol::Gemini,
                vec![priced_model("gemini-3.5-flash", token_rates(0.3, 2.5))],
            ),
        ],
    }
}

fn deployment(provider: &str) -> RouteDeployment {
    RouteDeployment {
        provider: ProviderId::new(provider),
        upstream_model: None,
        weight: None,
    }
}

fn chained(primary: &str, fallbacks: &[&str]) -> GatewayRoute {
    let mut r = route_to("claude-*", primary);
    r.fallbacks = fallbacks.iter().map(|p| deployment(p)).collect();
    r
}

fn scoped(unmapped: UnmappedScope) -> GatewayRoute {
    let mut r = chained("vertex-eu-w4", &["vertex-eu-w1"]);
    let mut chains = BTreeMap::new();
    chains.insert(
        "acme".to_owned(),
        ScopeChain {
            provider: ProviderId::new("vertex-acme-eu-w4"),
            upstream_model: None,
            fallbacks: vec![deployment("vertex-acme-eu-w1")],
            strategy: systemprompt_manifest::services::SelectionStrategy::Ordered,
            weight: None,
            context_fallbacks: Vec::new(),
        },
    );
    r.by_scope = Some(RouteScopeChains {
        dimension: ScopeDimension::new("project"),
        chains,
        unmapped,
    });
    r
}

fn attributed(dimension: &str, value: &str) -> RequestAttribution {
    RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::new(dimension),
            value: value.to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: None,
    }
}

fn providers(views: &[GatewayRoute]) -> Vec<&str> {
    views.iter().map(|v| v.provider.as_str()).collect()
}

#[test]
fn a_route_naming_fallback_provider_is_refused_and_the_error_names_the_key() {
    let yaml = "model_pattern: claude-*\nprovider: anthropic\nfallback_provider: vertex\n";
    let err = serde_yaml::from_str::<GatewayRoute>(yaml).expect_err("removed key is refused");
    assert!(err.to_string().contains("fallback_provider"), "{err}");
    let yaml = "model_pattern: claude-*\nprovider: anthropic\nfallback_upstream_model: m\n";
    let err = serde_yaml::from_str::<GatewayRoute>(yaml).expect_err("removed key is refused");
    assert!(err.to_string().contains("fallback_upstream_model"), "{err}");
}

#[test]
fn deployment_views_order_primary_then_fallbacks_under_the_route_id() {
    let mut r = chained("vertex-eu-w4", &["vertex-eu-w1", "anthropic"]);
    r.fallbacks[1].upstream_model = Some("claude-opus-5@20260501".to_owned());
    r.pricing = Some(token_rates(1.0, 2.0));
    let views = r.deployment_views(&RequestAttribution::none());
    assert_eq!(
        providers(&views),
        ["vertex-eu-w4", "vertex-eu-w1", "anthropic"]
    );
    assert!(views.iter().all(|v| v.effective_id() == r.effective_id()));
    assert!(
        views
            .iter()
            .all(|v| v.fallbacks.is_empty() && v.by_scope.is_none())
    );
    assert!(
        views[0].pricing.is_some(),
        "the primary keeps the route override"
    );
    assert!(
        views[1].pricing.is_none(),
        "an override is not a fallback's rate"
    );
    assert_eq!(
        views[2].upstream_model.as_deref(),
        Some("claude-opus-5@20260501")
    );
}

#[test]
fn a_mapped_scope_value_takes_its_own_chain() {
    let r = scoped(UnmappedScope::Deny);
    let attribution = attributed("project", "acme");
    let selection = r.chain_for(&attribution);
    assert!(matches!(
        selection,
        ChainSelection::Scope { value: "acme", .. }
    ));
    assert_eq!(
        selection.descriptor().as_deref(),
        Some("scope:project=acme")
    );
    assert_eq!(
        providers(&r.chain_views(&selection)),
        ["vertex-acme-eu-w4", "vertex-acme-eu-w1"]
    );
}

#[test]
fn no_value_for_the_dimension_takes_the_route_chain() {
    let r = scoped(UnmappedScope::Deny);
    for attribution in [
        RequestAttribution::none(),
        attributed("cost_centre", "acme"),
    ] {
        assert_eq!(r.chain_for(&attribution), ChainSelection::Route);
        assert_eq!(
            providers(&r.deployment_views(&attribution)),
            ["vertex-eu-w4", "vertex-eu-w1"]
        );
    }
}

#[test]
fn an_unmapped_scope_value_is_denied_by_default_and_shared_on_opt_in() {
    let attribution = attributed("project", "zeta");
    let denied = scoped(UnmappedScope::Deny);
    assert!(matches!(
        denied.chain_for(&attribution),
        ChainSelection::Unmapped { value: "zeta", .. }
    ));
    assert!(denied.deployment_views(&attribution).is_empty());
    let shared = scoped(UnmappedScope::Shared);
    assert_eq!(shared.chain_for(&attribution), ChainSelection::Route);
    assert_eq!(
        providers(&shared.deployment_views(&attribution)),
        ["vertex-eu-w4", "vertex-eu-w1"]
    );
}

#[test]
fn validate_accepts_chains_declared_in_the_registry() {
    let gw = enabled_gateway(vec![scoped(UnmappedScope::Deny)]);
    gw.validate(&registry())
        .expect("every deployment is declared and priced");
}

#[test]
fn validate_rejects_a_duplicate_deployment_in_a_chain() {
    let gw = enabled_gateway(vec![chained(
        "vertex-eu-w4",
        &["vertex-eu-w1", "vertex-eu-w4"],
    )]);
    match gw.validate(&registry()) {
        Err(GatewayProfileError::RouteDeploymentDuplicate {
            chain, provider, ..
        }) => {
            assert_eq!(chain, "route");
            assert_eq!(provider, "vertex-eu-w4");
        },
        other => panic!("expected RouteDeploymentDuplicate, got {other:?}"),
    }
}

#[test]
fn validate_rejects_an_unknown_deployment_provider() {
    let gw = enabled_gateway(vec![chained("vertex-eu-w4", &["ghost"])]);
    match gw.validate(&registry()) {
        Err(GatewayProfileError::RouteDeploymentProviderNotInRegistry { provider, .. }) => {
            assert_eq!(provider, "ghost");
        },
        other => panic!("expected RouteDeploymentProviderNotInRegistry, got {other:?}"),
    }
}

#[test]
fn validate_rejects_an_unknown_scope_chain_provider() {
    let mut r = scoped(UnmappedScope::Deny);
    if let Some(scoped) = r.by_scope.as_mut() {
        scoped.chains.get_mut("acme").unwrap().fallbacks = vec![deployment("ghost")];
    }
    match enabled_gateway(vec![r]).validate(&registry()) {
        Err(GatewayProfileError::RouteScopeChainProviderNotInRegistry {
            scope, provider, ..
        }) => {
            assert_eq!(scope, "acme");
            assert_eq!(provider, "ghost");
        },
        other => panic!("expected RouteScopeChainProviderNotInRegistry, got {other:?}"),
    }
}

#[test]
fn validate_prices_every_deployment() {
    let gw = enabled_gateway(vec![chained("vertex-eu-w4", &["gemini"])]);
    match gw.validate(&registry()) {
        Err(GatewayProfileError::RouteReachesNoPricedModel { provider, .. }) => {
            assert_eq!(provider, "gemini");
        },
        other => panic!("expected RouteReachesNoPricedModel for the fallback, got {other:?}"),
    }
}

#[test]
fn validate_governs_every_scope_deployment() {
    let mut registry = registry();
    for entry in &mut registry.providers {
        entry.governance = ModelGovernance {
            european: false,
            no_retain: entry.name.as_str() != "vertex-acme-eu-w1",
        };
    }
    let mut r = scoped(UnmappedScope::Deny);
    r.requires = Some(requires_no_retain());
    match enabled_gateway(vec![r]).validate(&registry) {
        Err(GatewayProfileError::RouteGovernanceUnsatisfied { requirements, .. }) => {
            assert_eq!(requirements, "no_retain");
        },
        other => panic!("expected RouteGovernanceUnsatisfied, got {other:?}"),
    }
}

#[test]
fn validate_rejects_by_scope_without_chains_or_with_an_empty_value() {
    let mut empty = scoped(UnmappedScope::Deny);
    empty.by_scope.as_mut().unwrap().chains.clear();
    assert!(matches!(
        enabled_gateway(vec![empty]).validate(&registry()),
        Err(GatewayProfileError::RouteScopeChainsEmpty { .. })
    ));
    let mut blank = scoped(UnmappedScope::Deny);
    let chains = &mut blank.by_scope.as_mut().unwrap().chains;
    let acme = chains.remove("acme").unwrap();
    chains.insert(" ".to_owned(), acme);
    assert!(matches!(
        enabled_gateway(vec![blank]).validate(&registry()),
        Err(GatewayProfileError::RouteScopeKeyEmpty { .. })
    ));
}

#[test]
fn yaml_route_round_trips_the_full_chain_shape() {
    let yaml = "\
model_pattern: claude-*
provider: vertex-eu-w4
fallbacks:
  - provider: vertex-eu-w1
  - provider: anthropic
    upstream_model: claude-opus-5@20260501
by_scope:
  dimension: project
  chains:
    acme:
      provider: vertex-acme-eu-w4
      fallbacks:
        - provider: vertex-acme-eu-w1
  unmapped: shared
";
    let r: GatewayRoute = serde_yaml::from_str(yaml).expect("route parses");
    assert_eq!(r.fallbacks.len(), 2);
    let scoped = r.by_scope.as_ref().expect("by_scope parses");
    assert_eq!(scoped.dimension.as_str(), "project");
    assert_eq!(scoped.unmapped, UnmappedScope::Shared);
    let back: GatewayRoute =
        serde_yaml::from_str(&serde_yaml::to_string(&r).expect("serialize")).expect("reparse");
    assert_eq!(back.fallbacks, r.fallbacks);
    assert_eq!(back.by_scope, r.by_scope);
    let plain = serde_yaml::to_string(&route_to("claude-*", "anthropic")).expect("serialize");
    assert!(
        !plain.contains("fallbacks") && !plain.contains("by_scope"),
        "{plain}"
    );
}

#[test]
fn by_scope_defaults_to_deny_and_rejects_an_invalid_dimension() {
    let yaml = "model_pattern: m\nprovider: p\nby_scope:\n  dimension: project\n  chains:\n    a: {provider: q}\n";
    let r: GatewayRoute = serde_yaml::from_str(yaml).expect("route parses");
    assert_eq!(r.by_scope.unwrap().unmapped, UnmappedScope::Deny);
    let bad = "model_pattern: m\nprovider: p\nby_scope:\n  dimension: \"\"\n  chains:\n    a: {provider: q}\n";
    assert!(serde_yaml::from_str::<GatewayRoute>(bad).is_err());
}
