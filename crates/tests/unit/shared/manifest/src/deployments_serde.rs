//! The YAML shapes of `deployments.yaml`, `providers.yaml` endpoints and
//! `models.yaml` entries.

use systemprompt_manifest::services::catalog::{
    BetaSpec, Capability, DeploymentRegistry, DeploymentTarget, EndpointConfig, EndpointRef,
    ModelEntry, ProviderRegistry,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;
use systemprompt_wire::anthropic::AnthropicBeta;

#[test]
fn a_deployment_list_mixes_bare_refs_and_detailed_entries() {
    let yaml = "deepseek-v4-pro:\n  - deepseek/messages\n  - endpoint: opencode-go/chat\n    upstream_model: deepseek-v4-pro\n    pricing: { cache_read_per_million: 0.132 }\n";
    let registry: DeploymentRegistry = serde_yaml::from_str(yaml).expect("deployments parse");
    let targets = registry.get("deepseek-v4-pro").expect("model deployed");
    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets[0],
        DeploymentTarget::Endpoint(EndpointRef::new("deepseek", "messages"))
    );
    assert_eq!(
        targets[1].endpoint(),
        &EndpointRef::new("opencode-go", "chat")
    );
    assert_eq!(targets[1].upstream_model(), Some("deepseek-v4-pro"));
    let pricing = targets[1].pricing().expect("override");
    assert_eq!(pricing.cache_read_per_million, Some(0.132));
    assert_eq!(pricing.input_per_million, None);
}

#[test]
fn an_endpoint_ref_is_exactly_provider_slash_endpoint() {
    assert!(EndpointRef::parse("vertex/claude-eu").is_ok());
    for bad in ["vertex", "/claude", "vertex/", "a/b/c", ""] {
        assert!(EndpointRef::parse(bad).is_err(), "{bad:?} must be refused");
    }
    let refused: Result<DeploymentRegistry, _> = serde_yaml::from_str("m: [not-a-ref]\n");
    assert!(refused.is_err());
}

#[test]
fn an_endpoint_ref_round_trips_through_yaml() {
    let target = DeploymentTarget::Endpoint(EndpointRef::new("vertex", "claude-eu"));
    let text = serde_yaml::to_string(&target).expect("serialise");
    assert_eq!(text.trim(), "vertex/claude-eu");
}

#[test]
fn a_detailed_deployment_refuses_unknown_fields() {
    let yaml = "m:\n  - endpoint: a/b\n    weight: 3\n";
    let refused: Result<DeploymentRegistry, _> = serde_yaml::from_str(yaml);
    assert!(
        refused.is_err(),
        "weight belongs to routes, not deployments"
    );
}

#[test]
fn endpoint_betas_are_a_list_or_an_inherit() {
    let yaml = "vertex:\n  credential: vertex-sa\n  endpoints:\n    claude-global: { wire: anthropic, url: \"https://aiplatform.googleapis.com/v1\", accepted_betas: [a-2025, b-2025] }\n    claude-eu: { wire: anthropic, url: \"https://aiplatform.googleapis.com/v1\", accepted_betas: \"inherit:claude-global\", price_multiplier: 1.1, region: europe-west4 }\n";
    let registry: ProviderRegistry = serde_yaml::from_str(yaml).expect("providers parse");
    let vertex = registry.get("vertex").expect("vertex declared");
    let global = vertex.endpoint("claude-global").expect("global");
    match global.accepted_betas.as_ref() {
        Some(BetaSpec::List(list)) => assert!(list.contains(&AnthropicBeta::new("a-2025"))),
        other => panic!("expected a list, got {other:?}"),
    }
    let eu = vertex.endpoint("claude-eu").expect("eu");
    let inherited = eu
        .accepted_betas
        .as_ref()
        .and_then(BetaSpec::inherited_endpoint)
        .expect("inherit")
        .expect("well formed");
    assert_eq!(inherited.as_str(), "claude-global");
    assert_eq!(eu.price_multiplier, Some(1.1));
    assert!(
        vertex.enabled,
        "providers are enabled unless they say otherwise"
    );
}

#[test]
fn an_endpoint_refuses_unknown_fields() {
    let refused: Result<EndpointConfig, _> =
        serde_yaml::from_str("{ wire: anthropic, url: \"https://x.test\", surface: anthropic }");
    assert!(
        refused.is_err(),
        "`surface` is gone; clients live on models"
    );
    let ok: EndpointConfig =
        serde_yaml::from_str("{ wire: openai-chat, url: \"https://x.test\" }").expect("parses");
    assert_eq!(ok.wire, WireProtocol::OpenAiChat);
}

#[test]
fn a_model_requires_clients_and_lists_only_capability_overrides() {
    let missing: Result<ModelEntry, _> = serde_yaml::from_str("{ pricing: {} }");
    assert!(
        missing.is_err(),
        "clients is required; [] means not advertised"
    );
    let model: ModelEntry =
        serde_yaml::from_str("{ clients: [claude, openai], capabilities: { vision: false } }")
            .expect("parses");
    assert_eq!(
        model.clients,
        vec![ClientFamily::Claude, ClientFamily::OpenAi]
    );
    assert_eq!(model.capabilities.get(&Capability::Vision), Some(&false));
    assert_eq!(model.capabilities.len(), 1);
}
