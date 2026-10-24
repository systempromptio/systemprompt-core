//! `ResolvedCatalog::resolve` without policy routes: the model's own
//! deployments are the chain, in order, every hop fully resolved.

use systemprompt_catalog_fixtures::{
    CatalogFixture, OPENCODE_GO_CHAT_URL, OPENCODE_GO_MESSAGES_URL, claude_model,
    one_platform_provider, opencode_go, vertex_regions,
};
use systemprompt_manifest::services::catalog::{
    DeploymentDetail, DeploymentTarget, EndpointRef, ModelRequest, PricingOverride, ResolveError,
};
use systemprompt_manifest::services::synthesize_route_id;
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;
use systemprompt_wire::anthropic::AnthropicBeta;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn a_model_resolves_through_its_deployment_to_endpoint_and_credential() {
    let catalog = opencode_go().install("example").build();
    let target = catalog
        .resolve(&ModelRequest::new("go-kimi-k3"))
        .expect("resolves");
    let hop = target.primary().expect("one hop");
    assert_eq!(hop.provider.as_str(), "opencode-go");
    assert_eq!(hop.endpoint.as_str(), "chat");
    assert_eq!(hop.wire, WireProtocol::OpenAiChat);
    assert_eq!(hop.url, OPENCODE_GO_CHAT_URL);
    assert_eq!(hop.upstream_model, "kimi-k3");
    assert_eq!(hop.credential.id.as_str(), "opencode-go");
    assert!(hop.credential.is_personal());
    assert!(hop.credential.platform_secret().is_none());
    assert!(target.route.is_none());

    let messages = catalog
        .resolve(&ModelRequest::new("go-minimax-m3"))
        .expect("resolves");
    let hop = messages.primary().expect("one hop");
    assert_eq!(hop.wire, WireProtocol::Anthropic);
    assert_eq!(hop.url, OPENCODE_GO_MESSAGES_URL);
    assert_eq!(
        hop.betas.as_ref().map(std::collections::BTreeSet::len),
        Some(0)
    );
    assert_eq!(
        hop.credential.id, target.chain[0].credential.id,
        "both wires of one subscription bill one credential"
    );
}

#[test]
fn install_placeholders_are_expanded_from_the_profile() {
    let catalog = opencode_go().install("example").build();
    let provider = catalog.provider("opencode-go").expect("declared");
    assert_eq!(
        provider.user_agent.as_deref(),
        Some("systemprompt-gateway (opencode-go; example)")
    );
    assert!(provider.forwards_header("X-OpenCode-Session"));
}

#[test]
fn the_context_marker_is_stripped_before_lookup() {
    let catalog = one_platform_provider().build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5[1m]"))
        .expect("resolves");
    assert_eq!(target.requested, "claude-sonnet-5");
    assert_eq!(target.chain[0].upstream_model, "claude-sonnet-5");
}

#[test]
fn an_unknown_model_is_refused_not_forwarded() {
    let catalog = one_platform_provider().build();
    assert_eq!(
        catalog
            .resolve(&ModelRequest::new("gpt-9"))
            .expect_err("unknown"),
        ResolveError::UnknownModel {
            requested: "gpt-9".to_owned()
        }
    );
}

#[test]
fn deployment_order_is_failover_order() {
    let catalog = vertex_regions().build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    let endpoints: Vec<&str> = target.chain.iter().map(|h| h.endpoint.as_str()).collect();
    assert_eq!(endpoints, vec!["claude-global", "claude-eu"]);
    assert!(target.chain[0].bills_same_credential(&target.chain[1]));
    assert_eq!(target.chain[1].region.as_deref(), Some("europe-west4"));
}

#[test]
fn inherited_betas_are_the_siblings_list() {
    let catalog = vertex_regions().build();
    let hops = catalog.deployments("claude-sonnet-5");
    let expected: std::collections::BTreeSet<_> = [AnthropicBeta::new("context-1m-2025-08-07")]
        .into_iter()
        .collect();
    assert_eq!(hops[0].betas.as_ref(), Some(&expected));
    assert_eq!(hops[1].betas.as_ref(), Some(&expected));
}

#[test]
fn pricing_precedence_is_override_then_multiplier_then_base() {
    let catalog = vertex_regions().build();
    let hops = catalog.deployments("claude-sonnet-5");
    assert!(
        close(hops[0].pricing.input_per_million, 3.0),
        "base on the global endpoint"
    );
    assert!(
        close(hops[1].pricing.input_per_million, 3.3),
        "EU premium from price_multiplier"
    );
    assert!(close(
        hops[1].pricing.cache_read_per_million.unwrap_or_default(),
        0.33
    ));

    let override_ = DeploymentTarget::Detailed(DeploymentDetail {
        endpoint: EndpointRef::new("vertex", "claude-eu"),
        upstream_model: Some("claude-sonnet-5@eu".to_owned()),
        pricing: Some(PricingOverride {
            input_per_million: Some(2.0),
            ..PricingOverride::default()
        }),
        clients: None,
    });
    let catalog = vertex_regions()
        .deploy(
            "claude-sonnet-5",
            [
                DeploymentTarget::from(EndpointRef::new("vertex", "claude-global")),
                override_,
            ],
        )
        .build();
    let eu = &catalog.deployments("claude-sonnet-5")[1];
    assert!(
        close(eu.pricing.input_per_million, 2.0),
        "the override field wins outright"
    );
    assert!(
        close(eu.pricing.output_per_million, 16.5),
        "unset fields keep the multiplied base"
    );
    assert_eq!(eu.upstream_model, "claude-sonnet-5@eu");
}

#[test]
fn clients_decide_advertisement_and_an_empty_list_hides_a_model() {
    let catalog = one_platform_provider().build();
    let claude: Vec<&str> = catalog
        .advertised_models(Some(ClientFamily::Claude))
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(claude, vec!["claude-sonnet-5"]);
    assert_eq!(
        catalog
            .advertised_models(Some(ClientFamily::OpenAi))
            .count(),
        0
    );
    let hidden = catalog.model("claude-internal").expect("declared");
    assert!(!hidden.is_advertised());
    assert!(
        catalog
            .resolve(&ModelRequest::new("claude-internal"))
            .is_ok(),
        "not advertised is not unservable"
    );
}

#[test]
fn a_disabled_provider_leaves_every_chain() {
    let catalog = vertex_regions()
        .credential(
            "anthropic",
            systemprompt_catalog_fixtures::platform("anthropic"),
        )
        .provider(
            "anthropic",
            systemprompt_manifest::services::catalog::ProviderConfig {
                enabled: false,
                ..systemprompt_manifest::services::catalog::ProviderConfig::new("anthropic")
            },
        )
        .endpoint(
            "anthropic",
            "api",
            systemprompt_manifest::services::catalog::EndpointConfig::new(
                WireProtocol::Anthropic,
                "https://api.anthropic.com/v1",
            ),
        )
        .model("opus-5", claude_model(1_000_000))
        .deploy("opus-5", ["anthropic/api"])
        .deploy(
            "claude-sonnet-5",
            ["anthropic/api", "vertex/claude-global", "vertex/claude-eu"],
        )
        .build();
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert_eq!(target.chain.len(), 2);
    assert!(
        target
            .chain
            .iter()
            .all(|hop| hop.provider.as_str() == "vertex")
    );
    assert_eq!(
        catalog
            .resolve(&ModelRequest::new("opus-5"))
            .expect_err("no hop"),
        ResolveError::NoEnabledDeployment {
            model: systemprompt_identifiers::ModelId::new("opus-5")
        }
    );
}

#[test]
fn capabilities_start_from_the_provider_and_apply_model_overrides() {
    let mut provider = systemprompt_manifest::services::catalog::ProviderConfig::new("k");
    provider.capabilities.vision = true;
    provider.capabilities.tools = true;
    let mut entry = claude_model(200_000);
    entry.capabilities.insert(
        systemprompt_manifest::services::catalog::Capability::Vision,
        false,
    );
    let catalog = CatalogFixture::new()
        .credential("k", systemprompt_catalog_fixtures::platform("k"))
        .provider("p", provider)
        .endpoint(
            "p",
            "api",
            systemprompt_manifest::services::catalog::EndpointConfig::new(
                WireProtocol::Anthropic,
                "https://api.anthropic.com/v1",
            ),
        )
        .model("m", entry)
        .deploy("m", ["p/api"])
        .build();
    let model = catalog.model("m").expect("declared");
    assert!(model.capabilities.tools, "inherited from the provider");
    assert!(!model.capabilities.vision, "overridden by the model");
}

#[test]
fn the_catalog_route_id_defaults_to_the_072_implicit_route_id() {
    let catalog = one_platform_provider().build();
    assert_eq!(
        catalog.catalog_route_id(),
        &synthesize_route_id("*", "anthropic")
    );
    let target = catalog
        .resolve(&ModelRequest::new("claude-sonnet-5"))
        .expect("resolves");
    assert_eq!(&target.access_route_id, catalog.catalog_route_id());
    assert!(catalog.route_ids().contains(catalog.catalog_route_id()));
}
