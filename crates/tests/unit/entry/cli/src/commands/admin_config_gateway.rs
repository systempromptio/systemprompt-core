//! Tests for the `admin config gateway` profile mutators: enable state,
//! route upsert/remove, default provider, and registry validation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::admin::config::gateway::{
    RouteAddArgs, add_route, clear_default_provider, remove_route, set_default_provider,
    set_enabled, spec_mut, validate_gateway,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{GatewayConfigSpec, GatewayState, ProviderRegistry};
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn make_profile(services: &Path) -> Profile {
    Profile {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: "https://example.com".to_string(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: Vec::new(),
        },
        paths: PathsConfig {
            system: services.parent().unwrap().to_string_lossy().to_string(),
            services: services.to_string_lossy().to_string(),
            bin: "/tmp/test/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        providers: ProviderRegistry::default_seed().unwrap(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

fn profile() -> Profile {
    make_profile(Path::new("/tmp/test/services"))
}

fn route_args(pattern: &str, provider: &str) -> RouteAddArgs {
    RouteAddArgs {
        model_pattern: pattern.to_string(),
        provider: provider.to_string(),
        upstream_model: None,
    }
}

fn spec(profile: &Profile) -> &GatewayConfigSpec {
    match profile.gateway.as_ref().unwrap() {
        GatewayState::Spec(s) => s,
        GatewayState::Resolved(_) => panic!("gateway unexpectedly resolved"),
    }
}

#[test]
fn set_enabled_creates_spec_and_toggles_flag() {
    let mut p = profile();
    let msg = set_enabled(&mut p, true).unwrap();
    assert_eq!(msg, "Gateway enabled = true");
    assert!(spec(&p).enabled);

    let msg = set_enabled(&mut p, false).unwrap();
    assert_eq!(msg, "Gateway enabled = false");
    assert!(!spec(&p).enabled);
}

#[test]
fn spec_mut_rejects_a_resolved_gateway() {
    let mut p = profile();
    p.gateway = Some(GatewayState::Resolved(GatewayConfigSpec::default().resolve()));
    let err = spec_mut(&mut p).unwrap_err().to_string();
    assert!(err.contains("resolved state"), "unexpected error: {err}");
}

#[test]
fn add_route_mints_an_id_and_upserts_by_pattern() {
    let mut p = profile();
    let msg = add_route(&mut p, &route_args("claude-*", "anthropic")).unwrap();
    assert_eq!(msg, "Route claude-* -> anthropic added");
    let first_id = spec(&p).routes[0].id.clone();
    assert!(!first_id.as_str().is_empty());

    add_route(&mut p, &route_args("claude-*", "openai")).unwrap();
    let routes = &spec(&p).routes;
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].provider.as_str(), "openai");
    assert_ne!(routes[0].id, first_id);
}

#[test]
fn remove_route_deletes_matching_pattern_and_errors_when_absent() {
    let mut p = profile();
    add_route(&mut p, &route_args("claude-*", "anthropic")).unwrap();
    add_route(&mut p, &route_args("gpt-*", "openai")).unwrap();

    let msg = remove_route(&mut p, "claude-*").unwrap();
    assert_eq!(msg, "Route claude-* removed");
    assert_eq!(spec(&p).routes.len(), 1);
    assert_eq!(spec(&p).routes[0].model_pattern, "gpt-*");

    let err = remove_route(&mut p, "claude-*").unwrap_err().to_string();
    assert!(err.contains("No route found"), "unexpected error: {err}");
}

#[test]
fn default_provider_set_and_clear_round_trip() {
    let mut p = profile();
    let msg = set_default_provider(&mut p, "anthropic").unwrap();
    assert_eq!(msg, "Gateway default provider set to anthropic");
    assert_eq!(
        spec(&p).default_provider.as_ref().unwrap().as_str(),
        "anthropic"
    );

    let msg = clear_default_provider(&mut p).unwrap();
    assert_eq!(msg, "Gateway default provider cleared");
    assert!(spec(&p).default_provider.is_none());
}

#[test]
fn validate_gateway_passes_without_gateway_and_with_registry_providers() {
    let mut p = profile();
    validate_gateway(&p).unwrap();

    add_route(&mut p, &route_args("claude-*", "anthropic")).unwrap();
    set_default_provider(&mut p, "openai").unwrap();
    validate_gateway(&p).unwrap();
}

#[test]
fn validate_gateway_rejects_route_provider_missing_from_registry() {
    let mut p = profile();
    add_route(&mut p, &route_args("claude-*", "no-such-provider")).unwrap();
    let err = validate_gateway(&p).unwrap_err().to_string();
    assert!(
        err.contains("gateway validation failed"),
        "unexpected error: {err}"
    );
    assert!(err.contains("no-such-provider"), "unexpected error: {err}");
}

#[test]
fn validate_gateway_rejects_unknown_default_provider() {
    let mut p = profile();
    set_default_provider(&mut p, "ghost").unwrap();
    let err = validate_gateway(&p).unwrap_err().to_string();
    assert!(err.contains("ghost"), "unexpected error: {err}");
}
