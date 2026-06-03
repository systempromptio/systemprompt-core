//! Unit tests for the API-surface-scoped model filter: `ApiSurface::from_tag`
//! (the inverse of `as_tag`, used to parse the `x-inference-protocol` header),
//! `WireProtocol::surface` (the wire-to-surface mapping), and
//! `ProviderRegistry::advertised_model_ids` (the single source of which models
//! a given client surface may see). A `surface: backend` provider is reachable
//! only through routes and must never appear in an advertised catalog.

use systemprompt_identifiers::{ModelId, ProviderId, SecretName};
use systemprompt_models::profile::{
    ApiSurface, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};

const ALL_SURFACES: &[ApiSurface] = &[
    ApiSurface::Anthropic,
    ApiSurface::OpenAi,
    ApiSurface::Gemini,
    ApiSurface::Backend,
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

fn provider(
    name: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    models: Vec<ProviderModel>,
) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        wire,
        surface,
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
                ApiSurface::Anthropic,
                vec![model("claude-sonnet-4-6", &["claude-sonnet"])],
            ),
            provider(
                "openai",
                WireProtocol::OpenAiChat,
                ApiSurface::OpenAi,
                vec![model("gpt-5", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                ApiSurface::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    }
}

#[test]
fn surface_from_tag_round_trips_every_surface() {
    for surface in ALL_SURFACES {
        assert_eq!(ApiSurface::from_tag(surface.as_tag()), Some(*surface));
    }
}

#[test]
fn surface_from_tag_rejects_unknown() {
    assert_eq!(ApiSurface::from_tag("grok"), None);
    assert_eq!(ApiSurface::from_tag(""), None);
}

#[test]
fn both_openai_wires_map_to_one_surface() {
    assert_eq!(WireProtocol::OpenAiChat.surface(), ApiSurface::OpenAi);
    assert_eq!(WireProtocol::OpenAiResponses.surface(), ApiSurface::OpenAi);
    assert_eq!(WireProtocol::Anthropic.surface(), ApiSurface::Anthropic);
    assert_eq!(WireProtocol::Gemini.surface(), ApiSurface::Gemini);
}

#[test]
fn advertised_scopes_to_surface() {
    let registry = registry();

    assert_eq!(
        registry.advertised_model_ids(&[ApiSurface::Anthropic]),
        vec!["claude-sonnet-4-6".to_owned(), "claude-sonnet".to_owned()]
    );
    assert_eq!(
        registry.advertised_model_ids(&[ApiSurface::Gemini]),
        vec!["gemini-3.1-flash-lite-preview".to_owned()]
    );
}

#[test]
fn advertised_unions_multiple_surfaces() {
    let models = registry().advertised_model_ids(&[ApiSurface::OpenAi, ApiSurface::Gemini]);
    assert!(models.iter().any(|m| m == "gpt-5"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
    assert!(!models.iter().any(|m| m.starts_with("claude")));
}

#[test]
fn advertised_empty_surfaces_returns_all() {
    let models = registry().advertised_model_ids(&[]);
    assert!(models.iter().any(|m| m == "claude-sonnet-4-6"));
    assert!(models.iter().any(|m| m == "gpt-5"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
}

#[test]
fn backend_surface_is_never_advertised() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                ApiSurface::Anthropic,
                vec![model("claude-3-7-sonnet-20250219", &[])],
            ),
            provider(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    assert_eq!(
        registry.advertised_model_ids(&[ApiSurface::Anthropic]),
        vec!["claude-3-7-sonnet-20250219".to_owned()]
    );
    assert!(
        !registry
            .advertised_model_ids(&[])
            .iter()
            .any(|m| m == "MiniMax-M2")
    );
    assert!(
        registry.contains_model("MiniMax-M2"),
        "backend model stays routable and cost-attributable via the registry"
    );
}

#[test]
fn is_advertised_excludes_only_backend() {
    assert!(ApiSurface::Anthropic.is_advertised());
    assert!(ApiSurface::OpenAi.is_advertised());
    assert!(ApiSurface::Gemini.is_advertised());
    assert!(!ApiSurface::Backend.is_advertised());
}

#[test]
fn advertised_providers_excludes_backend() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                ApiSurface::Anthropic,
                vec![model("claude-3-7-sonnet-20250219", &[])],
            ),
            provider(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    let names: Vec<&str> = registry
        .advertised_providers()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, vec!["anthropic"]);
}

#[test]
fn bridge_profile_dto_round_trips_typed_surface() {
    use systemprompt_models::bridge::profile::ProviderHealth;

    let health = ProviderHealth {
        name: "anthropic".to_owned(),
        surface: ApiSurface::Anthropic,
        configured: true,
        models: vec!["claude-sonnet-4-6".to_owned()],
        config_issue: None,
    };

    let json = serde_json::to_string(&health).expect("serialize");
    assert!(json.contains("\"surface\":\"anthropic\""));

    let back: ProviderHealth = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.surface, ApiSurface::Anthropic);
    assert_eq!(back.name, "anthropic");
}

#[test]
fn bridge_profile_build_excludes_backend_provider() {
    use systemprompt_models::bridge::profile;

    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                ApiSurface::Anthropic,
                vec![model("claude-3-7-sonnet-20250219", &[])],
            ),
            provider(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    let response = profile::build(
        "https://gw.invalid/v1".to_owned(),
        "bearer".to_owned(),
        None,
        &registry,
        |_| true,
    );

    assert!(response.providers.iter().all(|p| p.name != "minimax"));
    assert!(!response.models.iter().any(|m| m == "MiniMax-M2"));
    assert_eq!(response.models, vec!["claude-3-7-sonnet-20250219".to_owned()]);
}
