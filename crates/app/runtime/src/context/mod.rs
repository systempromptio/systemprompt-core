//! [`AppContext`] — the application-wide runtime container.
//!
//! Holds shared handles (config, database pool, extension registry,
//! analytics, route classifier, etc.) cloned cheaply via [`Arc`].
//! Constructed via [`crate::AppContextBuilder`] or [`AppContext::new`].

use std::sync::{Arc, OnceLock};

use tokio::task::JoinHandle;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository, GeoIpReader};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::services::SystemAdmin;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting, RouteClassifier};
use systemprompt_security::authz::SharedAuthzHook;
use systemprompt_users::UserService;

mod context_loaders;

use crate::builder::AppContextBuilder;
use crate::error::RuntimeResult;
use crate::registry::ModuleApiRegistry;

/// Database pool and the data-access services layered on it.
///
/// `fingerprint_repo` and `user_service` are `None` when the corresponding
/// resource failed to initialise; callers must degrade gracefully.
#[derive(Clone)]
pub struct DataPlane {
    pub database: DbPool,
    pub analytics_service: Arc<AnalyticsService>,
    pub fingerprint_repo: Option<Arc<FingerprintRepository>>,
    pub user_service: Option<Arc<UserService>>,
}

/// Resolved configuration, on-disk paths, and the routing derived from them.
///
/// `content_config` is `None` when no content configuration is present.
#[derive(Clone)]
pub struct ConfigPlane {
    pub config: Arc<Config>,
    pub app_paths: Arc<AppPaths>,
    pub content_config: Option<Arc<ContentConfigRaw>>,
    pub route_classifier: Arc<RouteClassifier>,
}

/// Extension, module-API, MCP, and marketplace registries.
#[derive(Clone)]
pub struct Plugins {
    pub extension_registry: Arc<ExtensionRegistry>,
    pub api_registry: Arc<ModuleApiRegistry>,
    pub mcp_registry: RegistryService,
    pub marketplace_filter: Arc<dyn MarketplaceFilter>,
}

/// Cross-cutting runtime subsystems: admin identity, authz hook, the event
/// bridge handle, and the optional `GeoIP` reader.
#[derive(Clone)]
pub struct Subsystems {
    pub system_admin: Arc<SystemAdmin>,
    pub authz_hook: SharedAuthzHook,
    pub event_bridge: Arc<OnceLock<JoinHandle<()>>>,
    pub geoip_reader: Option<GeoIpReader>,
}

/// Application-wide runtime container shared across the HTTP server, the
/// scheduler, and CLI commands.
///
/// Handles are grouped into four cohesive planes ([`DataPlane`],
/// [`ConfigPlane`], [`Plugins`], [`Subsystems`]); each field is an [`Arc`] (or
/// an `Arc`-internal handle such as [`DbPool`]), so `clone` is a
/// reference-count bump rather than a deep copy. Construct it via
/// [`AppContext::builder`] (or [`AppContext::new`] for the default build);
/// [`AppContext::from_parts`] bypasses the bootstrap and is intended for tests
/// and embedders that assemble the planes themselves. Read individual handles
/// through the accessor methods.
#[derive(Clone)]
pub struct AppContext {
    pub(crate) data: DataPlane,
    pub(crate) cfg: ConfigPlane,
    pub(crate) plugins: Plugins,
    pub(crate) subsystems: Subsystems,
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("config", &"Config")
            .field("database", &"DbPool")
            .field("api_registry", &"ModuleApiRegistry")
            .field("extension_registry", &self.plugins.extension_registry)
            .field("geoip_reader", &self.subsystems.geoip_reader.is_some())
            .field("content_config", &self.cfg.content_config.is_some())
            .field("route_classifier", &"RouteClassifier")
            .field("analytics_service", &"AnalyticsService")
            .field("fingerprint_repo", &self.data.fingerprint_repo.is_some())
            .field("user_service", &self.data.user_service.is_some())
            .field("app_paths", &"AppPaths")
            .field("marketplace_filter", &self.plugins.marketplace_filter)
            .field(
                "event_bridge",
                &self.subsystems.event_bridge.get().is_some(),
            )
            .field("system_admin", &self.subsystems.system_admin.username())
            .field("mcp_registry", &"RegistryService")
            .field("authz_hook", &"SharedAuthzHook")
            .finish()
    }
}

impl std::fmt::Debug for DataPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataPlane")
            .field("database", &"DbPool")
            .field("analytics_service", &"AnalyticsService")
            .field("fingerprint_repo", &self.fingerprint_repo.is_some())
            .field("user_service", &self.user_service.is_some())
            .finish()
    }
}

impl std::fmt::Debug for ConfigPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigPlane")
            .field("config", &"Config")
            .field("app_paths", &"AppPaths")
            .field("content_config", &self.content_config.is_some())
            .field("route_classifier", &"RouteClassifier")
            .finish()
    }
}

impl std::fmt::Debug for Plugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugins")
            .field("extension_registry", &self.extension_registry)
            .field("api_registry", &"ModuleApiRegistry")
            .field("mcp_registry", &"RegistryService")
            .field("marketplace_filter", &self.marketplace_filter)
            .finish()
    }
}

impl std::fmt::Debug for Subsystems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subsystems")
            .field("system_admin", &self.system_admin.username())
            .field("authz_hook", &"SharedAuthzHook")
            .field("event_bridge", &self.event_bridge.get().is_some())
            .field("geoip_reader", &self.geoip_reader.is_some())
            .finish()
    }
}

impl AppContext {
    /// Builds a context with default settings: schema installation off,
    /// extensions discovered via inventory, and the inventory-registered
    /// marketplace filter. Equivalent to `Self::builder().build().await`.
    pub async fn new() -> RuntimeResult<Self> {
        Self::builder().build().await
    }

    #[must_use]
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder::new()
    }

    /// Assembles a context directly from pre-built planes, bypassing the
    /// [`AppContextBuilder`] bootstrap. Intended for tests and embedders that
    /// own the construction of the individual handles.
    #[must_use]
    pub const fn from_parts(
        data: DataPlane,
        cfg: ConfigPlane,
        plugins: Plugins,
        subsystems: Subsystems,
    ) -> Self {
        Self {
            data,
            cfg,
            plugins,
            subsystems,
        }
    }

    pub fn load_geoip_database(config: &Config, show_warnings: bool) -> Option<GeoIpReader> {
        context_loaders::load_geoip_database(config, show_warnings)
    }

    pub fn load_content_config(
        config: &Config,
        app_paths: &AppPaths,
    ) -> Option<Arc<ContentConfigRaw>> {
        context_loaders::load_content_config(config, app_paths)
    }

    pub fn config(&self) -> &Config {
        &self.cfg.config
    }

    pub fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.cfg.content_config.as_ref().map(AsRef::as_ref)
    }

    pub fn content_routing(&self) -> Option<Arc<dyn ContentRouting>> {
        let concrete = Arc::clone(self.cfg.content_config.as_ref()?);
        let routing: Arc<dyn ContentRouting> = concrete;
        Some(routing)
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.data.database
    }

    pub fn api_registry(&self) -> &ModuleApiRegistry {
        &self.plugins.api_registry
    }

    pub fn extension_registry(&self) -> &ExtensionRegistry {
        &self.plugins.extension_registry
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.cfg.config.host, self.cfg.config.port)
    }

    pub const fn geoip_reader(&self) -> Option<&GeoIpReader> {
        self.subsystems.geoip_reader.as_ref()
    }

    pub const fn analytics_service(&self) -> &Arc<AnalyticsService> {
        &self.data.analytics_service
    }

    pub const fn route_classifier(&self) -> &Arc<RouteClassifier> {
        &self.cfg.route_classifier
    }

    pub fn app_paths(&self) -> &AppPaths {
        &self.cfg.app_paths
    }

    pub const fn app_paths_arc(&self) -> &Arc<AppPaths> {
        &self.cfg.app_paths
    }

    pub fn marketplace_filter(&self) -> &Arc<dyn MarketplaceFilter> {
        &self.plugins.marketplace_filter
    }

    pub const fn event_bridge(&self) -> &Arc<OnceLock<JoinHandle<()>>> {
        &self.subsystems.event_bridge
    }

    pub fn system_admin(&self) -> &SystemAdmin {
        &self.subsystems.system_admin
    }

    pub const fn mcp_registry(&self) -> &RegistryService {
        &self.plugins.mcp_registry
    }

    pub const fn authz_hook(&self) -> &SharedAuthzHook {
        &self.subsystems.authz_hook
    }
}
