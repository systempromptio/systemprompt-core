//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services. Failures at
//! any step propagate as [`RuntimeError`](crate::error::RuntimeError).
//! Subsystem resolution helpers live in [`assembly`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assembly;
mod core_layer;

use std::sync::{Arc, OnceLock};

use systemprompt_database::MigrationConfig;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, Plugins, Subsystems};
use crate::error::RuntimeResult;
use crate::registry::ModuleApiRegistry;
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

        let assembly::ContentAnalytics {
            geoip_reader,
            content_config,
            route_classifier,
            analytics_service,
            fingerprint_repo,
        } = assembly::assemble_content_analytics(
            &config,
            &app_paths,
            &database,
            self.show_startup_warnings,
        )?;

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
