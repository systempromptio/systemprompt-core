//! Minimal [`AppContext`] fixture for integration tests.
//!
//! Bypasses the full
//! [`AppContextBuilder`](systemprompt_runtime::AppContextBuilder)
//! bootstrap (profile / config / logging / system-admin resolution) and
//! assembles a context directly via `AppContext::from_parts`. The fixture wires
//! in an [`AllowAllHook`] so route handlers behave like permissive auth.
//!
//! [`test_app_context`] panics when the context cannot be assembled: a test
//! whose context is missing fails rather than skips (see [`crate::db`]).

use std::sync::Arc;

use anyhow::Result;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_governance::authz::{AllowAllHook, NullAuditSink, SharedAuthzHook};
use systemprompt_loader::BootContext;
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, PathsConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_manifest::Config;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceCache, MarketplaceFilter};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::RouteClassifier;
use systemprompt_runtime::{AppContext, ConfigPlane, DataPlane, Plugins, Subsystems};
use systemprompt_users::UserService;

use crate::user::{fixture_system_admin, fixture_user_id};

pub const FIXTURE_JWT_ISSUER: &str = "https://issuer.test";

pub fn fixture_analytics_repositories(
    db: &DbPool,
) -> Result<systemprompt_analytics::repository::AnalyticsRepositories> {
    Ok(
        systemprompt_analytics::repository::AnalyticsRepositories::new(
            db,
            Arc::new(systemprompt_users::SessionRepository::new(db)),
            Arc::new(systemprompt_logging::AnalyticsRepository::new(db)),
            Arc::new(systemprompt_content::repository::ContentRepository::new(db)),
        ),
    )
}

pub fn fixture_fingerprint_repository(db: &DbPool) -> Result<FingerprintRepository> {
    Ok(FingerprintRepository::new(
        db,
        Arc::new(systemprompt_users::SessionRepository::new(db)),
    ))
}

pub fn fixture_config(database_url: &str) -> Config {
    Config {
        instance_id: crate::unique_instance(),
        metrics_port: None,
        max_concurrent_streams: 16,
        role: Default::default(),
        max_in_flight: None,
        shutdown: Default::default(),
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: database_url.to_string(),
        database_write_url: None,
        database_session_url: None,
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
        jwt_issuer: FIXTURE_JWT_ISSUER.to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: systemprompt_models::auth::JwtAudience::standard(),
        allowed_resource_audiences: vec![],
        trusted_issuers: vec![],
        id_jag_ttl_secs: systemprompt_manifest::profile::DEFAULT_ID_JAG_TTL_SECS,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        use_https: false,
        rate_limits: RateLimitsConfig {
            disabled: true,
            ..RateLimitsConfig::default()
        },
        retention: systemprompt_manifest::profile::RetentionConfig::default(),
        planes: Default::default(),
        cors_allowed_origins: vec!["http://localhost:3000".to_string()],
        trusted_proxies: vec![],
        is_cloud: false,
        system_admin_username: "admin".to_string(),
        system_admin_email: Some(systemprompt_identifiers::Email::local_admin()),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        allow_dynamic_client_registration: true,
        api_key_max_lifetime_days: None,
        login_page_url: None,
    }
}

pub fn test_config() -> Arc<Config> {
    test_config_with(|_| {})
}

pub fn test_config_with(customize: impl FnOnce(&mut Config)) -> Arc<Config> {
    let mut config = fixture_config("postgres://x");
    config.allowed_resource_audiences = vec!["hook".to_owned()];
    customize(&mut config);
    Arc::new(config)
}

pub fn test_app_context(pool: &DbPool, database_url: &str) -> Arc<AppContext> {
    fixture_app_context_with_filter(pool, database_url, Arc::new(AllowAllFilter))
        .unwrap_or_else(|e| panic!("fixture AppContext over the test database: {e:#}"))
}

fn tmp_paths() -> PathsConfig {
    PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
        shipped_files: None,
        data: None,
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
    fixture_app_context_with_extensions(pool, config, ExtensionRegistry::new())
}

pub fn fixture_app_context_with_extensions(
    pool: &DbPool,
    config: Config,
    extension_registry: ExtensionRegistry,
) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool));
    fixture_app_context_assembled(
        pool,
        config,
        tmp_paths(),
        FixtureSeams {
            marketplace_filter: Arc::new(AllowAllFilter),
            authz_hook: hook,
            user_repository,
            extension_registry: Arc::new(extension_registry),
            services: None,
        },
    )
}

// The collaborators a test may swap out; everything else is built from the
// pool.
struct FixtureSeams {
    marketplace_filter: Arc<dyn MarketplaceFilter>,
    authz_hook: SharedAuthzHook,
    user_repository: Arc<systemprompt_users::UserRepository>,
    extension_registry: Arc<ExtensionRegistry>,
    services: Option<systemprompt_loader::ServicesState>,
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
            extension_registry: Arc::new(ExtensionRegistry::new()),
            services: None,
        },
    )
}

// Build a fixture context with an explicit authorization hook — used by tests
// that need to drive the `Deny` branch (pass a
// [`DenyAllHook`](systemprompt_governance::authz::DenyAllHook)).
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
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool));
    fixture_app_context_assembled(
        pool,
        fixture_config(database_url),
        paths,
        FixtureSeams {
            marketplace_filter,
            authz_hook,
            user_repository,
            extension_registry: Arc::new(ExtensionRegistry::new()),
            services: None,
        },
    )
}

fn fixture_app_context_assembled(
    pool: &DbPool,
    config: Config,
    paths: PathsConfig,
    seams: FixtureSeams,
) -> Result<Arc<AppContext>> {
    fixture_app_context_assembled_on(
        crate::bootstrap::ensure_test_bootstrap(),
        pool,
        config,
        paths,
        seams,
    )
}

pub fn test_app_context_on(
    bootstrap: &crate::bootstrap::TestBootstrap,
    pool: &DbPool,
) -> Arc<AppContext> {
    fixture_app_context_on_bootstrap_with_paths(
        bootstrap,
        pool,
        bootstrap.boot.profile().paths.clone(),
        Arc::new(AllowAllFilter),
    )
    .unwrap_or_else(|e| panic!("fixture AppContext on the supplied bootstrap: {e:#}"))
}

pub fn fixture_app_context_on_bootstrap(
    bootstrap: &crate::bootstrap::TestBootstrap,
    pool: &DbPool,
    config: Config,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool));
    fixture_app_context_assembled_on(
        bootstrap,
        pool,
        config,
        bootstrap.boot.profile().paths.clone(),
        FixtureSeams {
            marketplace_filter,
            authz_hook: hook,
            user_repository,
            extension_registry: Arc::new(ExtensionRegistry::new()),
            services: None,
        },
    )
}

pub fn fixture_app_context_on_bootstrap_with_paths(
    bootstrap: &crate::bootstrap::TestBootstrap,
    pool: &DbPool,
    paths: PathsConfig,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    let hook = Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let user_repository = Arc::new(systemprompt_users::UserRepository::new(pool));
    let services = systemprompt_loader::ServicesState::load(&bootstrap.boot)
        .unwrap_or_else(|_| bootstrap.services.clone());
    let mut config = fixture_config(&bootstrap.database_url);
    config.system_path = paths.system.clone();
    config.services_path = paths.services.clone();
    config.bin_path = paths.bin.clone();
    fixture_app_context_assembled_on(
        bootstrap,
        pool,
        config,
        paths,
        FixtureSeams {
            marketplace_filter,
            authz_hook: hook,
            user_repository,
            extension_registry: Arc::new(ExtensionRegistry::new()),
            services: Some(services),
        },
    )
}

fn fixture_app_context_assembled_on(
    bootstrap: &crate::bootstrap::TestBootstrap,
    pool: &DbPool,
    config: Config,
    paths: PathsConfig,
    seams: FixtureSeams,
) -> Result<Arc<AppContext>> {
    let FixtureSeams {
        marketplace_filter,
        authz_hook,
        user_repository,
        extension_registry,
        services,
    } = seams;
    let services = services.unwrap_or_else(|| bootstrap.services.clone());
    let governance = systemprompt_traits::ConfigWatch::new(
        systemprompt_governance::policy::GovernanceEngine::from_config(
            &systemprompt_governance::policy::GovernanceConfig::defaults(),
            Some(systemprompt_governance::policy::RateWindowRepository::new(
                pool.write_pool().as_ref().clone(),
            )),
        )?,
    );
    let app_paths = Arc::new(AppPaths::from_profile(
        &paths,
        systemprompt_manifest::PathResolution::Canonicalize,
        None,
    )?);

    let analytics_repositories = Arc::new(fixture_analytics_repositories(pool)?);
    let analytics_service = Arc::new(AnalyticsService::new(None, None, &analytics_repositories));
    let session_store = Arc::clone(analytics_service.session_store());
    let session_usage: systemprompt_traits::DynSessionUsageCounters = session_store;
    let file_storage =
        systemprompt_storage::build_file_storage(systemprompt_storage::FileStorageBackend::Local {
            root: app_paths.storage().root().to_path_buf(),
        });
    let instance_id = config.instance_id.clone();
    let config = Arc::new(config);
    let fixture_boot = &bootstrap.boot;
    let boot = BootContext::new(
        Arc::clone(fixture_boot.loaded_profile()),
        Arc::clone(fixture_boot.secrets()),
    );
    boot.install_config(config.as_ref().clone())?;
    let boot_secrets = boot.secrets().get();
    let ctx = AppContext::from_parts(
        DataPlane {
            database: Arc::clone(pool),
            analytics_service,
            fingerprint_repo: Some(Arc::new(fixture_fingerprint_repository(pool)?)),
            user_service: Some(Arc::new(
                UserService::new(Arc::clone(&user_repository))
                    .with_owner_reassignments(systemprompt_runtime::owner_reassignments(pool)),
            )),
            a2a_repositories: Arc::new(systemprompt_agent::repository::A2ARepositories::new(
                pool,
                systemprompt_agent::repository::A2aDependencies {
                    session_usage,
                    instance_id: instance_id.clone(),
                    managed_skills: crate::agent::not_managed_skills(),
                    tool_executions: crate::agent::tool_execution_ledger(
                        crate::agent::ToolExecutionLedger::Exists,
                    ),
                },
            )),
            content_repositories: Arc::new(
                systemprompt_content::repository::ContentRepositories::new(pool),
            ),
            oauth_repositories: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
                pool,
                Arc::from(boot_secrets.oauth_at_rest_pepper.as_str()),
            )),
            user_repository,
            service_repository: Arc::new(systemprompt_database::ServiceRepository::new(
                pool,
                instance_id.clone(),
            )),
            coordination: Arc::new(systemprompt_database::CoordinationRepositories::new(
                pool,
                instance_id,
            )),
            ai_repositories: Arc::new(systemprompt_ai::repository::AiRepositories::new(
                pool,
                systemprompt_security::at_rest::AtRestCipher::from_secrets(&boot_secrets)?,
            )),
            analytics_repositories,
            file_repository: Arc::new(systemprompt_files::FileRepository::new(pool)),
            mcp_session_repository: Arc::new(
                systemprompt_mcp::repository::McpSessionRepository::new(pool),
            ),
            managed_repository: Arc::new(
                systemprompt_marketplace::managed::ManagedRepository::new(pool),
            ),
        },
        ConfigPlane {
            boot,
            config: Arc::clone(&config),
            files_config: Arc::new(systemprompt_files::FilesConfig::new(
                app_paths.storage().root().to_path_buf(),
                systemprompt_files::FilesConfigYaml::default(),
            )),
            app_paths,
            content_config: None,
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
        Plugins {
            extension_registry,
            mcp_registry: RegistryService::new(fixture_user_id(), fixture_services_view(), config),
            marketplace_filter,
            marketplace_cache: Arc::new(MarketplaceCache::default()),
            subject_providers: fixture_subject_providers(pool),
            subjects: Default::default(),
        },
        Subsystems {
            system_admin: Arc::new(fixture_system_admin("admin")),
            logging: crate::fixture_logging_runtime(),
            authz_hook,
            signing_keys: crate::jwt::test_signing_keys(),
            governance,
            services,
            cache_epochs: systemprompt_traits::CacheEpochs::new(),
            ai_service: None,
            artifact_ingest: fixture_artifact_ingest(pool)?,
            schema_install: Arc::new(systemprompt_runtime::SchemaState::from(
                systemprompt_database::SchemaInstallReport::default(),
            )),
            event_bridge: fixture_event_bridge(pool),
            event_router: systemprompt_events::EventRouter::local_only(Arc::new(
                systemprompt_events::EventBroadcasters::new(),
            )),
            geoip_reader: None,
            file_storage,
            shutdown: Default::default(),
            background_tasks: Default::default(),
            spawner: crate::subprocess::test_spawner(),
            publish_guard: Arc::new(tokio::sync::Mutex::new(
                systemprompt_marketplace::inventory::PublishGuard::default(),
            )),
            connector_tokens: Arc::default(),
        },
    );

    Ok(Arc::new(ctx))
}

// An ingest over the fixture pool with no scanners: the narrow waist itself,
// not the installation's policy.
pub fn fixture_event_bridge(pool: &DbPool) -> Arc<systemprompt_events::EventBridgeHandle> {
    Arc::new(
        systemprompt_events::PostgresEventBridge::new(
            pool.write_pool().as_ref().clone(),
            crate::unique_instance(),
            systemprompt_traits::CacheEpochs::new(),
            Arc::new(systemprompt_events::EventBroadcasters::new()),
        )
        .into_idle_handle(),
    )
}

pub fn fixture_artifact_ingest(db: &DbPool) -> Result<Arc<systemprompt_mcp::ArtifactIngest>> {
    Ok(Arc::new(systemprompt_mcp::ArtifactIngest::from_db(
        db, None,
    )))
}

// A counter store that never answers: built without a runtime, so a sync
// test can arm a rate window; every charge through it fails closed.
pub fn offline_rate_windows() -> systemprompt_governance::policy::RateWindowRepository {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(50))
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_lazy("postgres://offline:offline@127.0.0.1:1/offline")
        .expect("lazy pool construction is infallible for a well-formed URL");
    systemprompt_governance::policy::RateWindowRepository::new(pool)
}

// The vendor-neutral warn-only chain: what a deployment without a
// `<services>/governance/config.yaml` boots with.
pub fn default_governance_engine() -> Arc<systemprompt_governance::policy::GovernanceEngine> {
    Arc::new(
        systemprompt_governance::policy::GovernanceEngine::from_config(
            &systemprompt_governance::policy::GovernanceConfig::defaults(),
            Some(offline_rate_windows()),
        )
        .expect("the default governance chain always builds"),
    )
}

pub fn default_governance_watch(
) -> systemprompt_traits::ConfigWatch<systemprompt_governance::policy::GovernanceEngine> {
    systemprompt_traits::ConfigWatch::new(
        systemprompt_governance::policy::GovernanceEngine::from_config(
            &systemprompt_governance::policy::GovernanceConfig::defaults(),
            Some(offline_rate_windows()),
        )
        .expect("the default governance chain always builds"),
    )
}

pub fn fixture_subject_providers(
    db: &systemprompt_database::DbPool,
) -> systemprompt_governance::authz::SubjectProviderSet {
    systemprompt_governance::authz::SubjectProviderSet::discover(
        &systemprompt_governance::authz::AuthzHookContext {
            pool: db.pool(),
            sink: std::sync::Arc::new(systemprompt_governance::authz::NullAuditSink),
            services: fixture_services_view(),
            subjects: Default::default(),
        },
    )
}

pub fn fixture_services_view() -> systemprompt_manifest::services::ServicesView {
    fixture_services_state().view()
}

// The services config the test bootstrap serves, or an empty one when the
// test runs without a services tree.
pub fn fixture_services_state() -> systemprompt_loader::ServicesState {
    let boot = &crate::bootstrap::ensure_test_bootstrap().boot;
    systemprompt_loader::ServicesState::load(boot).unwrap_or_else(|_| {
        systemprompt_loader::ServicesState::from_config(boot.profile(), Default::default())
            .expect("the default services config resolves")
    })
}
