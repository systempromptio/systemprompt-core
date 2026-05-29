//! Minimal [`AppContext`] fixture for integration tests.
//!
//! Bypasses the full
//! [`AppContextBuilder`](systemprompt_runtime::AppContextBuilder)
//! bootstrap (profile / config / logging / system-admin resolution) and
//! assembles a context directly via `AppContext::from_parts`. The fixture wires
//! in an [`AllowAllHook`](systemprompt_security::authz::AllowAllHook) so route
//! handlers behave like permissive auth.

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, PathsConfig, SecurityHeadersConfig};
use systemprompt_models::{AppPaths, Config, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_users::UserService;

use crate::user::{fixture_system_admin, fixture_user_id};

/// Minimal [`Config`] suitable for tests. Most fields point at `/tmp` or empty
/// vectors — production paths that consult them are exercised by the broader
/// service-level tests, not the fixture.
pub fn fixture_config(database_url: &str) -> Config {
    Config {
        instance_id: "fixture".to_string(),
        max_concurrent_streams: 16,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: database_url.to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: "/tmp".to_string(),
        geoip_database_path: None,
        web_path: "/tmp".to_string(),
        web_config_path: "/tmp".to_string(),
        web_metadata_path: "/tmp".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: vec![],
        allowed_resource_audiences: vec![],
        trusted_issuers: vec![],
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        use_https: false,
        rate_limits: RateLimitConfig {
            disabled: true,
            ..RateLimitConfig::default()
        },
        cors_allowed_origins: vec![],
        trusted_proxies: vec![],
        is_cloud: false,
        system_admin_username: "admin".to_string(),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
    }
}

/// Build an [`AppContext`] backed by `pool` and the fixture config.
///
/// `database_url` is folded into the config for any code that reads it back
/// out via `ctx.config().database_url`. Wires real
/// [`UserService`](systemprompt_users::UserService) and
/// [`FingerprintRepository`](systemprompt_analytics::FingerprintRepository)
/// instances so the api server's middleware stack (which hard-requires both)
/// can be assembled against the fixture context.
pub fn fixture_app_context(pool: &DbPool, database_url: &str) -> Result<Arc<AppContext>> {
    fixture_app_context_with_filter(pool, database_url, Arc::new(AllowAllFilter))
}

/// As [`fixture_app_context`] but with an injectable marketplace filter, so a
/// test can drive the real cascade-filtering path (e.g. the bridge manifest
/// E2E) instead of the permissive allow-all default.
pub fn fixture_app_context_with_filter(
    pool: &DbPool,
    database_url: &str,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths)?);

    let ctx = AppContext::from_parts(
        DataPlane {
            database: Arc::clone(pool),
            analytics_service: Arc::new(AnalyticsService::new(pool, None, None)?),
            fingerprint_repo: Some(Arc::new(FingerprintRepository::new(pool)?)),
            user_service: Some(Arc::new(UserService::new(pool)?)),
        },
        ConfigPlane {
            config: Arc::new(fixture_config(database_url)),
            app_paths,
            content_config: None,
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
        Plugins {
            extension_registry: Arc::new(ExtensionRegistry::new()),
            api_registry: Arc::new(ModuleApiRegistry::new()),
            mcp_registry: RegistryService::new(fixture_user_id()),
            marketplace_filter,
        },
        Subsystems {
            system_admin: Arc::new(fixture_system_admin("admin")),
            authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
            event_bridge: Arc::new(OnceLock::new()),
            geoip_reader: None,
        },
    );

    Ok(Arc::new(ctx))
}
