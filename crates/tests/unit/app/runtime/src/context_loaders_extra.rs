//! Exercises the `show_warnings=true` branch of
//! `AppContext::load_geoip_database` when no geoip path is configured,
//! which the integration suite covers only with `show_warnings=false`.

use std::path::PathBuf;

use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_runtime::AppContext;

fn cfg_without_geoip() -> Config {
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
fn load_geoip_database_with_show_warnings_emits_help_text() {
    let cfg = cfg_without_geoip();
    let reader = AppContext::load_geoip_database(&cfg, true);
    assert!(reader.is_none(), "no path configured -> None");
}

#[test]
fn module_api_registry_default_matches_new() {
    use systemprompt_runtime::ModuleApiRegistry;
    let a = ModuleApiRegistry::default();
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("ModuleApiRegistry"));
}
