//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services. Failures at
//! any step propagate as [`RuntimeError`](crate::error::RuntimeError).
//! Subsystem resolution helpers live in [`assembly`].

mod assembly;

use std::sync::{Arc, OnceLock};

use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{Database, MigrationConfig, install_extension_schemas_full};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::{AppPaths, Config};
use systemprompt_security::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, Plugins, Subsystems};
use crate::error::{RuntimeError, RuntimeResult};
use crate::registry::ModuleApiRegistry;

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
            .finish()
    }
}

struct CoreLayer {
    config: Arc<Config>,
    app_paths: Arc<AppPaths>,
    database: Arc<Database>,
    authz_hook: SharedAuthzHook,
}

impl AppContextBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies an explicit extension registry. When unset, `build()`
    /// discovers extensions via inventory ([`ExtensionRegistry::discover`]).
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

    /// Supplies an explicit marketplace filter. When unset, `build()` selects
    /// the highest-priority inventory-registered filter, falling back to an
    /// allow-all filter when none succeeds.
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

    /// Supplies an extension-built authz decision hook. The hook is wired
    /// only when `profile.governance.authz.hook.mode = extension`; pairing
    /// this call with any other mode is a bootstrap error.
    #[must_use]
    pub fn with_authz_hook<H>(mut self, hook: H) -> Self
    where
        H: AuthzDecisionHook + 'static,
    {
        self.authz_hook = Some(Arc::new(hook));
        self
    }

    /// Variant of [`Self::with_authz_hook`] for callers that already hold an
    /// `Arc<dyn AuthzDecisionHook>` (e.g. a pre-built [`CompositeAuthzHook`]
    /// shared across consumers).
    ///
    /// [`CompositeAuthzHook`]: systemprompt_security::authz::CompositeAuthzHook
    #[must_use]
    pub fn with_shared_authz_hook(mut self, hook: SharedAuthzHook) -> Self {
        self.authz_hook = Some(hook);
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
        } = init_core(self.authz_hook).await?;

        let api_registry = Arc::new(ModuleApiRegistry::new());
        let extension_registry = init_extensions(
            self.extension_registry,
            self.install_schemas,
            self.migration_config,
            &database,
        )
        .await?;

        let geoip_reader = AppContext::load_geoip_database(&config, self.show_startup_warnings);
        let content_config = AppContext::load_content_config(&config, &app_paths);
        let content_routing = assembly::content_routing_from(content_config.as_ref());
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

        // UserService is a mandatory dependency: the system admin cannot be
        // resolved without it, so a construction failure is fatal here rather
        // than a warning that re-surfaces as a less specific error downstream.
        let user_service = Arc::new(UserService::new(&database)?);

        let system_admin =
            assembly::resolve_and_install_system_admin(&config, &user_service).await?;
        let mcp_registry = RegistryService::new(system_admin.id().clone());

        let marketplace_filter = self
            .marketplace_filter
            .unwrap_or_else(|| assembly::build_marketplace_filter(&database));

        let event_bridge = Arc::new(OnceLock::new());

        Ok(AppContext::from_parts(
            DataPlane {
                database,
                analytics_service,
                fingerprint_repo,
                user_service: Some(user_service),
            },
            ConfigPlane {
                config,
                app_paths,
                content_config,
                route_classifier,
            },
            Plugins {
                extension_registry,
                api_registry,
                mcp_registry,
                marketplace_filter,
            },
            Subsystems {
                system_admin,
                authz_hook,
                event_bridge,
                geoip_reader,
            },
        ))
    }
}

/// Bootstraps profile, paths, files, config, database, authz, and logging.
///
/// The path/files/config inits are idempotent `OnceLock` guards, so a non-CLI
/// entry (API, tests) can build a context self-sufficiently while a CLI that
/// already ran them sees a no-op.
async fn init_core(authz_hook_override: Option<SharedAuthzHook>) -> RuntimeResult<CoreLayer> {
    let profile = ProfileBootstrap::get()?;
    let app_paths = Arc::new(AppPaths::from_profile(&profile.paths)?);
    systemprompt_files::FilesConfig::init(&app_paths)?;
    systemprompt_config::try_init_config()
        .map_err(|err| RuntimeError::Internal(format!("config init: {err}")))?;
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
        authz_hook_override,
    )
    .map_err(|err| RuntimeError::Internal(format!("authz bootstrap: {err}")))?;

    systemprompt_logging::init_logging(Arc::clone(&database));

    if config.database_write_url.is_some() {
        tracing::debug!(
            "Database read/write separation enabled: reads from replica, writes to primary"
        );
    }

    Ok(CoreLayer {
        config,
        app_paths,
        database,
        authz_hook,
    })
}

async fn init_extensions(
    extension_registry: Option<ExtensionRegistry>,
    install_schemas: bool,
    migration_config: MigrationConfig,
    database: &Arc<Database>,
) -> RuntimeResult<Arc<ExtensionRegistry>> {
    let registry = match extension_registry {
        Some(registry) => registry,
        None => ExtensionRegistry::discover()?,
    };
    registry.validate()?;

    if install_schemas {
        install_extension_schemas_full(&registry, database.write(), &[], migration_config).await?;
    }

    Ok(Arc::new(registry))
}
