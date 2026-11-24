//! [`AppContext`] fixtures for integration tests.
//!
//! Every context is built by [`AppContextBuilder`] over an admitted
//! composition, on a test bootstrap's profile with the given [`Config`]
//! installed and the fixture signing keys.
//! The fixture profile's authorization mode is `unrestricted`, so the default
//! builder supplies no extension hook; [`fixture_app_context_with_hook`] builds
//! on a copy of the profile whose mode is `extension`. Every build seeds the
//! fixture system admin ([`crate::credential::seed_fixture_system_admin`]) on
//! the config's database first. [`fixture_admitted`] is the core schema
//! composition (`systemprompt_test_migrate::core_members`); a test crate that
//! links the `systemprompt` facade passes `reference()` admitted instead.
//! Schema installation stays off: the database is migrated before the run.
//!
//! [`test_app_context`] panics when the context cannot be assembled: a test
//! whose context is missing fails rather than skips (see [`crate::db`]).

use std::sync::Arc;

use anyhow::Result;
use systemprompt_analytics::FingerprintRepository;
use systemprompt_config::LoadedProfile;
use systemprompt_database::DbPool;
use systemprompt_extension::{Admitted, Extension, StandingTag};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::BootContext;
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_manifest::Config;
use systemprompt_marketplace::MarketplaceFilter;
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

pub fn fixture_session_usage(db: &DbPool) -> systemprompt_traits::DynSessionUsageCounters {
    Arc::new(systemprompt_users::SessionRepository::new(db))
}

pub fn fixture_session_store(db: &DbPool) -> systemprompt_traits::DynSessionStore {
    Arc::new(systemprompt_users::SessionRepository::new(db))
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
        system_admin_username: crate::credential::FIXTURE_SYSTEM_ADMIN_USERNAME.to_owned(),
        system_admin_email: Some(
            systemprompt_identifiers::Email::try_new(crate::credential::FIXTURE_SYSTEM_ADMIN_EMAIL)
                .expect("the fixture admin email is well-formed"),
        ),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        allow_dynamic_client_registration: true,
        api_key_max_lifetime_days: None,
        login_page_url: None,
        ui_session: Default::default(),
        authorize: Default::default(),
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
    Arc::new(admitted_of(members))
}

// The schema-bearing core members and the documents member, as a child
// host admits them.
pub fn host_admitted() -> Admitted {
    let mut members = systemprompt_test_migrate::core_members();
    members.push(Box::new(
        systemprompt_documents::extension::DocumentsExtension,
    ));
    admitted_of(members)
}

fn admitted_of(members: Vec<Box<dyn Extension>>) -> Admitted {
    let members = members
        .iter()
        .map(|ext| {
            systemprompt_test_migrate::admitted_member(ext.as_ref(), StandingTag::Core)
                .unwrap_or_else(|e| panic!("{} contributes: {e}", ext.metadata().id))
        })
        .collect();
    Admitted::__from_facade(systemprompt_extension::admitted::AdmittedParts {
        members,
        ..Default::default()
    })
}

pub fn fixture_core_members() -> Vec<Box<dyn Extension>> {
    let mut members = systemprompt_test_migrate::core_members();
    members.push(Box::new(systemprompt_gateway::extension::GatewayExtension));
    members.push(Box::new(
        systemprompt_gateway_entry::extension::GatewayHttpExtension,
    ));
    members.push(Box::new(
        systemprompt_documents::extension::DocumentsExtension,
    ));
    members.push(Box::new(
        systemprompt_files_entry::extension::FilesHttpExtension,
    ));
    members.push(Box::new(
        systemprompt_identity_entry::extension::IdentityHttpExtension,
    ));
    members
}

pub fn fixture_admitted() -> Arc<Admitted> {
    fixture_admitted_of(fixture_core_members())
}

pub fn fixture_admitted_without(disabled: &[ExtensionId]) -> Arc<Admitted> {
    fixture_admitted_of(
        fixture_core_members()
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
    context_builder_on_profile(
        bootstrap,
        Arc::clone(bootstrap.boot.loaded_profile()),
        config,
        admitted,
    )
}

fn context_builder_on_profile(
    bootstrap: &TestBootstrap,
    profile: Arc<LoadedProfile>,
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<AppContextBuilder> {
    let boot = BootContext::new(profile, Arc::clone(bootstrap.boot.secrets()));
    boot.install_config(config)?;
    Ok(AppContextBuilder::new(boot, admitted)
        .with_signing_keys(crate::jwt::test_signing_keys())
        .with_services(bootstrap.services.clone()))
}

fn extension_mode_profile(bootstrap: &TestBootstrap) -> Result<Arc<LoadedProfile>> {
    let source = std::fs::read_to_string(&bootstrap.profile_path)?;
    let mut doc: serde_yaml::Value = serde_yaml::from_str(&source)?;
    let hook = doc
        .get_mut("governance")
        .and_then(|g| g.get_mut("authz"))
        .and_then(|a| a.get_mut("hook"))
        .and_then(serde_yaml::Value::as_mapping_mut)
        .ok_or_else(|| anyhow::anyhow!("the fixture profile declares governance.authz.hook"))?;
    hook.insert("mode".into(), "extension".into());
    hook.remove("acknowledgement");
    let dir = bootstrap
        .profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("the fixture profile has a parent directory"))?;
    let path = dir.join(format!("profile-extension-{}.yaml", uuid::Uuid::new_v4()));
    std::fs::write(&path, serde_yaml::to_string(&doc)?)?;
    Ok(LoadedProfile::from_path(&path)?)
}

pub async fn seed_fixture_system_admin_at(database_url: &str) -> Result<()> {
    let pool = crate::db::connect(database_url).await?;
    crate::credential::seed_fixture_system_admin(&pool).await
}

fn bootstrap_config(bootstrap: &TestBootstrap) -> Config {
    let paths = &bootstrap.boot.profile().paths;
    let mut config = fixture_config(&bootstrap.database_url);
    config.system_path = paths.system.clone();
    config.services_path = paths.services.clone();
    config.bin_path = paths.bin.clone();
    config
}

async fn build(builder: AppContextBuilder, database_url: &str) -> Result<Arc<AppContext>> {
    seed_fixture_system_admin_at(database_url).await?;
    Ok(Arc::new(builder.build().await?))
}

pub async fn test_app_context(database_url: &str) -> Arc<AppContext> {
    fixture_app_context_with_config(fixture_config(database_url))
        .await
        .unwrap_or_else(|e| panic!("fixture AppContext over the test database: {e:#}"))
}

pub async fn test_app_context_on(bootstrap: &TestBootstrap) -> Arc<AppContext> {
    test_app_context_on_with(bootstrap, fixture_admitted()).await
}

// The services tree is read again at build, so files a test writes into the
// bootstrap's services directory after the bootstrap was created are seen.
pub async fn test_app_context_on_with(
    bootstrap: &TestBootstrap,
    admitted: Arc<Admitted>,
) -> Arc<AppContext> {
    let services = systemprompt_loader::ServicesState::load(&bootstrap.boot)
        .unwrap_or_else(|_| bootstrap.services.clone());
    let built = match fixture_context_builder(bootstrap, bootstrap_config(bootstrap), admitted) {
        Ok(builder) => build(builder.with_services(services), &bootstrap.database_url).await,
        Err(error) => Err(error),
    };
    built.unwrap_or_else(|e| panic!("fixture AppContext on the supplied bootstrap: {e:#}"))
}

pub async fn fixture_app_context_reloaded_on(
    bootstrap: &TestBootstrap,
    database_url: &str,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    let mut config = bootstrap_config(bootstrap);
    database_url.clone_into(&mut config.database_url);
    fixture_app_context_reloaded_with(bootstrap, config, admitted).await
}

// As [`fixture_app_context_on_bootstrap`], with the services tree read again
// at build, so files a test writes after the bootstrap was created are seen.
pub async fn fixture_app_context_reloaded_with(
    bootstrap: &TestBootstrap,
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    let services = systemprompt_loader::ServicesState::load(&bootstrap.boot)?;
    let database_url = config.database_url.clone();
    build(
        fixture_context_builder(bootstrap, config, admitted)?.with_services(services),
        &database_url,
    )
    .await
}

pub async fn fixture_app_context_on_bootstrap(
    bootstrap: &TestBootstrap,
    config: Config,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    let database_url = config.database_url.clone();
    build(
        fixture_context_builder(bootstrap, config, admitted)?,
        &database_url,
    )
    .await
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
            crate::marketplace::fixture_admitted_with_filter(marketplace_filter),
        )?,
        database_url,
    )
    .await
}

pub async fn fixture_app_context_with_hook(
    database_url: &str,
    authz_hook: SharedAuthzHook,
) -> Result<Arc<AppContext>> {
    fixture_app_context_with_hook_over(database_url, authz_hook, fixture_admitted()).await
}

pub async fn fixture_app_context_with_hook_over(
    database_url: &str,
    authz_hook: SharedAuthzHook,
    admitted: Arc<Admitted>,
) -> Result<Arc<AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    build(
        context_builder_on_profile(
            bootstrap,
            extension_mode_profile(bootstrap)?,
            fixture_config(database_url),
            admitted,
        )?
        .with_shared_authz_hook(authz_hook),
        database_url,
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
        database_url,
    )
    .await
}

pub async fn fixture_app_context_with_services(
    config: Config,
    services: systemprompt_loader::ServicesState,
) -> Result<Arc<AppContext>> {
    let database_url = config.database_url.clone();
    build(
        fixture_context_builder(ensure_test_bootstrap(), config, fixture_admitted())?
            .with_services(services),
        &database_url,
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
        db,
        systemprompt_governance::policy::secrets::SecretScannerCell::default(),
    )))
}

// A counter store that never answers: built without a runtime, so a sync
// test can arm a rate window; every charge through it fails closed.
pub fn offline_rate_windows() -> systemprompt_governance::policy::RateWindowRepository {
    systemprompt_governance::policy::RateWindowRepository::new(sqlx::PgPool::clone(
        &crate::db::lazy_pg_pool(),
    ))
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

pub fn fixture_job_context_over(
    pool: &DbPool,
    seed: systemprompt_extension::prelude::Dependencies,
) -> systemprompt_traits::JobContext {
    systemprompt_traits::JobContext::new(
        crate::fixture_actor(),
        seed.with(Arc::clone(pool))
            .with(systemprompt_database::BudgetedDb::new(
                Arc::clone(pool),
                None,
            )),
    )
}

pub fn fixture_kernel(
    pool: &DbPool,
    seed: systemprompt_extension::prelude::Dependencies,
) -> systemprompt_runtime::Kernel {
    let database: Arc<dyn systemprompt_traits::DatabaseHandle> = Arc::clone(pool) as _;
    systemprompt_runtime::Kernel::new(database, systemprompt_traits::BackgroundTasks::new(), seed)
}

// A kernel over a pool that never connects; it needs no Tokio runtime (see
// [`crate::db::lazy_pg_pool`]), so a sync test may build one.
pub fn offline_kernel(
    seed: systemprompt_extension::prelude::Dependencies,
) -> systemprompt_runtime::Kernel {
    let pool: DbPool = Arc::new(systemprompt_database::Database::from_pools(
        crate::db::lazy_pg_pool(),
        None,
    ));
    fixture_kernel(&pool, seed)
}

pub struct FixtureSeedParts {
    pub file_storage: Arc<dyn systemprompt_traits::FileStorage>,
    pub background_tasks: systemprompt_traits::BackgroundTasks,
    pub event_router: systemprompt_events::EventRouter,
    pub documents: systemprompt_files::documents::DocumentsHandlesSlot,
}

// The kernel seeds a host composes, built by the host's own
// `base_kernel_state` over the test bootstrap: a fixture that resolves an
// admitted member over these seeds sees what the member sees in a real host.
// Coordination is the child-host shape (`UnavailableCoordination`).
pub fn fixture_kernel_seed(
    pool: &DbPool,
    parts: FixtureSeedParts,
) -> systemprompt_extension::prelude::Dependencies {
    let bootstrap = ensure_test_bootstrap();
    let (seed, _binders) =
        systemprompt_runtime::base_kernel_state(systemprompt_runtime::KernelSeed {
            database: pool,
            app_paths: &Arc::new(bootstrap.app_paths.clone()),
            files_config: &Arc::new(bootstrap.files_config()),
            profile: bootstrap.boot.profile(),
            config: bootstrap.boot_config(),
            cache_epochs: &systemprompt_traits::CacheEpochs::new(),
            verification: crate::jwt::test_signing_keys().keyring(),
            services: fixture_services_view(),
            services_root: crate::bootstrap::test_boot_active_services_root(),
            profile_path: systemprompt_manifest::services::root::ActiveProfilePath {
                path: bootstrap.profile_path.clone(),
            },
            secrets: fixture_secret_store(),
            secret_lookup: Arc::<systemprompt_config::SecretsService>::clone(
                crate::bootstrap::ensure_test_bootstrap().boot.secrets(),
            ),
            connector_tokens: Arc::default(),
            file_storage: parts.file_storage,
            background_tasks: parts.background_tasks,
            coordination: systemprompt_runtime::KernelCoordination {
                instances: Arc::new(systemprompt_traits::UnavailableCoordination),
                leases: Arc::new(systemprompt_traits::UnavailableCoordination),
                locks: Arc::new(systemprompt_traits::UnavailableCoordination),
            },
            system_admin: &crate::user::fixture_system_admin(
                crate::credential::FIXTURE_SYSTEM_ADMIN_USERNAME,
            ),
            event_router: &parts.event_router,
            documents: parts.documents,
            catalog_roles: systemprompt_models::authz::CatalogRoleNames::default(),
            escalation: systemprompt_notifications::service::EscalationDelivery::InProcess,
        });
    seed
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
