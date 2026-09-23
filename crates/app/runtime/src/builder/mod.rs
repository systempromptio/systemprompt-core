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

use composition::{build_data_plane, build_repositories, ensure_legacy_context};

use std::sync::{Arc, OnceLock};

use systemprompt_database::MigrationConfig;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, Plugins, ShutdownRequest, Subsystems};
use crate::error::RuntimeResult;
use crate::registry::ModuleApiRegistry;
pub use core_layer::discover_vertex_models as discover_models;
use core_layer::{CoreLayer, init_core, init_extensions};

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
    install_schemas: bool,
    migration_config: MigrationConfig,
    shutdown: Option<ShutdownRequest>,
}

impl std::fmt::Debug for AppContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContextBuilder")
            .field("extension_registry", &self.extension_registry.is_some())
            .field("show_startup_warnings", &self.show_startup_warnings)
            .field("marketplace_filter", &self.marketplace_filter.is_some())
            .field("authz_hook", &self.authz_hook.is_some())
            .field("install_schemas", &self.install_schemas)
            .field("migration_config", &self.migration_config)
            .field("shutdown", &self.shutdown.is_some())
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
        self.install_schemas = install;
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
    pub const fn with_migration_config(mut self, config: MigrationConfig) -> Self {
        self.migration_config = config;
        self
    }

    pub async fn build(self) -> RuntimeResult<AppContext> {
        let shutdown = self.shutdown.unwrap_or_default();
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
            self.install_schemas,
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
            artifact_ingest: build_artifact_ingest(&database, &governance)?,
            system_admin,
            authz_hook,
            governance,
            schema_install: Arc::new(schema_install),
            event_bridge: Arc::new(OnceLock::new()),
            geoip_reader,
            file_storage,
            shutdown,
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
                api_registry: Arc::new(ModuleApiRegistry::new()),
                mcp_registry,
                marketplace_filter,
                marketplace_cache: Arc::default(),
            },
            subsystems,
        ))
    }
}

fn build_artifact_ingest(
    database: &systemprompt_database::DbPool,
    governance: &systemprompt_security::policy::GovernanceEngine,
) -> RuntimeResult<Arc<systemprompt_mcp::ArtifactIngest>> {
    let ingest = systemprompt_mcp::ArtifactIngest::from_db(
        database,
        governance
            .secret_scanner()
            .map(|scanner| Arc::new(scanner.clone())),
    )
    .map_err(|e| crate::RuntimeError::Internal(format!("artifact ingest: {e}")))?;
    Ok(Arc::new(ingest))
}

async fn build_domain_layer(
    config: &systemprompt_models::Config,
    database: &systemprompt_database::DbPool,
    analytics_repositories: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
) -> RuntimeResult<(
    composition::RepositoryBundles,
    Arc<UserService>,
    Arc<systemprompt_models::SystemAdmin>,
    RegistryService,
)> {
    let mut repositories = build_repositories(
        database,
        analytics_repositories,
        systemprompt_identifiers::InstanceId::new(&config.instance_id),
    )?;
    let user_service = Arc::new(UserService::new(Arc::clone(&repositories.users)));
    let system_admin = assembly::resolve_and_install_system_admin(config, &user_service).await?;
    repositories.install_organization_resolver(system_admin.id());
    let mcp_registry = RegistryService::new(system_admin.id().clone());
    ensure_legacy_context(&repositories, &system_admin).await?;
    Ok((repositories, user_service, system_admin, mcp_registry))
}
