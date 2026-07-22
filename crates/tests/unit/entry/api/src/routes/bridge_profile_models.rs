//! Unit tests for the protocol-scoped model filter that selects which provider
//! models are advertised to Cowork / Claude Desktop. Gateway-mode hosts reject
//! the whole enterprise config if any advertised model is not from their wire
//! protocol, so the `/bridge/profile` front door and `/v1/models` both scope
//! the list via `ProviderRegistry::advertised_model_ids` /
//! `models::model_entries`.

use systemprompt_api::routes::gateway::bridge::{canonicalize_org_uuid, provider_health};
use systemprompt_api::routes::gateway::models::model_entries;
use systemprompt_identifiers::{ModelId, ProviderId, SecretName, TenantId};
use systemprompt_models::profile::{
    ApiSurface, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};

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

fn provider(name: &str, wire: WireProtocol, models: Vec<ProviderModel>) -> ProviderEntry {
    provider_with_secret(name, wire, name, models)
}

fn provider_with_secret(
    name: &str,
    wire: WireProtocol,
    secret: &str,
    models: Vec<ProviderModel>,
) -> ProviderEntry {
    provider_with_surface(name, wire, wire.surface(), secret, models)
}

fn provider_with_surface(
    name: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    secret: &str,
    models: Vec<ProviderModel>,
) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        wire,
        surface,
        endpoint: "https://example.invalid/v1".to_owned(),
        api_key_secret: SecretName::new(secret),
        extra_headers: Default::default(),
        models,
    }
}

#[test]
fn includes_anthropic_ids_and_aliases() {
    let registry = ProviderRegistry {
        providers: vec![provider(
            "anthropic",
            WireProtocol::Anthropic,
            vec![
                model("claude-sonnet-4-6", &["claude-sonnet"]),
                model("claude-haiku-4-5", &[]),
            ],
        )],
    };

    let models = registry.advertised_model_ids(&[ApiSurface::Anthropic]);

    assert_eq!(
        models,
        vec![
            "claude-sonnet-4-6".to_owned(),
            "claude-sonnet".to_owned(),
            "claude-haiku-4-5".to_owned(),
        ]
    );
}

#[test]
fn excludes_non_anthropic_providers() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &["gemini-flash"])],
            ),
            provider(
                "openai",
                WireProtocol::OpenAiChat,
                vec![model("gpt-5", &[])],
            ),
        ],
    };

    let models = registry.advertised_model_ids(&[ApiSurface::Anthropic]);

    assert_eq!(models, vec!["claude-sonnet-4-6".to_owned()]);
    assert!(!models.iter().any(|m| m.starts_with("gemini")));
    assert!(!models.iter().any(|m| m.starts_with("gpt")));
}

#[test]
fn empty_when_no_anthropic_provider() {
    let registry = ProviderRegistry {
        providers: vec![provider(
            "gemini",
            WireProtocol::Gemini,
            vec![model("gemini-3.1-flash-lite-preview", &[])],
        )],
    };

    assert!(
        registry
            .advertised_model_ids(&[ApiSurface::Anthropic])
            .is_empty()
    );
}

#[test]
fn empty_protocols_returns_full_catalog() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    };

    let models = registry.advertised_model_ids(&[]);

    assert!(models.iter().any(|m| m == "claude-sonnet-4-6"));
    assert!(models.iter().any(|m| m == "gemini-3.1-flash-lite-preview"));
}

#[test]
fn model_entries_scope_to_requested_protocol() {
    let registry = ProviderRegistry {
        providers: vec![
            provider(
                "anthropic",
                WireProtocol::Anthropic,
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider(
                "gemini",
                WireProtocol::Gemini,
                vec![model("gemini-3.1-flash-lite-preview", &["gemini-flash"])],
            ),
        ],
    };

    let entries = model_entries(&registry, &[ApiSurface::Anthropic]);

    let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["claude-sonnet-4-6"]);
    assert!(entries.iter().all(|e| e.kind == "model"));
    assert!(!ids.iter().any(|id| id.starts_with("gemini")));
}

#[test]
fn provider_health_reports_configured_and_models() {
    let registry = ProviderRegistry {
        providers: vec![provider_with_secret(
            "anthropic",
            WireProtocol::Anthropic,
            "anthropic_key",
            vec![model("claude-sonnet-4-6", &["claude-sonnet"])],
        )],
    };

    let health = provider_health(&registry, |name| name == "anthropic_key");

    assert_eq!(health.len(), 1);
    let entry = &health[0];
    assert_eq!(entry.name, "anthropic");
    assert_eq!(entry.surface, ApiSurface::Anthropic);
    assert!(entry.configured);
    assert!(entry.config_issue.is_none());
    assert_eq!(
        entry.models,
        vec!["claude-sonnet-4-6".to_owned(), "claude-sonnet".to_owned()]
    );
}

#[test]
fn provider_health_flags_missing_secret() {
    let registry = ProviderRegistry {
        providers: vec![
            provider_with_secret(
                "anthropic",
                WireProtocol::Anthropic,
                "anthropic_key",
                vec![model("claude-sonnet-4-6", &[])],
            ),
            provider_with_secret(
                "gemini",
                WireProtocol::Gemini,
                "gemini_key",
                vec![model("gemini-3.1-flash-lite-preview", &[])],
            ),
        ],
    };

    let health = provider_health(&registry, |name| name == "anthropic_key");

    let gemini = health.iter().find(|h| h.name == "gemini").unwrap();
    assert!(!gemini.configured);
    assert_eq!(
        gemini.config_issue.as_deref(),
        Some("API key secret 'gemini_key' is not configured")
    );

    let anthropic = health.iter().find(|h| h.name == "anthropic").unwrap();
    assert!(anthropic.configured);
}

#[test]
fn provider_health_excludes_backend_provider() {
    let registry = ProviderRegistry {
        providers: vec![
            provider_with_secret(
                "anthropic",
                WireProtocol::Anthropic,
                "anthropic_key",
                vec![model("claude-3-7-sonnet-20250219", &[])],
            ),
            provider_with_surface(
                "minimax",
                WireProtocol::Anthropic,
                ApiSurface::Backend,
                "minimax_key",
                vec![model("MiniMax-M2", &[])],
            ),
        ],
    };

    let health = provider_health(&registry, |_| true);

    assert!(
        health.iter().all(|h| h.name != "minimax"),
        "backend provider must never appear in bridge profile provider health"
    );
    assert!(
        !health
            .iter()
            .flat_map(|h| h.models.iter())
            .any(|m| m == "MiniMax-M2")
    );
}

#[test]
fn canonicalize_org_uuid_peels_local_prefix_from_uuid_tenant() {
    let tenant = TenantId::new("local_6fa459ea-ee8a-3ca4-894e-db77e160355e");
    assert_eq!(
        canonicalize_org_uuid(&tenant),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn canonicalize_org_uuid_normalizes_bare_uuid_to_lowercase_canonical() {
    let tenant = TenantId::new("6FA459EA-EE8A-3CA4-894E-DB77E160355E");
    assert_eq!(
        canonicalize_org_uuid(&tenant),
        "6fa459ea-ee8a-3ca4-894e-db77e160355e"
    );
}

#[test]
fn canonicalize_org_uuid_derives_stable_v5_for_non_uuid_tenant() {
    let tenant = TenantId::new("local_acme-corp");
    let got = canonicalize_org_uuid(&tenant);
    assert_eq!(got.len(), 36, "{got}");
    assert_eq!(got.as_bytes()[14], b'5', "expected a v5 UUID: {got}");
    assert_eq!(
        got,
        canonicalize_org_uuid(&tenant),
        "derivation must be deterministic"
    );
    assert_ne!(
        got,
        canonicalize_org_uuid(&TenantId::new("local_other-corp")),
        "distinct tenants must not collide"
    );
}
