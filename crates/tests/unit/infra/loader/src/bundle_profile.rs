//! A minimal in-memory profile for the bundle boot-path tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{
    BundleVerification, FetchFailurePolicy, HttpsServicesSource, ServicesProfileConfig,
    ServicesSource, default_resource_audiences,
};
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

pub fn https_source(name: &str, url: &str, keys: Vec<String>) -> ServicesSource {
    ServicesSource {
        name: name.to_owned(),
        https: Some(HttpsServicesSource {
            url: url.to_owned(),
            auth_secret: None,
            verify: BundleVerification {
                sha256: None,
                ed25519_public_keys: keys,
            },
        }),
        oci: None,
    }
}

pub fn profile(
    services_root: &Path,
    cache_dir: &Path,
    sources: Vec<ServicesSource>,
    on_fetch_failure: FetchFailurePolicy,
) -> Profile {
    Profile {
        storage: Default::default(),
        name: "p".to_owned(),
        display_name: "Profile P".to_owned(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Site".to_owned(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_owned(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_owned(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_owned(),
            api_internal_url: "http://localhost:8080".to_owned(),
            api_external_url: "https://example.com".to_owned(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            metrics_port: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: vec!["fc00::/7".parse().expect("cidr")],
        },
        paths: PathsConfig {
            system: "/tmp/system".to_owned(),
            services: services_root.to_string_lossy().into_owned(),
            bin: "/tmp/bin".to_owned(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_owned(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: default_resource_audiences(),
            allow_registration: true,
            login_page_url: None,
            signing_key_path: std::path::PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        governance: None,
        services: ServicesProfileConfig {
            port_offset: 0,
            sources,
            cache_dir: Some(cache_dir.to_string_lossy().into_owned()),
            on_fetch_failure,
        },
        system_admin: SystemAdminConfig {
            username: "admin".to_owned(),
            email: None,
        },
    }
}
