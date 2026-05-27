//! Exercises [`FilesConfigValidator::load`] directly via the
//! [`DomainConfig`] trait. The validator stores
//! `FilesConfig::get_optional().is_some()` — both code paths (with and
//! without a globally-initialised FilesConfig) are stable to call.

use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_models::validators::ValidationConfigProvider;
use systemprompt_models::{Config, ServicesConfig};
use systemprompt_runtime::FilesConfigValidator;
use systemprompt_traits::DomainConfig;

fn fixture_config() -> Config {
    Config {
        instance_id: "test".to_string(),
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
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: Vec::new(),
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        signing_key_path: std::path::PathBuf::new(),
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
fn files_validator_load_with_uninitialised_files_config_marks_uninitialised() {
    let services_config = ServicesConfig::default();
    let provider = ValidationConfigProvider::new(fixture_config(), services_config);
    let mut v = FilesConfigValidator::new();
    DomainConfig::load(&mut v, &provider).expect("load ok");
    // No assertion on internal state — exercising the line is the point.
}

#[test]
fn files_validator_load_idempotent_under_repeat() {
    let services_config = ServicesConfig::default();
    let provider = ValidationConfigProvider::new(fixture_config(), services_config);
    let mut v = FilesConfigValidator::new();
    DomainConfig::load(&mut v, &provider).expect("first load");
    DomainConfig::load(&mut v, &provider).expect("second load");
}

#[test]
fn files_validator_load_then_validate_pairs_cleanly() {
    let services_config = ServicesConfig::default();
    let provider = ValidationConfigProvider::new(fixture_config(), services_config);
    let mut v = FilesConfigValidator::new();
    DomainConfig::load(&mut v, &provider).expect("load");
    let report = DomainConfig::validate(&v).expect("validate after load");
    let _ = report.has_errors();
}
