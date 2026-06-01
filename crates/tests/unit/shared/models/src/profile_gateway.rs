use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    GatewayCatalog, GatewayCatalogSource, GatewayConfig, GatewayConfigSpec, GatewayModel,
    GatewayProfileError, GatewayProvider, GatewayRoute, default_resource_audiences, slugify_pattern,
    synthesize_route_id,
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
fn ensure_id_backfills_empty_id() {
    let mut r = route("claude-*");
    assert!(r.id.as_str().is_empty());
    r.ensure_id();
    assert!(!r.id.as_str().is_empty());
    let preserved = r.id.clone();
    r.ensure_id();
    assert_eq!(r.id, preserved, "ensure_id must be idempotent");
}

fn catalog_with_endpoint(endpoint: &str) -> GatewayCatalog {
    GatewayCatalog {
        providers: vec![GatewayProvider {
            name: ProviderId::new("test"),
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new("test"),
            extra_headers: HashMap::new(),
        }],
        models: vec![GatewayModel {
            id: ModelId::new("any"),
            provider: ProviderId::new("test"),
            aliases: Vec::new(),
            display_name: None,
            upstream_model: None,
            pricing: None,
        }],
    }
}

#[test]
fn catalog_validate_accepts_public_https_endpoint() {
    assert!(
        catalog_with_endpoint("https://api.anthropic.com/v1")
            .validate()
            .is_ok()
    );
}

#[test]
fn catalog_validate_allows_loopback_http_for_local_dev() {
    assert!(
        catalog_with_endpoint("http://localhost:8080")
            .validate()
            .is_ok()
    );
    assert!(
        catalog_with_endpoint("http://127.0.0.1:8080")
            .validate()
            .is_ok()
    );
}

#[test]
fn catalog_validate_rejects_cloud_metadata_endpoint() {
    assert!(
        catalog_with_endpoint("http://169.254.169.254/latest/meta-data/")
            .validate()
            .is_err()
    );
}

#[test]
fn catalog_validate_rejects_private_ranges() {
    for endpoint in [
        "https://10.0.0.5/v1",
        "https://192.168.1.10/v1",
        "https://172.16.0.1/v1",
        "https://[fd00::1]/v1",
    ] {
        assert!(
            catalog_with_endpoint(endpoint).validate().is_err(),
            "expected {endpoint} to be rejected as a private/ULA address"
        );
    }
}

#[test]
fn catalog_validate_rejects_non_http_scheme_and_plain_http_to_remote() {
    assert!(
        catalog_with_endpoint("ftp://example.com/v1")
            .validate()
            .is_err()
    );
    assert!(
        catalog_with_endpoint("http://api.anthropic.com/v1")
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
    match config.validate() {
        Err(GatewayProfileError::DuplicateRouteId { id }) => assert_eq!(id, "dup"),
        other => panic!("expected DuplicateRouteId, got {other:?}"),
    }
}

#[test]
fn gateway_spec_round_trips_catalog_path() {
    let spec = GatewayConfigSpec {
        enabled: true,
        routes: vec![route("claude-*")],
        catalog: Some(GatewayCatalogSource::Path {
            path: PathBuf::from("catalog.yaml"),
        }),
        ..GatewayConfigSpec::default()
    };

    let yaml = serde_yaml::to_string(&spec).expect("serialize gateway spec");
    assert!(yaml.contains("path: catalog.yaml"), "got:\n{yaml}");
    assert!(!yaml.contains("catalog_path"), "got:\n{yaml}");

    let back: GatewayConfigSpec = serde_yaml::from_str(&yaml).expect("round-trip deserialize");
    assert!(matches!(
        back.catalog,
        Some(GatewayCatalogSource::Path { .. })
    ));
}

#[test]
fn gateway_spec_rejects_legacy_catalog_path_key() {
    let legacy = "enabled: true\nroutes: []\ncatalog_path: catalog.yaml\n";
    assert!(
        serde_yaml::from_str::<GatewayConfigSpec>(legacy).is_err(),
        "the flat catalog_path key must be rejected by deny_unknown_fields"
    );
}

#[test]
fn default_resource_audiences_cover_gateway_requirements() {
    let audiences = default_resource_audiences();
    assert!(audiences.contains(&"hook".to_owned()));
    assert!(!audiences.is_empty());
}
