//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services. Failures at
//! any step propagate as [`RuntimeError`](crate::error::RuntimeError).

use std::sync::{Arc, OnceLock};

use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{Database, MigrationConfig, install_extension_schemas_full};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter, discover_filters};
use systemprompt_mcp::services::registry::RegistryManager;
use systemprompt_models::services::{SystemAdmin, SystemAdminConfig};
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting};
use systemprompt_users::UserService;

use crate::context::{AppContext, AppContextParts};
use crate::context_loaders;
use crate::error::{RuntimeError, RuntimeResult};
use crate::registry::ModuleApiRegistry;

#[derive(Default)]
pub struct AppContextBuilder {
    extension_registry: Option<ExtensionRegistry>,
    show_startup_warnings: bool,
    marketplace_filter: Option<Arc<dyn MarketplaceFilter>>,
    install_schemas: bool,
    migration_config: MigrationConfig,
}

impl std::fmt::Debug for AppContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContextBuilder")
            .field("extension_registry", &self.extension_registry.is_some())
            .field("show_startup_warnings", &self.show_startup_warnings)
            .field("marketplace_filter", &self.marketplace_filter.is_some())
            .field("install_schemas", &self.install_schemas)
            .field("migration_config", &self.migration_config)
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

    /// Install / migrate extension schemas as part of `build()`. Off by
    /// default so admin tools (`db doctor`, repair scripts) can open a
    /// connection without mutating the schema. `serve` turns this on.
    #[must_use]
    pub const fn with_migrations(mut self, install: bool) -> Self {
        self.install_schemas = install;
        self
    }

    #[must_use]
    pub const fn with_migration_config(mut self, config: MigrationConfig) -> Self {
        self.migration_config = config;
        self
    }

    pub async fn build(self) -> RuntimeResult<AppContext> {
        let profile = ProfileBootstrap::get()?;
        let app_paths = Arc::new(AppPaths::from_profile(&profile.paths)?);
        systemprompt_files::FilesConfig::init(&app_paths)?;
        let config = Arc::new(Config::get()?.clone());

        let database = Arc::new(
            Database::from_config_with_write(
                &config.database_type,
                &config.database_url,
                config.database_write_url.as_deref(),
            )
            .await?,
        );

        let authz_audit_pool = database.write_pool_arc().ok();
        let authz_hook = systemprompt_security::authz::build_authz_hook(
            profile.governance.as_ref(),
            authz_audit_pool,
        )
        .map_err(|err| RuntimeError::Internal(format!("authz bootstrap: {err}")))?;

        systemprompt_logging::init_logging(Arc::clone(&database));

        if config.database_write_url.is_some() {
            tracing::debug!(
                "Database read/write separation enabled: reads from replica, writes to primary"
            );
        }

        let api_registry = Arc::new(ModuleApiRegistry::new());

        let registry = match self.extension_registry {
            Some(registry) => registry,
            None => ExtensionRegistry::discover()?,
        };
        registry.validate()?;

        if self.install_schemas {
            install_extension_schemas_full(
                &registry,
                database.write_provider(),
                &[],
                self.migration_config,
            )
            .await?;
        }

        let extension_registry = Arc::new(registry);

        let geoip_reader = AppContext::load_geoip_database(&config, self.show_startup_warnings);
        let content_config = AppContext::load_content_config(&config, &app_paths);
        let content_routing = content_routing_from(content_config.as_ref());
        let route_classifier = Arc::new(systemprompt_models::RouteClassifier::new(
            content_routing.clone(),
        ));
        let analytics_service = Arc::new(AnalyticsService::new(
            &database,
            geoip_reader.clone(),
            content_routing,
        )?);

        let fingerprint_repo = match FingerprintRepository::new(&database) {
            Ok(repo) => Some(Arc::new(repo)),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize fingerprint repository");
                None
            },
        };

        let user_service = match UserService::new(&database) {
            Ok(svc) => Some(Arc::new(svc)),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize user service");
                None
            },
        };

        let system_admin = resolve_and_install_system_admin(&config, user_service.as_ref()).await?;
        let mcp_registry = RegistryManager::new(system_admin.id().clone());

        let marketplace_filter = self
            .marketplace_filter
            .unwrap_or_else(|| build_marketplace_filter(&database));

        let event_bridge = Arc::new(OnceLock::new());

        Ok(AppContext::from_parts(AppContextParts {
            config,
            database,
            api_registry,
            extension_registry,
            geoip_reader,
            content_config,
            route_classifier,
            analytics_service,
            fingerprint_repo,
            user_service,
            app_paths,
            marketplace_filter,
            event_bridge,
            system_admin,
            mcp_registry,
            authz_hook,
        }))
    }
}

async fn resolve_and_install_system_admin(
    config: &Config,
    user_service: Option<&Arc<UserService>>,
) -> RuntimeResult<Arc<SystemAdmin>> {
    let users = user_service.ok_or(RuntimeError::SystemAdminUserServiceUnavailable)?;
    let cfg = SystemAdminConfig {
        username: config.system_admin_username.clone(),
    };
    let resolved = context_loaders::resolve_system_admin(&cfg, users.as_ref()).await?;
    systemprompt_logging::install_log_attribution(resolved.clone());
    Ok(Arc::new(resolved))
}

fn build_marketplace_filter(
    database: &systemprompt_database::DbPool,
) -> Arc<dyn MarketplaceFilter> {
    for reg in discover_filters() {
        match (reg.factory)(database) {
            Ok(filter) => {
                tracing::debug!(
                    priority = reg.priority,
                    "marketplace filter registered via inventory; using highest-priority impl",
                );
                return filter;
            },
            Err(err) => {
                tracing::error!(
                    priority = reg.priority,
                    error = %err,
                    "marketplace filter factory failed; trying next candidate",
                );
            },
        }
    }
    let fallback: Arc<dyn MarketplaceFilter> = Arc::new(AllowAllFilter);
    fallback
}

fn content_routing_from(
    content_config: Option<&Arc<ContentConfigRaw>>,
) -> Option<Arc<dyn ContentRouting>> {
    let concrete = Arc::clone(content_config?);
    let routing: Arc<dyn ContentRouting> = concrete;
    Some(routing)
}
