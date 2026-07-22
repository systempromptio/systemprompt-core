//! Tests for deploy-target resolution preconditions.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::cloud::deploy::resolve_deploy_target;
use systemprompt_models::profile::CloudConfig;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn profile(cloud: Option<CloudConfig>) -> Profile {
    Profile {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        target: ProfileType::Cloud,
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
            system: "/tmp/test".to_string(),
            services: "/tmp/test/services".to_string(),
            bin: "/tmp/test/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![systemprompt_models::auth::JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud,
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
fn missing_cloud_config_is_rejected() {
    let err = resolve_deploy_target(&profile(None)).unwrap_err();
    assert!(err.to_string().contains("No cloud configuration"));
}

#[test]
fn missing_tenant_id_is_rejected() {
    let cloud: CloudConfig = serde_yaml::from_str("{}").unwrap();
    let err = resolve_deploy_target(&profile(Some(cloud))).unwrap_err();
    assert!(err.to_string().contains("No tenant configured"));
}
