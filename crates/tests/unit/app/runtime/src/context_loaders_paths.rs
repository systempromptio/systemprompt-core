//! Loader arms of `AppContext::load_geoip_database` /
//! `AppContext::load_content_config` not reachable from the existing tests:
//! a valid MaxMind database (Some arm) and a content-config path that exists
//! but cannot be read (it is a directory).

use std::path::PathBuf;

use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, PathsConfig, SecurityHeadersConfig};
use systemprompt_models::{AppPaths, Config};
use systemprompt_runtime::AppContext;

fn fixture_config(geoip: Option<String>) -> Config {
    Config {
        instance_id: "t".to_string(),
        max_concurrent_streams: 16,
        sitename: "t".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://x/y".to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: String::new(),
        geoip_database_path: geoip,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://base.example".to_string(),
        jwt_issuer: "https://issuer.test".to_string(),
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

fn app_paths_for(system: &std::path::Path) -> AppPaths {
    let paths = PathsConfig {
        system: system.display().to_string(),
        services: system.join("services").display().to_string(),
        bin: system.join("bin").display().to_string(),
        web_path: Some(system.join("web").display().to_string()),
        storage: Some(system.join("storage").display().to_string()),
        geoip_database: None,
    };
    AppPaths::from_profile(&paths).expect("app paths")
}

fn valid_mmdb_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../domain/analytics/fixtures/GeoIP2-City-Test.mmdb")
        .display()
        .to_string()
}

#[test]
fn load_geoip_database_returns_reader_for_valid_mmdb() {
    let cfg = fixture_config(Some(valid_mmdb_path()));
    let reader = AppContext::load_geoip_database(&cfg, false);
    assert!(reader.is_some(), "a valid .mmdb must produce a reader");
}

#[test]
fn load_geoip_database_returns_none_for_invalid_mmdb_with_warnings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bogus = tmp.path().join("not-a-database.mmdb");
    std::fs::write(&bogus, b"this is not a maxmind database").expect("write bogus mmdb");
    let cfg = fixture_config(Some(bogus.display().to_string()));
    let reader = AppContext::load_geoip_database(&cfg, true);
    assert!(reader.is_none(), "an invalid .mmdb must be rejected");
}

#[test]
fn load_content_config_returns_none_when_path_is_unreadable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // content_config path resolves to <system>/services/content/config.yaml;
    // make it a directory so exists() passes but read_to_string fails.
    std::fs::create_dir_all(tmp.path().join("services/content/config.yaml"))
        .expect("mkdir config.yaml directory");
    let app_paths = app_paths_for(tmp.path());
    let cfg = fixture_config(None);

    let loaded = AppContext::load_content_config(&cfg, &app_paths);
    assert!(
        loaded.is_none(),
        "an unreadable content config degrades to None"
    );
}

#[test]
fn load_content_config_returns_none_when_file_is_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app_paths = app_paths_for(tmp.path());
    let cfg = fixture_config(None);

    let loaded = AppContext::load_content_config(&cfg, &app_paths);
    assert!(
        loaded.is_none(),
        "a missing content config degrades to None"
    );
}

#[test]
fn load_content_config_returns_none_for_malformed_yaml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("services/content")).expect("mkdir content dir");
    std::fs::write(
        tmp.path().join("services/content/config.yaml"),
        ": : not yaml [",
    )
    .expect("write malformed content config");
    let app_paths = app_paths_for(tmp.path());
    let cfg = fixture_config(None);

    let loaded = AppContext::load_content_config(&cfg, &app_paths);
    assert!(
        loaded.is_none(),
        "a malformed content config degrades to None"
    );
}

#[test]
fn load_content_config_rewrites_organization_urls() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("services/content")).expect("mkdir content dir");
    std::fs::write(
        tmp.path().join("services/content/config.yaml"),
        "metadata:\n  structured_data:\n    organization:\n      logo: /images/logo.svg\n",
    )
    .expect("write content config");
    let app_paths = app_paths_for(tmp.path());
    let cfg = fixture_config(None);

    let loaded =
        AppContext::load_content_config(&cfg, &app_paths).expect("valid content config loads");
    assert_eq!(
        loaded.metadata.structured_data.organization.url,
        "http://base.example"
    );
    assert_eq!(
        loaded.metadata.structured_data.organization.logo,
        "http://base.example/images/logo.svg"
    );
}
