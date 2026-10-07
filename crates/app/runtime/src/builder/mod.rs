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

pub use composition::owner_reassignments;
use composition::{build_data_plane, build_repositories, ensure_legacy_context};

use std::sync::{Arc, OnceLock};

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_traits::BackgroundTasks;
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, Plugins, ShutdownRequest, Subsystems};
use crate::error::RuntimeResult;
pub use core_layer::discover_vertex_models;
use core_layer::{CoreLayer, SchemaPolicy, init_core, init_extensions};

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
    authz_hook: Option<SharedAuthzHook>,
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
            .field("authz_hook", &self.authz_hook.is_some())
            .field("install_schemas", &self.schema.install)
            .field("verify_schema", &self.schema.verify)
            .field("migration_config", &self.migration_config)
            .field("shutdown", &self.shutdown.is_some())
            .field("background_tasks", &self.background_tasks.is_some())
            .field("event_router", &self.event_router)
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
        self.authz_hook = Some(Arc::new(hook));
        self
    }

    #[must_use]
    pub fn with_shared_authz_hook(mut self, hook: SharedAuthzHook) -> Self {
        self.authz_hook = Some(hook);
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
    pub const fn with_migration_config(mut self, config: MigrationConfig) -> Self {
        self.migration_config = config;
        self
    }

    pub async fn build(self) -> RuntimeResult<AppContext> {
        let CoreLayer {
            config,
            app_paths,
            database,
            authz_hook,
            governance,
            file_storage,
        } = init_core(self.authz_hook).await?;

        let (extension_registry, schema_install) = init_extensions(
            self.extension_registry,
            self.schema,
            self.migration_config,
            &database,
        )
        .await?;

        let assembly::ContentAnalytics {
            geoip_reader,
            content_config,
            route_classifier,
            analytics_service,
            analytics_repositories,
            fingerprint_repo,
        } = assembly::assemble_content_analytics(
            &config,
            &app_paths,
            &database,
            self.show_startup_warnings,
        )?;

        let (repositories, user_service, system_admin, mcp_registry) =
            build_domain_layer(&config, &database, analytics_repositories).await?;

        let marketplace_filter = self
            .marketplace_filter
            .unwrap_or_else(|| assembly::build_marketplace_filter(&database));

        let subsystems = Subsystems {
            ai_service: ai_service::build_ai_service(&database, &repositories, &mcp_registry)?,
            artifact_ingest: build_artifact_ingest(&database, &governance),
            system_admin,
            authz_hook,
            governance,
            schema_install: Arc::new(schema_install),
            event_bridge: Arc::new(OnceLock::new()),
            event_router: self
                .event_router
                .unwrap_or_else(|| outbox_router(&database, &config.instance_id)),
            geoip_reader,
            file_storage,
            shutdown: self.shutdown.unwrap_or_default(),
            background_tasks: self.background_tasks.unwrap_or_default(),
            publish_guard: Arc::default(),
        };

        Ok(AppContext::from_parts(
            build_data_plane(
                database,
                analytics_service,
                fingerprint_repo,
                user_service,
                repositories,
            ),
            ConfigPlane {
                config,
                app_paths,
                content_config,
                route_classifier,
            },
            Plugins {
                extension_registry,
                mcp_registry,
                marketplace_filter,
                marketplace_cache: Arc::default(),
            },
            subsystems,
        ))
    }
}

fn outbox_router(
    database: &systemprompt_database::DbPool,
    instance_id: &systemprompt_identifiers::InstanceId,
) -> EventRouter {
    EventRouter::with_outbox(database.write_pool().as_ref().clone(), instance_id.clone())
}

fn build_artifact_ingest(
    database: &systemprompt_database::DbPool,
    governance: &systemprompt_security::policy::GovernanceEngine,
) -> Arc<systemprompt_mcp::ArtifactIngest> {
    let ingest = systemprompt_mcp::ArtifactIngest::from_db(
        database,
        governance
            .secret_scanner()
            .map(|scanner| Arc::new(scanner.clone())),
    );
    Arc::new(ingest)
}

async fn build_domain_layer(
    config: &systemprompt_manifest::Config,
    database: &systemprompt_database::DbPool,
    analytics_repositories: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
) -> RuntimeResult<(
    composition::RepositoryBundles,
    Arc<UserService>,
    Arc<systemprompt_manifest::SystemAdmin>,
    RegistryService,
)> {
    let mut repositories =
        build_repositories(database, analytics_repositories, config.instance_id.clone());
    let user_service = Arc::new(
        UserService::new(Arc::clone(&repositories.users))
            .with_owner_reassignments(owner_reassignments(database)),
    );
    let system_admin = assembly::resolve_and_install_system_admin(config, &user_service).await?;
    repositories.install_organization_resolver(system_admin.id());
    let mcp_registry = RegistryService::new(system_admin.id().clone());
    ensure_legacy_context(&repositories, &system_admin).await?;
    Ok((repositories, user_service, system_admin, mcp_registry))
}
