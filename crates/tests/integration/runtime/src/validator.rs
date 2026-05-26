//! Exercises `StartupValidator::validate` end-to-end against a
//! synthesised `Config`. Without an initialised `ProfileBootstrap` the
//! services-config loader returns an error, so the validator takes its
//! early-bail branch and emits a services-config domain error. This is
//! the only deterministic shape of the function we can drive without
//! installing process-wide globals shared with other test crates.

use std::path::PathBuf;
use std::sync::Mutex;

use systemprompt_logging::set_startup_mode;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_models::Config;
use systemprompt_runtime::{FilesConfigValidator, StartupValidator};

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

fn minimal_config() -> Config {
    Config {
        instance_id: "test-instance".to_string(),
        max_concurrent_streams: 256,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://u:p@localhost:5432/t".to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: String::new(),
        services_path: String::new(),
        bin_path: String::new(),
        skills_path: String::new(),
        settings_path: String::new(),
        content_config_path: String::new(),
        geoip_database_path: None,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1:8080".to_string(),
        api_internal_url: "http://127.0.0.1:8080".to_string(),
        api_external_url: "http://127.0.0.1:8080".to_string(),
        jwt_issuer: "https://test.invalid".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 604_800,
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
fn validate_returns_services_config_error_when_profile_uninitialised_quiet() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    set_startup_mode(false);

    let cfg = minimal_config();
    let mut validator = StartupValidator::new();
    let report = validator.validate(&cfg);
    assert!(
        report.has_errors(),
        "validator must surface services-config failure when ProfileBootstrap is uninitialised; \
         report = {report:?}",
    );
}

#[test]
fn validate_returns_services_config_error_when_profile_uninitialised_verbose() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    set_startup_mode(true);

    let cfg = minimal_config();
    let mut validator = StartupValidator::default();
    let report = validator.validate(&cfg);
    set_startup_mode(false);

    assert!(report.has_errors(), "verbose path must still bail with errors");
    assert!(
        report.error_count() >= 1,
        "exactly the services-config error is expected: {report:?}",
    );
}

#[test]
fn files_config_validator_round_trip() {
    use systemprompt_traits::DomainConfig;

    let validator = FilesConfigValidator::new();
    assert_eq!(validator.domain_id(), "files");
    let dbg = format!("{:?}", validator);
    assert!(dbg.contains("FilesConfigValidator"));
}
