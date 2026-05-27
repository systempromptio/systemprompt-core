//! Boot the full API server router via `setup_api_server`. Exercises the
//! per-route wiring functions (oauth/agent/mcp/stream/content/misc/extensions
//! mounts), global middleware stack assembly (analytics, jwt-context, session,
//! cors, trailing slash, trace, served-by, content-negotiation, security
//! headers, ip-ban, jti-revocation, metrics), and the discovery /
//! authenticated-discovery / wellknown / static routers.
//!
//! The prometheus recorder is process-global, so this entire file builds a
//! single ApiServer.

use std::sync::{Arc, OnceLock};

use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{AppPaths, RouteClassifier};
use systemprompt_runtime::{AppContext, AppContextParts, ModuleApiRegistry};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, fixture_db_pool, fixture_system_admin, fixture_user_id,
};
use systemprompt_users::UserService;

#[tokio::test]
async fn setup_api_server_assembles_full_router() -> anyhow::Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let pool = fixture_db_pool(&bootstrap.database_url).await?;

    let mut config = fixture_config(&bootstrap.database_url);
    // CORS layer requires at least one origin; the fixture default is empty
    // because most route-level tests bypass CORS.
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths)?);

    let parts = AppContextParts {
        config: Arc::new(config),
        database: Arc::clone(&pool),
        api_registry: Arc::new(ModuleApiRegistry::new()),
        extension_registry: Arc::new(ExtensionRegistry::new()),
        geoip_reader: None,
        content_config: None,
        route_classifier: Arc::new(RouteClassifier::new(None)),
        analytics_service: Arc::new(AnalyticsService::new(&pool, None, None)?),
        fingerprint_repo: Some(Arc::new(FingerprintRepository::new(&pool)?)),
        user_service: Some(Arc::new(UserService::new(&pool)?)),
        app_paths,
        marketplace_filter: Arc::new(AllowAllFilter),
        event_bridge: Arc::new(OnceLock::new()),
        system_admin: Arc::new(fixture_system_admin("admin")),
        mcp_registry: RegistryService::new(fixture_user_id()),
        authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
    };
    let ctx = Arc::new(AppContext::from_parts(parts));

    let server = setup_api_server(&ctx, None)
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    drop(server);
    Ok(())
}
