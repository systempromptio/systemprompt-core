//! Drives `StartupValidator::validate(&Config)` end-to-end against a
//! minimal config against a boot context whose services tree does not parse,
//! so the services load fails and the validator takes the early-return /
//! error-report branch in `load_configs`.
//!
//! Cheap but covers lines in `startup_validation/mod.rs` and
//! `startup_validation/config_loaders.rs` that the constructor-only
//! tests cannot reach.

use std::path::PathBuf;

use systemprompt_manifest::Config;
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_runtime::StartupValidator;
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn minimal_config() -> Config {
    Config {
        instance_id: systemprompt_identifiers::InstanceId::new("test-instance"),
        metrics_port: None,
        max_concurrent_streams: 16,
        role: Default::default(),
        advertise_url: None,
        max_in_flight: None,
        shutdown: Default::default(),
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://localhost/x".to_string(),
        database_write_url: None,
        database_session_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: String::new(),
        geoip_database_path: None,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1:8080".to_string(),
        api_internal_url: "http://127.0.0.1:8080".to_string(),
        api_external_url: "http://example.test".to_string(),
        jwt_issuer: "https://issuer.test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: Vec::new(),
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: 300,
        signing_key_path: PathBuf::new(),
        use_https: false,
        rate_limits: RateLimitsConfig::default(),
        retention: systemprompt_manifest::profile::RetentionConfig::default(),
        planes: Default::default(),
        cors_allowed_origins: Vec::new(),
        trusted_proxies: Vec::new(),
        is_cloud: false,
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        allow_dynamic_client_registration: true,
        api_key_max_lifetime_days: None,
        login_page_url: None,
        system_admin_username: "admin".to_string(),
        system_admin_email: None,
    }
}

#[test]
fn validate_with_an_unloadable_services_tree_short_circuits_with_services_config_error() {
    let unloadable = ensure_test_bootstrap();
    std::fs::write(
        unloadable.services_path.join("config/config.yaml"),
        "agents: [not, a, mapping",
    )
    .expect("corrupt the services config");
    let mut validator = StartupValidator::new(unloadable.boot.clone()).expect("startup validator");
    let cfg = minimal_config();
    let report = validator.validate(&cfg);

    assert!(
        report.has_errors(),
        "validation with an unloadable services tree must report errors"
    );
    assert!(
        report.domains.iter().any(|d| d.domain == "services"),
        "expected a `services` domain error, got domains: {:?}",
        report.domains.iter().map(|d| &d.domain).collect::<Vec<_>>()
    );
}
