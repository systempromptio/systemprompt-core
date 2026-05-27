//! Drives `StartupValidator::validate(&Config)` end-to-end against a
//! minimal config. In a unit-test process the profile bootstrap is not
//! initialised, so `ConfigLoader::load()` fails and the validator
//! takes the early-return / error-report branch in `load_configs`.
//!
//! Cheap but covers lines in `startup_validation/mod.rs` and
//! `startup_validation/config_loaders.rs` that the constructor-only
//! tests cannot reach.

use std::path::PathBuf;

use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_models::Config;
use systemprompt_runtime::StartupValidator;

fn minimal_config() -> Config {
    Config {
        instance_id: "test-instance".to_string(),
        max_concurrent_streams: 16,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://localhost/x".to_string(),
        database_write_url: None,
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
        jwt_issuer: "test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: Vec::new(),
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        signing_key_path: PathBuf::new(),
        use_https: false,
        rate_limits: RateLimitConfig::default(),
        cors_allowed_origins: Vec::new(),
        trusted_proxies: Vec::new(),
        is_cloud: false,
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        system_admin_username: "admin".to_string(),
    }
}

#[test]
fn validate_without_profile_short_circuits_with_services_config_error() {
    let mut validator = StartupValidator::new();
    let cfg = minimal_config();
    let report = validator.validate(&cfg);

    // Profile is not bootstrapped in this process, so `ConfigLoader::load()`
    // fails inside `load_configs` and the validator records an error on the
    // `services` domain before returning early.
    assert!(report.has_errors(), "validation without profile must report errors");
    assert!(
        report.domains.iter().any(|d| d.domain == "services"),
        "expected a `services` domain error, got domains: {:?}",
        report.domains.iter().map(|d| &d.domain).collect::<Vec<_>>()
    );
}
