//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services. Failures at
//! any step propagate as [`RuntimeError`](crate::error::RuntimeError).
//! Subsystem resolution helpers live in [`assembly`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_service;
mod assembly;
mod composition;
mod core_layer;
mod database;
mod events;
mod governance;

pub use composition::owner_reassignments;
use composition::{build_data_plane, build_plugins, build_repositories, ensure_legacy_context};

use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_governance::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_manifest::Config;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::BackgroundTasks;
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, ShutdownRequest, Subsystems};
use crate::error::RuntimeResult;
pub use core_layer::discover_vertex_models;
use core_layer::{CoreLayer, CoreOverrides, SchemaPolicy, init_core};
use events::event_plane;
use governance::init_governed_extensions;
pub(crate) use governance::load_governance;

/// Assembles an [`AppContext`], owning the bootstrap order described on the
/// module.
///
/// All fields default to a no-op build: extensions are discovered via
/// inventory, schema installation is off, and the marketplace filter falls
/// back to the inventory-registered implementation (or an allow-all filter).
/// Override these with the `with_*` methods before calling
/// [`build`](Self::build).
#[derive(Default)]
pub struct AppContextBuilder {
    extension_registry: Option<ExtensionRegistry>,
    show_startup_warnings: bool,
    marketplace_filter: Option<Arc<dyn MarketplaceFilter>>,
    core: CoreOverrides,
    schema: SchemaPolicy,
    migration_config: MigrationConfig,
    shutdown: Option<ShutdownRequest>,
    background_tasks: Option<BackgroundTasks>,
    event_router: Option<EventRouter>,
}

impl std::fmt::Debug for AppContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContextBuilder")
            .field("extension_registry", &self.extension_registry.is_some())
            .field("show_startup_warnings", &self.show_startup_warnings)
            .field("marketplace_filter", &self.marketplace_filter.is_some())
            .field("authz_hook", &self.core.authz_hook.is_some())
            .field("signing_keys", &self.core.signing_keys.is_some())
            .field("install_schemas", &self.schema.install)
            .field("verify_schema", &self.schema.verify)
            .field("database_url_override", &self.core.database_url.is_some())
            .field("migration_config", &self.migration_config)
            .field("shutdown", &self.shutdown.is_some())
            .field("background_tasks", &self.background_tasks.is_some())
            .field("event_router", &self.event_router)
            .field("services", &self.core.services.is_some())
            .finish()
    }
}

impl AppContextBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_extensions(mut self, registry: ExtensionRegistry) -> Self {
        self.extension_registry = Some(registry);
        self
    }

    #[must_use]
    pub const fn with_startup_warnings(mut self, show: bool) -> Self {
        self.show_startup_warnings = show;
        self
    }

    #[must_use]
    pub fn with_marketplace_filter(mut self, filter: Arc<dyn MarketplaceFilter>) -> Self {
        self.marketplace_filter = Some(filter);
        self
    }

    #[must_use]
    pub const fn with_migrations(mut self, install: bool) -> Self {
        self.schema.install = install;
        self
    }

    #[must_use]
    pub const fn with_schema_verification(mut self, verify: bool) -> Self {
        self.schema.verify = verify;
        self
    }

    #[must_use]
    pub fn with_authz_hook<H>(mut self, hook: H) -> Self
    where
        H: AuthzDecisionHook + 'static,
    {
        self.core.authz_hook = Some(Arc::new(hook));
        self
    }

    #[must_use]
    pub fn with_signing_keys(mut self, keys: SigningKeys) -> Self {
        self.core.signing_keys = Some(keys);
        self
    }

    #[must_use]
    pub fn with_shared_authz_hook(mut self, hook: SharedAuthzHook) -> Self {
        self.core.authz_hook = Some(hook);
        self
    }

    #[must_use]
    pub fn with_shutdown(mut self, shutdown: ShutdownRequest) -> Self {
        self.shutdown = Some(shutdown);
        self
    }

    #[must_use]
    pub fn with_background_tasks(mut self, tasks: BackgroundTasks) -> Self {
        self.background_tasks = Some(tasks);
        self
    }

    #[must_use]
    pub fn with_event_router(mut self, router: EventRouter) -> Self {
        self.event_router = Some(router);
        self
    }

    #[must_use]
    pub fn with_database_url(mut self, url: impl Into<String>) -> Self {
        self.core.database_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn with_services(mut self, services: systemprompt_loader::ServicesState) -> Self {
        self.core.services = Some(services);
        self
    }

    #[must_use]
    pub const fn with_migration_config(mut self, config: MigrationConfig) -> Self {
        self.migration_config = config;
        self
    }

    pub async fn build(self) -> RuntimeResult<AppContext> {
        let CoreLayer {
            config,
            app_paths,
            files_config,
            database,
            authz_hook,
            signing_keys,
            file_storage,
            cache_epochs,
            log_sink,
            policy,
            services,
        } = init_core(self.core).await?;

        let (extensions, schema_install, governance) = init_governed_extensions(
            self.extension_registry,
            self.schema,
            self.migration_config,
            &database,
        )
        .await?;

        let content = assembly::assemble_content_analytics(
            &config,
            &app_paths,
            &database,
            self.show_startup_warnings,
        )?;

        let analytics = content.analytics_repositories;
        let (repositories, user_service, system_admin, mcp_registry) =
            build_domain_layer(&config, &database, analytics, policy, &services).await?;

        let (ai_service, connector_tokens) =
            ai_service::build_ai_service(&database, &repositories, &mcp_registry, &services);
        let (router, bridge) = event_plane(self.event_router, &database, &config, &cache_epochs);
        let subsystems = Subsystems {
            ai_service,
            artifact_ingest: build_artifact_ingest(&database, &governance.get()),
            logging: systemprompt_logging::LoggingRuntime::new(log_sink, Arc::clone(&system_admin)),
            system_admin,
            authz_hook,
            signing_keys,
            governance,
            services,
            cache_epochs,
            schema_install: Arc::new(crate::SchemaState::from(schema_install)),
            event_bridge: Arc::new(bridge),
            event_router: router,
            geoip_reader: content.geoip_reader,
            file_storage,
            shutdown: self.shutdown.unwrap_or_default(),
            background_tasks: self.background_tasks.unwrap_or_default(),
            spawner: new_spawner()?,
            publish_guard: Arc::default(),
            connector_tokens,
        };

        let plugins = build_plugins(extensions, mcp_registry, self.marketplace_filter, &database);

        Ok(AppContext::from_parts(
            build_data_plane(
                database,
                content.analytics_service,
                content.fingerprint_repo,
                user_service,
                repositories,
            ),
            ConfigPlane {
                config,
                app_paths,
                files_config,
                content_config: content.content_config,
                route_classifier: content.route_classifier,
            },
            plugins,
            subsystems,
        ))
    }
}

fn build_artifact_ingest(
    database: &systemprompt_database::DbPool,
    governance: &systemprompt_governance::policy::GovernanceEngine,
) -> Arc<systemprompt_mcp::ArtifactIngest> {
    let scanner = governance.secret_scanner().map(|s| Arc::new(s.clone()));
    Arc::new(systemprompt_mcp::ArtifactIngest::from_db(database, scanner))
}

async fn build_domain_layer(
    config: &Config,
    database: &systemprompt_database::DbPool,
    analytics_repositories: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    policy: systemprompt_marketplace::managed::PublicationPolicy,
    services: &systemprompt_loader::ServicesState,
) -> RuntimeResult<(
    composition::RepositoryBundles,
    Arc<UserService>,
    Arc<systemprompt_manifest::SystemAdmin>,
    RegistryService,
)> {
    let mut repositories = build_repositories(
        database,
        analytics_repositories,
        config.instance_id.clone(),
        policy,
    );
    let user_service = Arc::new(
        UserService::new(Arc::clone(&repositories.users))
            .with_owner_reassignments(owner_reassignments(database)),
    );
    let system_admin = assembly::load_system_admin(config, &user_service).await?;
    repositories.install_organization_resolver(system_admin.id());
    let mcp_registry = RegistryService::new(system_admin.id().clone(), services.clone());
    ensure_legacy_context(&repositories, &system_admin).await?;
    Ok((repositories, user_service, system_admin, mcp_registry))
}

fn new_spawner() -> RuntimeResult<Arc<systemprompt_loader::subprocess::Spawner>> {
    systemprompt_loader::subprocess::Spawner::new()
        .map(Arc::new)
        .map_err(crate::error::RuntimeError::Spawner)
}
