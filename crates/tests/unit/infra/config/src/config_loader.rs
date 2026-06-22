#![allow(clippy::all)]

//! Unit tests for `systemprompt_config::validate_database_config` — the
//! database-type and Postgres-URL guards applied to a built `Config`.

use std::path::PathBuf;

use systemprompt_config::{ConfigError, validate_database_config};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};

fn base_config() -> Config {
    Config {
        instance_id: "t".to_string(),
        max_concurrent_streams: 16,
        sitename: "t".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://user:pass@localhost/db".to_string(),
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
        jwt_issuer: "t".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: Vec::new(),
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: 300,
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
fn validate_database_config_accepts_postgres() {
    let cfg = base_config();
    validate_database_config(&cfg).expect("postgres type with valid url should pass");
}

#[test]
fn validate_database_config_accepts_postgresql_alias() {
    let mut cfg = base_config();
    cfg.database_type = "postgresql".to_string();
    cfg.database_url = "postgresql://user:pass@localhost/db".to_string();
    validate_database_config(&cfg).expect("postgresql alias should pass");
}

#[test]
fn validate_database_config_is_case_insensitive_on_type() {
    let mut cfg = base_config();
    cfg.database_type = "PostgreSQL".to_string();
    validate_database_config(&cfg).expect("mixed-case database type should pass");
}

#[test]
fn validate_database_config_rejects_unsupported_type() {
    let mut cfg = base_config();
    cfg.database_type = "mysql".to_string();

    let err = validate_database_config(&cfg).unwrap_err();
    match err {
        ConfigError::UnsupportedDatabaseType { db_type } => {
            assert_eq!(db_type, "mysql");
        },
        other => panic!("expected UnsupportedDatabaseType, got: {other:?}"),
    }
}

#[test]
fn validate_database_config_rejects_non_postgres_url() {
    let mut cfg = base_config();
    cfg.database_url = "mysql://user:pass@localhost/db".to_string();

    let err = validate_database_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidDatabaseUrl { .. }),
        "expected InvalidDatabaseUrl, got: {err:?}"
    );
}

#[test]
fn validate_database_config_validates_write_url_when_present() {
    let mut cfg = base_config();
    cfg.database_write_url = Some("not-a-postgres-url".to_string());

    let err = validate_database_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidDatabaseUrl { .. }),
        "expected InvalidDatabaseUrl from bad write url, got: {err:?}"
    );
}

#[test]
fn validate_database_config_accepts_valid_write_url() {
    let mut cfg = base_config();
    cfg.database_write_url = Some("postgres://user:pass@write-host/db".to_string());

    validate_database_config(&cfg).expect("valid write url should pass");
}
