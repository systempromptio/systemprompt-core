//! Unit tests for the protocol-scoped model filter: `WireProtocol::from_tag`
//! (the inverse of `as_tag`, used to parse the `x-inference-protocol` header)
//! and `ProviderRegistry::advertised_model_ids` (the single source of which
//! models a given wire-protocol client may see).

use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::profile::{ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol};

const ALL: &[WireProtocol] = &[
    WireProtocol::Anthropic,
    WireProtocol::OpenAiChat,
    WireProtocol::OpenAiResponses,
    WireProtocol::Gemini,
];

fn model(id: &str, aliases: &[&str]) -> ProviderModel {
    ProviderModel {
        id: ModelId::new(id),
        aliases: aliases.iter().map(|a| ModelId::new(*a)).collect(),
        upstream_model: None,
        pricing: Default::default(),
        capabilities: Default::default(),
        limits: Default::default(),
    }
}

fn provider(name: &str, protocol: WireProtocol, models: Vec<ProviderModel>) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        protocol,
        endpoint: "https://example.invalid/v1".to_owned(),
        api_key_secret: SecretName::new(name),
        extra_headers: Default::default(),
        models,
    }
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &["claude-sonnet"])],
            ),
            provider(
                "openai",
                WireProtocol::OpenAiChat,
                vec![model("gpt-5", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    }
}

#[test]
fn from_tag_round_trips_every_protocol() {
    for protocol in ALL {
        assert_eq!(WireProtocol::from_tag(protocol.as_tag()), Some(*protocol));
    }
}

#[test]
fn from_tag_accepts_aliases() {
    assert_eq!(
        WireProtocol::from_tag("openai"),
        Some(WireProtocol::OpenAiChat)
    );
    assert_eq!(
        WireProtocol::from_tag("openai_responses"),
        Some(WireProtocol::OpenAiResponses)
    );
}

#[test]
fn from_tag_rejects_unknown() {
    assert_eq!(WireProtocol::from_tag("grok"), None);
    assert_eq!(WireProtocol::from_tag(""), None);
}

#[test]
fn advertised_scopes_to_protocol() {
    let registry = registry();

    assert_eq!(
        registry.advertised_model_ids(&[WireProtocol::Anthropic]),
        vec!["claude-sonnet-4-6".to_owned(), "claude-sonnet".to_owned()]
    );
    assert_eq!(
        registry.advertised_model_ids(&[WireProtocol::Gemini]),
        vec!["gemini-3.1-flash-lite-preview".to_owned()]
    );
}

#[test]
fn advertised_unions_multiple_protocols() {
    let models = registry().advertised_model_ids(&[WireProtocol::OpenAiChat, WireProtocol::Gemini]);
    assert!(models.iter().any(|m| m == "gpt-5"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
    assert!(!models.iter().any(|m| m.starts_with("claude")));
}

#[test]
fn advertised_empty_protocols_returns_all() {
    let models = registry().advertised_model_ids(&[]);
    assert!(models.iter().any(|m| m == "claude-sonnet-4-6"));
    assert!(models.iter().any(|m| m == "gpt-5"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
}
