//! Exercises `StartupValidator::validate` end-to-end against a
//! synthesised `Config`. Against a boot context whose services tree does not
//! parse, the services-config loader returns an error, so the validator takes
//! its early-bail branch and emits a services-config domain error.

use std::path::PathBuf;
use std::sync::Mutex;

use systemprompt_files::FilesConfigValidator;
use systemprompt_logging::OutputState;
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_runtime::StartupValidator;
use tracing_subscriber::layer::SubscriberExt;

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

fn unloadable_boot() -> &'static systemprompt_test_fixtures::TestBootstrap {
    let env = systemprompt_test_fixtures::ensure_test_bootstrap();
    std::fs::write(
        env.services_path.join("config/config.yaml"),
        "agents: [not, a, mapping",
    )
    .expect("corrupt the services config");
    env
}

fn minimal_config() -> Config {
    Config {
        instance_id: systemprompt_manifest::config::InstanceId::new("test-instance"),
        metrics_port: None,
        max_concurrent_streams: 256,
        role: Default::default(),
        max_in_flight: None,
        shutdown: Default::default(),
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://u:p@localhost:5432/t".to_string(),
        database_write_url: None,
        database_session_url: None,
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
fn validate_returns_services_config_error_when_the_services_tree_is_unloadable_quiet() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");
    let output = OutputState::new();
    output.set_startup_mode(false);
    let _scope = tracing::subscriber::set_default(tracing_subscriber::registry().with(output));

    let cfg = minimal_config();
    let mut validator =
        StartupValidator::new(unloadable_boot().boot.clone()).expect("startup validator");
    let report = validator.validate(&cfg);
    assert!(
        report.has_errors(),
        "validator must surface services-config failure when the services tree is unloadable; \
         report = {report:?}",
    );
}

#[test]
fn validate_returns_services_config_error_when_the_services_tree_is_unloadable_verbose() {
    let _guard = STARTUP_MODE_LOCK
        .lock()
        .expect("startup-mode lock poisoned");

    let cfg = minimal_config();
    let mut validator =
        StartupValidator::new(unloadable_boot().boot.clone()).expect("startup validator");
    let report = validator.validate(&cfg);

    assert!(
        report.has_errors(),
        "verbose path must still bail with errors"
    );
    assert!(
        report.error_count() >= 1,
        "exactly the services-config error is expected: {report:?}",
    );
}

#[test]
fn files_config_validator_round_trip() {
    use systemprompt_manifest::validators::DomainConfig;

    let validator = FilesConfigValidator::new(
        systemprompt_test_fixtures::ensure_test_bootstrap()
            .app_paths
            .clone(),
    );
    assert_eq!(validator.domain_id(), "files");
    let dbg = format!("{:?}", validator);
    assert!(dbg.contains("FilesConfigValidator"));
}
