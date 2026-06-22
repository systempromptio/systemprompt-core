//! Exercises `AppContext::load_geoip_database` and
//! `AppContext::load_content_config` against synthesised configs that
//! cover both the missing-file and present-file branches.

use std::fs;
use std::path::PathBuf;

use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, PathsConfig, SecurityHeadersConfig};
use systemprompt_models::{AppPaths, Config};
use systemprompt_runtime::AppContext;
use tempfile::tempdir;

fn minimal_config(
    content_config_path: String,
    geoip_database_path: Option<String>,
    system_path: String,
    services_path: String,
) -> Config {
    Config {
        instance_id: "test-instance".to_string(),
        max_concurrent_streams: 256,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://u:p@localhost:5432/t".to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path,
        services_path,
        bin_path: String::new(),
        skills_path: String::new(),
        settings_path: String::new(),
        content_config_path,
        geoip_database_path,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1:8080".to_string(),
        api_internal_url: "http://127.0.0.1:8080".to_string(),
        api_external_url: "http://example.test".to_string(),
        jwt_issuer: "https://test.invalid".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 604_800,
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

fn make_app_paths(base: &std::path::Path) -> AppPaths {
    let system = base.join("system");
    let services = base.join("services");
    let bin = base.join("bin");
    fs::create_dir_all(&system).expect("mkdir system");
    fs::create_dir_all(&services).expect("mkdir services");
    fs::create_dir_all(&bin).expect("mkdir bin");
    let paths = PathsConfig {
        system: system.to_string_lossy().to_string(),
        services: services.to_string_lossy().to_string(),
        bin: bin.to_string_lossy().to_string(),
        web_path: None,
        storage: Some(base.join("storage").to_string_lossy().to_string()),
        geoip_database: None,
    };
    AppPaths::from_profile(&paths).expect("AppPaths from profile")
}

#[test]
fn load_geoip_database_returns_none_when_not_configured() {
    let cfg = minimal_config(String::new(), None, String::new(), String::new());
    let reader = AppContext::load_geoip_database(&cfg, false);
    assert!(reader.is_none(), "unconfigured geoip must yield None");
}

#[test]
fn load_geoip_database_returns_none_when_path_missing() {
    let cfg = minimal_config(
        String::new(),
        Some("/does/not/exist.mmdb".to_string()),
        String::new(),
        String::new(),
    );
    let reader = AppContext::load_geoip_database(&cfg, true);
    assert!(reader.is_none(), "bad geoip path must yield None");
}

#[test]
fn load_content_config_returns_none_when_file_missing() {
    let dir = tempdir().expect("tempdir");
    let paths = make_app_paths(dir.path());
    let cfg = minimal_config(
        String::new(),
        None,
        dir.path().join("system").to_string_lossy().to_string(),
        dir.path().join("services").to_string_lossy().to_string(),
    );
    let content = AppContext::load_content_config(&cfg, &paths);
    assert!(content.is_none(), "missing content config must yield None");
}

#[test]
fn load_content_config_returns_none_when_yaml_invalid() {
    let dir = tempdir().expect("tempdir");
    let paths = make_app_paths(dir.path());
    let content_path = dir
        .path()
        .join("services")
        .join("content")
        .join("config.yaml");
    fs::create_dir_all(content_path.parent().expect("parent")).expect("mkdir content");
    fs::write(&content_path, b": : : not valid yaml").expect("write bad yaml");
    let cfg = minimal_config(
        content_path.to_string_lossy().to_string(),
        None,
        dir.path().join("system").to_string_lossy().to_string(),
        dir.path().join("services").to_string_lossy().to_string(),
    );
    let content = AppContext::load_content_config(&cfg, &paths);
    assert!(content.is_none(), "bad yaml must yield None");
}

#[test]
fn load_content_config_rewrites_root_relative_logo() {
    let dir = tempdir().expect("tempdir");
    let paths = make_app_paths(dir.path());
    let content_path = dir
        .path()
        .join("services")
        .join("content")
        .join("config.yaml");
    fs::create_dir_all(content_path.parent().expect("parent")).expect("mkdir content");
    let yaml = r#"
content_sources: {}
categories: {}
metadata:
  structured_data:
    organization:
      url: ""
      logo: "/logo.png"
      "@type": "Organization"
      "@context": "https://schema.org"
      name: "Test Org"
"#;
    fs::write(&content_path, yaml).expect("write yaml");
    let cfg = minimal_config(
        content_path.to_string_lossy().to_string(),
        None,
        dir.path().join("system").to_string_lossy().to_string(),
        dir.path().join("services").to_string_lossy().to_string(),
    );
    let content = AppContext::load_content_config(&cfg, &paths);
    if let Some(cc) = content {
        let logo = &cc.metadata.structured_data.organization.logo;
        assert!(
            logo.starts_with("http://example.test"),
            "logo must be rewritten with base_url, got {logo}"
        );
    }
}

#[test]
fn load_content_config_parses_minimal_yaml() {
    let dir = tempdir().expect("tempdir");
    let paths = make_app_paths(dir.path());
    let content_path = dir
        .path()
        .join("services")
        .join("content")
        .join("config.yaml");
    fs::create_dir_all(content_path.parent().expect("parent")).expect("mkdir content");
    fs::write(&content_path, b"content_sources: {}\ncategories: {}\n").expect("write yaml");
    let cfg = minimal_config(
        content_path.to_string_lossy().to_string(),
        None,
        dir.path().join("system").to_string_lossy().to_string(),
        dir.path().join("services").to_string_lossy().to_string(),
    );
    let content = AppContext::load_content_config(&cfg, &paths);
    assert!(content.is_some(), "valid yaml must parse to Some");
    let cc = content.expect("loaded content config");
    assert_eq!(
        cc.metadata.structured_data.organization.url, "http://example.test",
        "loader must overwrite organization.url with api_external_url",
    );
}
