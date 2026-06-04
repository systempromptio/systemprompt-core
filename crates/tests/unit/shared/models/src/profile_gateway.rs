use std::borrow::Cow;
use std::collections::HashMap;

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, GatewayConfigSpec, GatewayProfileError, GatewayRoute, GatewayState,
    ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol, default_resource_audiences,
    slugify_pattern, synthesize_route_id,
};

fn route(pattern: &str) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new(""),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new("test"),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
    }
}

#[test]
fn exact_pattern_matches() {
    assert!(route("claude-sonnet-4-6").matches("claude-sonnet-4-6"));
    assert!(!route("claude-sonnet-4-6").matches("claude-opus-4-7"));
}

#[test]
fn prefix_wildcard_matches() {
    assert!(route("claude-*").matches("claude-sonnet-4-6"));
    assert!(!route("claude-*").matches("moonshot-v1-8k"));
}

#[test]
fn catch_all_matches() {
    assert!(route("*").matches("any-model-name"));
}

#[test]
fn route_finds_matching_model() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![GatewayRoute {
            id: RouteId::new(""),
            model_pattern: "kimi-*".to_owned(),
            provider: ProviderId::new("moonshot"),
            upstream_model: Some("moonshot-v1-32k".to_owned()),
            extra_headers: HashMap::new(),
            pricing: None,
        }],
        ..GatewayConfig::default()
    };
    let matched = config.find_route("kimi-latest").expect("route must match");
    assert_eq!(matched.provider.as_str(), "moonshot");
    assert_eq!(
        matched.effective_upstream_model("kimi-latest"),
        "moonshot-v1-32k"
    );
}

#[test]
fn slugify_replaces_star_and_non_alnum() {
    assert_eq!(slugify_pattern("claude-*"), "claude-star");
    assert_eq!(slugify_pattern("foo/bar baz!"), "foo-bar-baz");
    assert_eq!(slugify_pattern("---"), "route");
    assert_eq!(slugify_pattern(""), "route");
    assert_eq!(slugify_pattern("Claude_3.7"), "claude-3-7");
}

#[test]
fn synthesize_route_id_is_stable_and_input_dependent() {
    let a = synthesize_route_id("claude-*", "anthropic");
    let b = synthesize_route_id("claude-*", "anthropic");
    assert_eq!(a, b, "synthesize_route_id must be deterministic");
    assert!(a.as_str().starts_with("claude-star-"));

    let c = synthesize_route_id("claude-*", "openai");
    assert_ne!(a, c, "provider change must produce a different id");

    let d = synthesize_route_id("gpt-*", "anthropic");
    assert_ne!(a, d, "model_pattern change must produce a different id");
}

#[test]
fn synthesize_route_id_matches_golden_fnv1a_digests() {
    let cases = [
        ("*", "minimax", "star-2a5453"),
        ("*", "gemini", "star-aac356"),
        ("claude-*", "anthropic", "claude-star-4203d1"),
        ("claude-*", "openai", "claude-star-4f8d12"),
        ("gpt-*", "anthropic", "gpt-star-f15ce8"),
        ("claude-opus-4-8", "gemini", "claude-opus-4-8-46a2bc"),
    ];
    for (pattern, provider, expected) in cases {
        assert_eq!(
            synthesize_route_id(pattern, provider).as_str(),
            expected,
            "FNV-1a route id drifted for ({pattern}, {provider}): a hash-algorithm \
             change has re-keyed gateway routes; fix the regression, do not rebaseline"
        );
    }
}

#[test]
fn ensure_id_backfills_empty_id() {
    let mut r = route("claude-*");
    assert!(r.id.as_str().is_empty());
    r.ensure_id();
    assert!(!r.id.as_str().is_empty());
    let preserved = r.id.clone();
    r.ensure_id();
    assert_eq!(r.id, preserved, "ensure_id must be idempotent");
}

// SSRF endpoint validation now lives on the ProviderRegistry: the gateway owns
// no catalog, and the registry is the authority for outbound connectivity.
fn registry_with_endpoint(endpoint: &str) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("test"),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new("test"),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new("any"),
                aliases: Vec::new(),
                upstream_model: None,
                pricing: Default::default(),
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    }
}

#[test]
fn registry_validate_accepts_public_https_endpoint() {
    assert!(
        registry_with_endpoint("https://api.anthropic.com/v1")
            .validate()
            .is_ok()
    );
}

#[test]
fn registry_validate_allows_loopback_http_for_local_dev() {
    assert!(
        registry_with_endpoint("http://localhost:8080")
            .validate()
            .is_ok()
    );
    assert!(
        registry_with_endpoint("http://127.0.0.1:8080")
            .validate()
            .is_ok()
    );
}

#[test]
fn registry_validate_rejects_cloud_metadata_endpoint() {
    assert!(
        registry_with_endpoint("http://169.254.169.254/latest/meta-data/")
            .validate()
            .is_err()
    );
}

#[test]
fn registry_validate_rejects_private_ranges() {
    for endpoint in [
        "https://10.0.0.5/v1",
        "https://192.168.1.10/v1",
        "https://172.16.0.1/v1",
        "https://[fd00::1]/v1",
    ] {
        assert!(
            registry_with_endpoint(endpoint).validate().is_err(),
            "expected {endpoint} to be rejected as a private/ULA address"
        );
    }
}

#[test]
fn registry_validate_rejects_non_http_scheme_and_plain_http_to_remote() {
    assert!(
        registry_with_endpoint("ftp://example.com/v1")
            .validate()
            .is_err()
    );
    assert!(
        registry_with_endpoint("http://api.anthropic.com/v1")
            .validate()
            .is_err()
    );
}

#[test]
fn validate_rejects_duplicate_route_id() {
    let mut a = route("claude-*");
    a.id = RouteId::new("dup");
    let mut b = route("gpt-*");
    b.id = RouteId::new("dup");
    let config = GatewayConfig {
        enabled: true,
        routes: vec![a, b],
        ..GatewayConfig::default()
    };
    let registry = two_provider_registry();
    match config.validate(&registry) {
        Err(GatewayProfileError::DuplicateRouteId { id }) => assert_eq!(id, "dup"),
        other => panic!("expected DuplicateRouteId, got {other:?}"),
    }
}

fn provider_entry(name: &str, endpoint: &str, models: Vec<ProviderModel>) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        wire: WireProtocol::Anthropic,
        surface: ApiSurface::Anthropic,
        endpoint: endpoint.to_owned(),
        api_key_secret: SecretName::new(name),
        extra_headers: HashMap::new(),
        models,
    }
}

fn model(id: &str) -> ProviderModel {
    ProviderModel {
        id: ModelId::new(id),
        aliases: Vec::new(),
        upstream_model: None,
        pricing: Default::default(),
        capabilities: Default::default(),
        limits: Default::default(),
    }
}

fn two_provider_registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            provider_entry(
                "anthropic",
                "https://api.anthropic.com/v1",
                vec![model("claude-sonnet-4-20250514")],
            ),
            provider_entry(
                "gemini",
                "https://generativelanguage.googleapis.com/v1beta",
                vec![model("gemini-2.5-flash")],
            ),
        ],
    }
}

fn two_provider_config(default_provider: Option<&str>) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes: vec![
            route_to("claude-*", "anthropic"),
            route_to("gemini-*", "gemini"),
        ],
        default_provider: default_provider.map(ProviderId::new),
        ..GatewayConfig::default()
    }
}

fn route_to(pattern: &str, provider: &str) -> GatewayRoute {
    let mut r = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
    };
    r.ensure_id();
    r
}

#[test]
fn resolve_route_prefers_explicit_match_over_default() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let resolved = config
        .resolve_route(&registry, "claude-opus-4-7")
        .expect("explicit route must match");
    assert_eq!(resolved.provider.as_str(), "anthropic");
}

#[test]
fn resolve_route_falls_back_to_default_provider() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let resolved = config
        .resolve_route(&registry, "some-unknown-model")
        .expect("default provider must absorb unmatched model");
    assert_eq!(resolved.provider.as_str(), "gemini");
    // The synthetic default route forwards the requested model verbatim;
    // per-model upstream rewrites are applied downstream from the registry.
    assert_eq!(
        resolved.effective_upstream_model("some-unknown-model"),
        "some-unknown-model",
        "synthetic default route must pass the requested model through unchanged"
    );
}

#[test]
fn resolve_route_is_none_without_default_or_match() {
    let config = two_provider_config(None);
    let registry = two_provider_registry();
    assert!(
        config
            .resolve_route(&registry, "some-unknown-model")
            .is_none()
    );
}

#[test]
fn is_model_exposed_is_closed_by_default_even_with_default_provider() {
    let registry = two_provider_registry();
    assert!(
        !two_provider_config(None).is_model_exposed(&registry, "some-unknown-model"),
        "closed gateway must deny unknown models"
    );
    // A default provider authorizes the synthetic catch-all route, but it does
    // NOT, on its own, expose an unlisted model to dispatch; the gateway stays
    // a closed allowlist unless allow_unlisted_models is set.
    assert!(
        !two_provider_config(Some("gemini")).is_model_exposed(&registry, "some-unknown-model"),
        "a default provider alone must not open the gateway to unlisted models"
    );
}

#[test]
fn is_model_exposed_opens_only_when_allow_unlisted_models() {
    let registry = two_provider_registry();
    let open = GatewayConfig {
        allow_unlisted_models: true,
        ..two_provider_config(Some("gemini"))
    };
    assert!(
        open.is_model_exposed(&registry, "some-unknown-model"),
        "allow_unlisted_models opts into forwarding unlisted models to default_provider"
    );
    // …but a routed model and a registry model are exposed regardless of the flag.
    assert!(open.is_model_exposed(&registry, "claude-sonnet-4-20250514"));
    let closed_no_default = GatewayConfig {
        allow_unlisted_models: true,
        ..two_provider_config(None)
    };
    assert!(
        !closed_no_default.is_model_exposed(&registry, "some-unknown-model"),
        "allow_unlisted_models without a default_provider still denies unknown models"
    );
}

#[test]
fn is_model_exposed_admits_registry_model() {
    let registry = two_provider_registry();
    assert!(
        two_provider_config(None).is_model_exposed(&registry, "claude-sonnet-4-20250514"),
        "a model present in the registry must be exposed even without a default provider"
    );
}

#[test]
fn validate_rejects_default_provider_absent_from_registry() {
    let registry = two_provider_registry();
    match two_provider_config(Some("openai")).validate(&registry) {
        Err(GatewayProfileError::DefaultProviderNotInRegistry { provider }) => {
            assert_eq!(provider, "openai");
        },
        other => panic!("expected DefaultProviderNotInRegistry, got {other:?}"),
    }
    assert!(
        two_provider_config(Some("gemini"))
            .validate(&registry)
            .is_ok(),
        "a default provider present in the registry must validate"
    );
}

fn route_id(route: Cow<'_, GatewayRoute>) -> RouteId {
    let mut route = route.into_owned();
    route.ensure_id();
    route.id
}

#[test]
fn dispatchable_route_ids_cover_every_candidate_route() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let ids = config.dispatchable_route_ids(&registry);

    for route in config.candidate_routes(&registry) {
        let id = route_id(route);
        assert!(
            ids.contains(&id),
            "candidate {id:?} absent from catalog {ids:?}"
        );
    }

    let resolved = config
        .resolve_route(&registry, "some-unknown-model")
        .expect("default provider must absorb unmatched model");
    assert!(ids.contains(&route_id(resolved)));
}

#[test]
fn dispatchable_route_ids_omits_default_when_unset() {
    let registry = two_provider_registry();
    let ids = two_provider_config(None).dispatchable_route_ids(&registry);
    assert_eq!(ids.len(), 2);
}

#[test]
fn dispatchable_route_ids_dedupes_explicit_catch_all() {
    let mut config = two_provider_config(Some("gemini"));
    config.routes.push(route_to("*", "gemini"));
    let registry = two_provider_registry();
    let ids = config.dispatchable_route_ids(&registry);
    let mut unique = ids.clone();
    unique.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "route ids must be unique: {ids:?}");
}

#[test]
fn gateway_state_dispatchable_route_ids_resolves_spec() {
    let config = two_provider_config(Some("gemini"));
    let registry = two_provider_registry();
    let from_state = GatewayState::Spec(config.to_spec()).dispatchable_route_ids(&registry);
    assert_eq!(from_state, config.dispatchable_route_ids(&registry));
}

#[test]
fn gateway_spec_round_trips_default_provider() {
    let spec = two_provider_config(Some("gemini")).to_spec();
    let yaml = serde_yaml::to_string(&spec).expect("serialize");
    assert!(yaml.contains("default_provider: gemini"), "got:\n{yaml}");

    let back: GatewayConfigSpec = serde_yaml::from_str(&yaml).expect("round-trip");
    assert_eq!(
        back.default_provider.as_ref().map(ProviderId::as_str),
        Some("gemini")
    );
}

#[test]
fn default_resource_audiences_cover_gateway_requirements() {
    let audiences = default_resource_audiences();
    assert!(audiences.contains(&"hook".to_owned()));
    assert!(!audiences.is_empty());
}
