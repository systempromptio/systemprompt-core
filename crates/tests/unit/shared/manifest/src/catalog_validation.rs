//! What `ResolvedCatalog::build` refuses.

use systemprompt_catalog_fixtures::{
    CatalogFixture, claude_model, model, one_platform_provider, opencode_go, platform, priced,
    vertex_regions,
};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{
    BetaSpec, CatalogError, EndpointConfig, EndpointRef, ProviderConfig,
};
use systemprompt_manifest::services::{
    GatewayProfileError, GatewayRoute, ModelGlobs, RouteAlias, RouteDeployment, RouteRequirements,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

fn refused(fixture: &CatalogFixture) -> CatalogError {
    fixture
        .try_build()
        .expect_err("the catalog must be refused")
}

#[test]
fn every_canned_fixture_resolves() {
    for fixture in [one_platform_provider(), opencode_go(), vertex_regions()] {
        let _catalog = fixture.build();
    }
}

#[test]
fn a_model_without_a_deployment_is_an_error() {
    let fixture = one_platform_provider().model("orphan", claude_model(1000));
    assert!(matches!(
        refused(&fixture),
        CatalogError::ModelWithoutDeployment { .. }
    ));
}

#[test]
fn a_deployment_of_an_undeclared_model_or_endpoint_is_an_error() {
    let unknown_model = one_platform_provider().deploy("ghost", ["anthropic/api"]);
    assert!(matches!(
        refused(&unknown_model),
        CatalogError::DeploymentForUnknownModel { .. }
    ));
    let unknown_endpoint = one_platform_provider().deploy("claude-sonnet-5", ["anthropic/eu"]);
    assert!(matches!(
        refused(&unknown_endpoint),
        CatalogError::UnknownEndpoint { .. }
    ));
    let twice =
        one_platform_provider().deploy("claude-sonnet-5", ["anthropic/api", "anthropic/api"]);
    assert!(matches!(
        refused(&twice),
        CatalogError::DuplicateDeployment { .. }
    ));
}

#[test]
fn the_default_model_must_be_a_deployed_model_without_the_context_marker() {
    let unknown = one_platform_provider().default_model("claude-nope");
    assert!(matches!(
        refused(&unknown),
        CatalogError::UnknownDefaultModel { .. }
    ));
    let marked = one_platform_provider().default_model("claude-sonnet-5[1m]");
    assert!(matches!(
        refused(&marked),
        CatalogError::Gateway(GatewayProfileError::ContextVariantInConfig { .. })
    ));
    let disabled = one_platform_provider().with_provider("anthropic", |p| p.enabled = false);
    assert!(matches!(
        refused(&disabled),
        CatalogError::DefaultModelNotServable { .. }
    ));
}

#[test]
fn beta_inheritance_must_name_a_sibling_and_must_not_loop() {
    let unknown = vertex_regions().with_provider("vertex", |p| {
        if let Some(eu) = p.endpoints.get_mut("claude-eu") {
            eu.accepted_betas = Some(BetaSpec::Inherit("inherit:nowhere".to_owned()));
        }
    });
    assert!(matches!(
        refused(&unknown),
        CatalogError::UnknownBetaInherit { .. }
    ));
    let cyclic = vertex_regions().with_provider("vertex", |p| {
        if let Some(global) = p.endpoints.get_mut("claude-global") {
            global.accepted_betas = Some(BetaSpec::Inherit("inherit:claude-eu".to_owned()));
        }
    });
    assert!(matches!(
        refused(&cyclic),
        CatalogError::CyclicBetaInherit { .. }
    ));
    let malformed = vertex_regions().with_provider("vertex", |p| {
        if let Some(eu) = p.endpoints.get_mut("claude-eu") {
            eu.accepted_betas = Some(BetaSpec::Inherit("claude-global".to_owned()));
        }
    });
    assert!(matches!(
        refused(&malformed),
        CatalogError::MalformedBetaInherit { .. }
    ));
}

#[test]
fn endpoints_are_guarded() {
    let literal = vertex_regions().endpoint(
        "vertex",
        "literal",
        EndpointConfig::new(
            WireProtocol::Gemini,
            "https://aiplatform.googleapis.com/v1/projects/acme-prod/locations/global",
        ),
    );
    assert!(matches!(
        refused(&literal),
        CatalogError::LiteralProjectInUrl { .. }
    ));
    let blocked = one_platform_provider().endpoint(
        "anthropic",
        "local",
        EndpointConfig::new(WireProtocol::Anthropic, "http://169.254.169.254/latest"),
    );
    assert!(matches!(
        refused(&blocked),
        CatalogError::BlockedEndpointUrl { .. }
    ));
    let multiplier = vertex_regions().with_provider("vertex", |p| {
        if let Some(eu) = p.endpoints.get_mut("claude-eu") {
            eu.price_multiplier = Some(0.0);
        }
    });
    assert!(matches!(
        refused(&multiplier),
        CatalogError::InvalidPriceMultiplier { .. }
    ));
    let headers = one_platform_provider().with_provider("anthropic", |p| {
        p.extra_headers
            .insert("Authorization".to_owned(), "x".to_owned());
    });
    assert!(matches!(
        refused(&headers),
        CatalogError::ReservedHeader { .. }
    ));
    let empty = one_platform_provider().provider("bare", ProviderConfig::new("anthropic"));
    assert!(matches!(
        refused(&empty),
        CatalogError::ProviderWithoutEndpoints { .. }
    ));
}

#[test]
fn with_the_gateway_on_every_hop_is_priced_with_a_cache_rate() {
    let unpriced = one_platform_provider()
        .model(
            "free",
            model(&[ClientFamily::Claude], 1000, priced(0.0, 0.0, 0.0)),
        )
        .deploy("free", ["anthropic/api"]);
    assert!(matches!(
        refused(&unpriced),
        CatalogError::UnpricedDeployment { .. }
    ));
    let mut no_cache = priced(1.0, 2.0, 0.0);
    no_cache.cache_read_per_million = None;
    let uncached = one_platform_provider()
        .model("uncached", model(&[], 1000, no_cache))
        .deploy("uncached", ["anthropic/api"]);
    assert!(matches!(
        refused(&uncached),
        CatalogError::CacheRateUndeclared { .. }
    ));
    let _catalog = uncached.clone().gateway_enabled(false).build();
}

#[test]
fn routes_only_narrow_a_models_own_deployments() {
    let foreign = GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        deployments: vec![RouteDeployment::Endpoint(EndpointRef::new(
            "vertex", "gemini",
        ))],
        ..GatewayRoute::new("claude-on-gemini")
    };
    let fixture = vertex_regions().route(foreign);
    assert!(matches!(
        refused(&fixture),
        CatalogError::RouteDeploymentNotDeployed { .. }
    ));
    let nothing = GatewayRoute {
        models: ModelGlobs(vec!["mistral-*".to_owned()]),
        ..GatewayRoute::new("mistral")
    };
    assert!(matches!(
        refused(&vertex_regions().route(nothing)),
        CatalogError::RouteMatchesNoModel { .. }
    ));
}

#[test]
fn route_ids_are_declared_and_unique() {
    let a = GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        ..GatewayRoute::new("same")
    };
    let fixture = vertex_regions().route(a.clone()).route(a);
    assert!(matches!(
        refused(&fixture),
        CatalogError::Gateway(GatewayProfileError::DuplicateRouteId { .. })
    ));
    let shapeless = vertex_regions().route(GatewayRoute::new("shapeless"));
    assert!(matches!(
        refused(&shapeless),
        CatalogError::Gateway(GatewayProfileError::RouteMatchesNothing { .. })
    ));
    let empty = vertex_regions().route(GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        ..GatewayRoute::new(" ")
    });
    assert!(matches!(
        refused(&empty),
        CatalogError::Gateway(GatewayProfileError::EmptyRouteId)
    ));
}

#[test]
fn an_alias_must_not_shadow_a_model_and_must_target_one() {
    let shadow = GatewayRoute {
        alias: Some(RouteAlias {
            requested: ModelId::new("claude-sonnet-5"),
            model: ModelId::new("claude-internal"),
        }),
        ..GatewayRoute::new("shadow")
    };
    assert!(matches!(
        refused(&one_platform_provider().route(shadow)),
        CatalogError::AliasShadowsModel { .. }
    ));
    let dangling = GatewayRoute {
        alias: Some(RouteAlias {
            requested: ModelId::new("claude-next"),
            model: ModelId::new("claude-ghost"),
        }),
        ..GatewayRoute::new("dangling")
    };
    assert!(matches!(
        refused(&one_platform_provider().route(dangling)),
        CatalogError::UnknownAliasTarget { .. }
    ));
}

#[test]
fn a_route_requirement_is_checked_on_every_hop_it_can_reach() {
    let eu_only = GatewayRoute {
        models: ModelGlobs(vec!["claude-*".to_owned()]),
        requires: Some(RouteRequirements {
            european: true,
            no_retain: false,
        }),
        ..GatewayRoute::new("eu-only")
    };
    let fixture = vertex_regions().route(eu_only.clone());
    assert!(matches!(
        refused(&fixture),
        CatalogError::RouteGovernanceUnsatisfied { .. }
    ));
    let _catalog = vertex_regions()
        .with_provider("vertex", |p| p.governance.european = true)
        .route(eu_only)
        .build();
}

#[test]
fn a_model_id_never_carries_the_context_marker() {
    let fixture = CatalogFixture::new()
        .credential("k", platform("k"))
        .provider("p", ProviderConfig::new("k"))
        .endpoint(
            "p",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .model("claude-x[1m]", claude_model(1_000_000))
        .deploy("claude-x[1m]", ["p/api"]);
    assert!(matches!(
        refused(&fixture),
        CatalogError::ContextVariantModelId { .. }
    ));
}
