//! Minimal [`AppContext`] fixture for integration tests.
//!
//! Bypasses the full
//! [`AppContextBuilder`](systemprompt_runtime::AppContextBuilder)
//! bootstrap (profile / config / logging / system-admin resolution) and
//! assembles a context directly via `AppContext::from_parts`. The fixture wires
//! in an [`AllowAllHook`] so route handlers behave like permissive auth.

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceCache, MarketplaceFilter};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, PathsConfig, SecurityHeadersConfig};
use systemprompt_models::{Config, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink, SharedAuthzHook};
use systemprompt_users::UserService;

use crate::user::{fixture_system_admin, fixture_user_id};

pub fn fixture_analytics_repositories(
    db: &DbPool,
) -> Result<systemprompt_analytics::repository::AnalyticsRepositories> {
    Ok(
        systemprompt_analytics::repository::AnalyticsRepositories::new(
            db,
            Arc::new(systemprompt_users::sessions::SessionRepository::new(db)?),
            Arc::new(systemprompt_logging::AnalyticsRepository::new(db)?),
            Arc::new(systemprompt_content::repository::ContentRepository::new(
                db,
            )?),
        )?,
    )
}

pub fn fixture_fingerprint_repository(db: &DbPool) -> Result<FingerprintRepository> {
    Ok(FingerprintRepository::new(
        db,
        Arc::new(systemprompt_users::sessions::SessionRepository::new(db)?),
    )?)
}

pub async fn refresh_reporting(db: &DbPool) -> Result<()> {
    systemprompt_runtime::reporting::rebuild(db).await?;
    Ok(())
}

/// Deliver captured reporting evidence through the production projector.
/// Call only for an owned fixture database, with its source writers quiescent.
pub async fn drain_reporting(db: &DbPool) -> Result<usize> {
    systemprompt_runtime::reporting::initialize(db).await?;
    let mut total = 0;
    for _ in 0..100 {
        let processed = systemprompt_runtime::reporting::process_pending(db, 100).await?;
        total += processed;
        if systemprompt_runtime::reporting::status(db)
            .await?
            .pending_count
            == 0
        {
            return Ok(total);
        }
    }
    anyhow::bail!("Reporting fixture evidence did not drain within 100 bounded batches")
}

pub fn fixture_config(database_url: &str) -> Config {
    Config {
        instance_id: "fixture".to_string(),
        metrics_port: None,
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
        jwt_issuer: "https://issuer.test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: systemprompt_models::auth::JwtAudience::standard(),
        allowed_resource_audiences: vec![],
        trusted_issuers: vec![],
        id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        use_https: false,
        rate_limits: RateLimitConfig {
            disabled: true,
            ..RateLimitConfig::default()
        },
        cors_allowed_origins: vec!["http://localhost:3000".to_string()],
        trusted_proxies: vec![],
        is_cloud: false,
        system_admin_username: "admin".to_string(),
        system_admin_email: Some(systemprompt_identifiers::Email::local_admin()),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        allow_dynamic_client_registration: true,
        login_page_url: None,
    }
}

pub fn fixture_app_context(pool: &DbPool, database_url: &str) -> Result<Arc<AppContext>> {
    fixture_app_context_with_filter(pool, database_url, Arc::new(AllowAllFilter))
}

fn tmp_paths() -> PathsConfig {
    PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    }
}

pub fn fixture_app_context_with_filter(
    pool: &DbPool,
    database_url: &str,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    fixture_app_context_with(pool, database_url, tmp_paths(), marketplace_filter)
}

pub fn fixture_app_context_with(
    pool: &DbPool,
    database_url: &str,
    paths: PathsConfig,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    fixture_app_context_full(pool, database_url, paths, marketplace_filter, hook)
}

pub fn fixture_app_context_with_config(pool: &DbPool, config: Config) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool)?);
    fixture_app_context_assembled(
        pool,
        config,
        tmp_paths(),
        FixtureSeams {
            marketplace_filter: Arc::new(AllowAllFilter),
            authz_hook: hook,
            user_repository,
        },
    )
}

// The collaborators a test may swap out; everything else is built from the
// pool.
struct FixtureSeams {
    marketplace_filter: Arc<dyn MarketplaceFilter>,
    authz_hook: SharedAuthzHook,
    user_repository: Arc<systemprompt_users::UserRepository>,
}

// Build a fixture context whose user repository is supplied by the test —
// used to drive the user-data read failures of a route while every other
// repository stays healthy.
pub fn fixture_app_context_with_user_repository(
    pool: &DbPool,
    database_url: &str,
    paths: PathsConfig,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
    user_repository: Arc<systemprompt_users::UserRepository>,
) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    fixture_app_context_assembled(
        pool,
        fixture_config(database_url),
        paths,
        FixtureSeams {
            marketplace_filter,
            authz_hook: hook,
            user_repository,
        },
    )
}

// Build a fixture context with an explicit authorization hook — used by tests
// that need to drive the `Deny` branch (pass a
// [`DenyAllHook`](systemprompt_security::authz::DenyAllHook)).
pub fn fixture_app_context_with_hook(
    pool: &DbPool,
    database_url: &str,
    authz_hook: SharedAuthzHook,
) -> Result<Arc<AppContext>> {
    fixture_app_context_full(
        pool,
        database_url,
        tmp_paths(),
        Arc::new(AllowAllFilter),
        authz_hook,
    )
}

fn fixture_app_context_full(
    pool: &DbPool,
    database_url: &str,
    paths: PathsConfig,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
    authz_hook: SharedAuthzHook,
) -> Result<Arc<AppContext>> {
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool)?);
    fixture_app_context_assembled(
        pool,
        fixture_config(database_url),
        paths,
        FixtureSeams {
            marketplace_filter,
            authz_hook,
            user_repository,
        },
    )
}

fn fixture_app_context_assembled(
    pool: &DbPool,
    config: Config,
    paths: PathsConfig,
    seams: FixtureSeams,
) -> Result<Arc<AppContext>> {
    let FixtureSeams {
        marketplace_filter,
        authz_hook,
        user_repository,
    } = seams;
    let governance = Arc::new(
        systemprompt_security::policy::GovernanceEngine::from_services_root(std::path::Path::new(
            &paths.services,
        ))?,
    );
    let app_paths = Arc::new(AppPaths::from_profile(
        &paths,
        systemprompt_models::PathResolution::Canonicalize,
        None,
    )?);

    let analytics_repositories = Arc::new(fixture_analytics_repositories(pool)?);
    let analytics_service = Arc::new(AnalyticsService::new(None, None, &analytics_repositories));
    let session_usage: systemprompt_traits::DynSessionUsageCounters =
        analytics_service.session_repo().owner();
    let file_storage = systemprompt_storage::build_file_storage(
        systemprompt_models::profile::StorageBackend::Local,
        app_paths.storage().root(),
    );
    let sqlx_pool = pool.pool_arc()?.as_ref().clone();
    let ctx = AppContext::from_parts(
        DataPlane {
            database: Arc::clone(pool),
            analytics_service,
            fingerprint_repo: Some(Arc::new(fixture_fingerprint_repository(pool)?)),
            user_service: Some(Arc::new(UserService::new(Arc::clone(&user_repository)))),
            a2a_repositories: Arc::new(systemprompt_agent::repository::A2ARepositories::new(
                pool,
                systemprompt_agent::repository::A2aDependencies {
                    session_usage,
                    instance_id: systemprompt_identifiers::InstanceId::new("test-instance"),
                    managed_skills: crate::agent::not_managed_skills(),
                    tool_executions: crate::agent::tool_execution_ledger(
                        crate::agent::ToolExecutionLedger::Exists,
                    ),
                },
            )?),
            content_repositories: Arc::new(
                systemprompt_content::repository::ContentRepositories::new(pool)?,
            ),
            oauth_repositories: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
                pool,
            )?),
            user_repository,
            service_repository: Arc::new(systemprompt_database::ServiceRepository::new(
                pool,
                systemprompt_identifiers::InstanceId::new("test-instance"),
            )?),
            ai_repositories: Arc::new(systemprompt_ai::repository::AiRepositories::new(pool)?),
            analytics_repositories,
            file_repository: Arc::new(systemprompt_files::FileRepository::new(pool)?),
            mcp_session_repository: Arc::new(
                systemprompt_mcp::repository::McpSessionRepository::new(pool)?,
            ),
            feedback_snapshots_repository: Arc::new(
                systemprompt_analytics::snapshots::FeedbackSnapshotsRepository::new(
                    sqlx_pool.clone(),
                    systemprompt_analytics::feedback::FeedbackFactsRepository::new(
                        sqlx_pool.clone(),
                    ),
                ),
            ),
            feedback_facts_repository: Arc::new(
                systemprompt_analytics::feedback::FeedbackFactsRepository::new(sqlx_pool.clone()),
            ),
            managed_repository: Arc::new(
                systemprompt_marketplace::managed::ManagedRepository::new(pool)?,
            ),
        },
        ConfigPlane {
            config: Arc::new(config),
            app_paths,
            content_config: None,
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
        Plugins {
            extension_registry: Arc::new(ExtensionRegistry::new()),
            api_registry: Arc::new(ModuleApiRegistry::new()),
            mcp_registry: RegistryService::new(fixture_user_id()),
            marketplace_filter,
            marketplace_cache: Arc::new(MarketplaceCache::default()),
        },
        Subsystems {
            system_admin: Arc::new(fixture_system_admin("admin")),
            authz_hook,
            governance,
            ai_service: None,
            schema_install: Arc::new(systemprompt_database::SchemaInstallReport::default()),
            event_bridge: Arc::new(OnceLock::new()),
            geoip_reader: None,
            file_storage,
            shutdown: Default::default(),
            publish_guard: Arc::new(tokio::sync::Mutex::new(
                systemprompt_marketplace::inventory::PublishGuard::default(),
            )),
            snapshot_wakeup: Arc::new(systemprompt_runtime::reporting::SnapshotWakeup::default()),
        },
    );

    Ok(Arc::new(ctx))
}

// The vendor-neutral warn-only chain: what a deployment without a
// `<services>/governance/config.yaml` boots with.
pub fn default_governance_engine() -> Arc<systemprompt_security::policy::GovernanceEngine> {
    Arc::new(
        systemprompt_security::policy::GovernanceEngine::from_config(
            &systemprompt_security::policy::GovernanceConfig::defaults(),
        )
        .expect("the default governance chain always builds"),
    )
}
