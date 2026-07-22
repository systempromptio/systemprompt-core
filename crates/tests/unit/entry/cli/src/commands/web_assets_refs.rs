//! Tests for `web assets show` config-reference detection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_cli::web::assets::show::find_config_references;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn profile_with_services(services: &Path) -> Profile {
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
        providers: systemprompt_models::profile::ProviderRegistry::default(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

#[test]
fn missing_config_files_yield_no_references() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = profile_with_services(&tmp.path().join("services"));

    assert!(find_config_references("logo.png", &profile).is_empty());
}

#[test]
fn references_found_in_web_config_and_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let services = tmp.path().join("services");
    let web_dir = services.join("web");
    fs::create_dir_all(&web_dir).unwrap();
    fs::write(web_dir.join("config.yaml"), "logo: /assets/logo.png\n").unwrap();
    fs::write(web_dir.join("metadata.yaml"), "og_image: assets/logo.png\n").unwrap();

    let profile = profile_with_services(&services);
    let refs = find_config_references("logo.png", &profile);

    assert_eq!(refs.len(), 2);
    assert!(refs[0].starts_with("web config: "));
    assert!(refs[1].starts_with("metadata: "));
}

#[test]
fn unreferenced_asset_reports_nothing_even_with_configs_present() {
    let tmp = tempfile::tempdir().unwrap();
    let services = tmp.path().join("services");
    let web_dir = services.join("web");
    fs::create_dir_all(&web_dir).unwrap();
    fs::write(web_dir.join("config.yaml"), "logo: /assets/other.png\n").unwrap();
    fs::write(web_dir.join("metadata.yaml"), "no assets here\n").unwrap();

    let profile = profile_with_services(&services);
    assert!(find_config_references("logo.png", &profile).is_empty());
}
