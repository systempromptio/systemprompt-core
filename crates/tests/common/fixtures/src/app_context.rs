//! [`AppContext`] fixtures for integration tests.
//!
//! Every context is built by [`AppContextBuilder`] over an admitted
//! composition, on a test bootstrap's profile with the given [`Config`]
//! installed, an allow-all authorization hook and marketplace filter, and the
//! fixture signing keys. [`fixture_admitted`] is the core schema composition
//! (`systemprompt_test_migrate::core_members`); a test crate that links the
//! `systemprompt` facade passes `reference()` admitted instead. Schema
//! installation stays off: the database is migrated before the run.
//!
//! [`test_app_context`] panics when the context cannot be assembled: a test
//! whose context is missing fails rather than skips (see [`crate::db`]).

use std::sync::Arc;

use anyhow::Result;
use systemprompt_analytics::FingerprintRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::{Admitted, Extension, StandingTag};
use systemprompt_governance::authz::{AllowAllHook, NullAuditSink, SharedAuthzHook};
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::BootContext;
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_manifest::Config;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter};
use systemprompt_runtime::{AppContext, AppContextBuilder};

use crate::bootstrap::{ensure_test_bootstrap, TestBootstrap};

pub const FIXTURE_JWT_ISSUER: &str = "https://issuer.test";

pub fn fixture_analytics_repositories(
    db: &DbPool,
) -> Result<systemprompt_analytics::repository::AnalyticsRepositories> {
    Ok(
        systemprompt_analytics::repository::AnalyticsRepositories::new(
            db,
            Arc::new(systemprompt_users::SessionRepository::new(db)),
            Arc::new(systemprompt_logging::AnalyticsRepository::new(db)),
        ),
    )
}

pub fn fixture_fingerprint_repository(db: &DbPool) -> Result<FingerprintRepository> {
    Ok(FingerprintRepository::new(
        db,
        Arc::new(systemprompt_users::SessionRepository::new(db)),
    ))
}

pub const FIXTURE_UPSTREAM_ISSUER: &str = "https://upstream.fixture.invalid";

pub fn fixture_upstream_issuer() -> systemprompt_manifest::profile::TrustedIssuer {
    systemprompt_manifest::profile::TrustedIssuer {
        issuer: FIXTURE_UPSTREAM_ISSUER.to_owned(),
        jwks_uri: "http://127.0.0.1:1/jwks".to_owned(),
        audience: "fixture-upstream".to_owned(),
        typ_allowlist: Vec::new(),
        allowed_client_ids: Vec::new(),
        can_issue_id_jag: true,
        link: systemprompt_traits::LinkPolicy::default(),
    }
}

pub fn fixture_config(database_url: &str) -> Config {
    Config {
        instance_id: crate::unique_instance(),
        metrics_port: None,
        max_concurrent_streams: 16,
        role: Default::default(),
        advertise_url: None,
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
        trusted_issuers: vec![fixture_upstream_issuer()],
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

pub fn fixture_admitted_of(members: Vec<Box<dyn Extension>>) -> Arc<Admitted> {
    let members = members
        .iter()
        .map(|ext| {
            systemprompt_test_migrate::admitted_member(ext.as_ref(), StandingTag::Core)
                .unwrap_or_else(|e| panic!("{} contributes: {e}", ext.metadata().id))
        })
        .collect();
    Arc::new(Admitted::__from_facade(
        systemprompt_extension::admitted::AdmittedParts {
            members,
            ..Default::default()
        },
    ))
}

pub fn fixture_admitted() -> Arc<Admitted> {
    fixture_admitted_of(systemprompt_test_migrate::core_members())
}

pub fn fixture_admitted_without(disabled: &[ExtensionId]) -> Arc<Admitted> {
    fixture_admitted_of(
        systemprompt_test_migrate::core_members()
            .into_iter()
            .filter(|ext| !disabled.iter().any(|id| id.as_str() == ext.metadata().id))
            .collect(),
    )
}

pub fn fixture_context_builder(
    bootstrap: &TestBootstrap,
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<AppContextBuilder> {
    let boot = BootContext::new(
        Arc::clone(bootstrap.boot.loaded_profile()),
        Arc::clone(bootstrap.boot.secrets()),
    );
    boot.install_config(config)?;
    Ok(AppContextBuilder::new(boot, admitted)
        .with_shared_authz_hook(allow_all_hook())
        .with_marketplace_filter(Arc::new(AllowAllFilter))
        .with_signing_keys(crate::jwt::test_signing_keys())
        .with_services(bootstrap.services.clone()))
}

fn allow_all_hook() -> SharedAuthzHook {
    Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)))
}

fn bootstrap_config(bootstrap: &TestBootstrap) -> Config {
    Config::clone(bootstrap.boot_config())
}

async fn build(builder: AppContextBuilder) -> Result<Arc<AppContext>> {
    Ok(Arc::new(builder.build().await?))
}

pub async fn test_app_context(database_url: &str) -> Arc<AppContext> {
    fixture_app_context_with_config(fixture_config(database_url))
        .await
        .unwrap_or_else(|e| panic!("fixture AppContext over the test database: {e:#}"))
}

pub async fn test_app_context_on(bootstrap: &TestBootstrap) -> Arc<AppContext> {
    fixture_app_context_on_bootstrap(bootstrap, bootstrap_config(bootstrap), fixture_admitted())
        .await
        .unwrap_or_else(|e| panic!("fixture AppContext on the supplied bootstrap: {e:#}"))
}

pub async fn fixture_app_context_on_bootstrap(
    bootstrap: &TestBootstrap,
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    build(fixture_context_builder(bootstrap, config, admitted)?).await
}

pub async fn fixture_app_context_with_config(config: Config) -> Result<Arc<AppContext>> {
    fixture_app_context_with_extensions(config, fixture_admitted()).await
}

pub async fn fixture_app_context_with_extensions(
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    fixture_app_context_on_bootstrap(ensure_test_bootstrap(), config, admitted).await
}

pub async fn fixture_app_context_with_filter(
    database_url: &str,
    marketplace_filter: Arc<dyn MarketplaceFilter>,
) -> Result<Arc<AppContext>> {
    build(
        fixture_context_builder(
            ensure_test_bootstrap(),
            fixture_config(database_url),
            fixture_admitted(),
        )?
        .with_marketplace_filter(marketplace_filter),
    )
    .await
}

pub async fn fixture_app_context_with_hook(
    database_url: &str,
    authz_hook: SharedAuthzHook,
) -> Result<Arc<AppContext>> {
    build(
        fixture_context_builder(
            ensure_test_bootstrap(),
            fixture_config(database_url),
            fixture_admitted(),
        )?
        .with_shared_authz_hook(authz_hook),
    )
    .await
}

pub async fn fixture_app_context_with_build(
    database_url: &str,
    build_identity: systemprompt_models::BuildIdentity,
) -> Result<Arc<AppContext>> {
    build(
        fixture_context_builder(
            ensure_test_bootstrap(),
            fixture_config(database_url),
            fixture_admitted(),
        )?
        .with_build_identity(build_identity),
    )
    .await
}

pub async fn fixture_app_context_with_services(
    config: Config,
    services: systemprompt_loader::ServicesState,
) -> Result<Arc<AppContext>> {
    build(
        fixture_context_builder(ensure_test_bootstrap(), config, fixture_admitted())?
            .with_services(services),
    )
    .await
}

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
            &systemprompt_governance::policy::PolicyCatalog::builtin(),
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
            &systemprompt_governance::policy::PolicyCatalog::builtin(),
            Some(offline_rate_windows()),
        )
        .expect("the default governance chain always builds"),
    )
}

pub fn fixture_subject_providers(
    _db: &systemprompt_database::DbPool,
) -> systemprompt_governance::authz::SubjectProviderSet {
    systemprompt_governance::authz::SubjectProviderSet::default()
}

pub async fn fixture_extension_context(
    database_url: &str,
) -> Arc<dyn systemprompt_extension::ExtensionContext> {
    let concrete = test_app_context(database_url).await;
    let ctx: Arc<dyn systemprompt_extension::ExtensionContext> = concrete;
    ctx
}

pub fn fixture_kernel(
    pool: &DbPool,
    seed: systemprompt_extension::prelude::Dependencies,
) -> systemprompt_runtime::Kernel {
    let database: Arc<dyn systemprompt_traits::DatabaseHandle> = Arc::clone(pool) as _;
    systemprompt_runtime::Kernel::new(database, systemprompt_traits::BackgroundTasks::new(), seed)
}

pub fn offline_kernel(
    seed: systemprompt_extension::prelude::Dependencies,
) -> systemprompt_runtime::Kernel {
    let pool: DbPool = Arc::new(systemprompt_database::Database::from_pools(
        crate::db::lazy_pg_pool(),
        None,
    ));
    fixture_kernel(&pool, seed)
}

pub fn fixture_secret_store() -> Arc<dyn systemprompt_traits::SecretStore> {
    let concrete = Arc::clone(crate::bootstrap::ensure_test_bootstrap().boot.secrets());
    let store: Arc<dyn systemprompt_traits::SecretStore> = concrete;
    store
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
