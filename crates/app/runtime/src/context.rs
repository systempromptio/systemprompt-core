//! [`AppContext`] — the application-wide runtime container.
//!
//! Holds shared handles (config, database pool, extension registry,
//! analytics, route classifier, etc.) cloned cheaply via [`Arc`].
//! Constructed via [`crate::AppContextBuilder`] or [`AppContext::new`].

use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository, GeoIpReader};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting, RouteClassifier};
use systemprompt_users::UserService;

use crate::builder::AppContextBuilder;
use crate::context_loaders;
use crate::error::RuntimeResult;
use crate::registry::ModuleApiRegistry;

/// Application-wide runtime context.
///
/// Cloning is cheap (every field is an [`Arc`] or [`Option<Arc<_>>`]),
/// so handlers and background tasks should clone the whole context
/// rather than threading individual handles.
#[derive(Clone)]
pub struct AppContext {
    pub(crate) config: Arc<Config>,
    pub(crate) database: DbPool,
    pub(crate) api_registry: Arc<ModuleApiRegistry>,
    pub(crate) extension_registry: Arc<ExtensionRegistry>,
    pub(crate) geoip_reader: Option<GeoIpReader>,
    pub(crate) content_config: Option<Arc<ContentConfigRaw>>,
    pub(crate) route_classifier: Arc<RouteClassifier>,
    pub(crate) analytics_service: Arc<AnalyticsService>,
    pub(crate) fingerprint_repo: Option<Arc<FingerprintRepository>>,
    pub(crate) user_service: Option<Arc<UserService>>,
    pub(crate) app_paths: Arc<AppPaths>,
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("config", &"Config")
            .field("database", &"DbPool")
            .field("api_registry", &"ModuleApiRegistry")
            .field("extension_registry", &self.extension_registry)
            .field("geoip_reader", &self.geoip_reader.is_some())
            .field("content_config", &self.content_config.is_some())
            .field("route_classifier", &"RouteClassifier")
            .field("analytics_service", &"AnalyticsService")
            .field("fingerprint_repo", &self.fingerprint_repo.is_some())
            .field("user_service", &self.user_service.is_some())
            .field("app_paths", &"AppPaths")
            .finish()
    }
}

/// Decomposed form of [`AppContext`] used by [`AppContext::from_parts`].
///
/// Useful for tests and embedders that pre-build individual handles.
#[derive(Debug)]
pub struct AppContextParts {
    /// Application configuration singleton.
    pub config: Arc<Config>,
    /// Database pool (read replica or single primary).
    pub database: DbPool,
    /// Module API route registry built from `inventory` registrations.
    pub api_registry: Arc<ModuleApiRegistry>,
    /// Extension registry resolved from `inventory` and validated.
    pub extension_registry: Arc<ExtensionRegistry>,
    /// Optional `MaxMind` `GeoIP2` reader.
    pub geoip_reader: Option<GeoIpReader>,
    /// Parsed `content.yaml`, when available.
    pub content_config: Option<Arc<ContentConfigRaw>>,
    /// Route classifier driven by the content config.
    pub route_classifier: Arc<RouteClassifier>,
    /// Analytics service handle.
    pub analytics_service: Arc<AnalyticsService>,
    /// Optional fingerprint repository.
    pub fingerprint_repo: Option<Arc<FingerprintRepository>>,
    /// Optional user service.
    pub user_service: Option<Arc<UserService>>,
    /// Resolved application paths.
    pub app_paths: Arc<AppPaths>,
}

impl AppContext {
    /// Build a fully-populated [`AppContext`] using default builder
    /// settings. Equivalent to `Self::builder().build().await`.
    pub async fn new() -> RuntimeResult<Self> {
        Self::builder().build().await
    }

    /// Construct a fresh [`AppContextBuilder`].
    #[must_use]
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder::new()
    }

    /// Assemble an [`AppContext`] from pre-built [`AppContextParts`].
    pub fn from_parts(parts: AppContextParts) -> Self {
        Self {
            config: parts.config,
            database: parts.database,
            api_registry: parts.api_registry,
            extension_registry: parts.extension_registry,
            geoip_reader: parts.geoip_reader,
            content_config: parts.content_config,
            route_classifier: parts.route_classifier,
            analytics_service: parts.analytics_service,
            fingerprint_repo: parts.fingerprint_repo,
            user_service: parts.user_service,
            app_paths: parts.app_paths,
        }
    }

    /// Load the optional `GeoIP2` database referenced by `config`.
    pub fn load_geoip_database(config: &Config, show_warnings: bool) -> Option<GeoIpReader> {
        context_loaders::load_geoip_database(config, show_warnings)
    }

    /// Load the optional `content.yaml` referenced by `app_paths`.
    pub fn load_content_config(
        config: &Config,
        app_paths: &AppPaths,
    ) -> Option<Arc<ContentConfigRaw>> {
        context_loaders::load_content_config(config, app_paths)
    }

    /// Borrow the [`Config`] singleton.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Borrow the parsed `content.yaml`, when present.
    pub fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.content_config.as_ref().map(AsRef::as_ref)
    }

    /// Erased view of the content config as a [`ContentRouting`].
    pub fn content_routing(&self) -> Option<Arc<dyn ContentRouting>> {
        let concrete = Arc::clone(self.content_config.as_ref()?);
        let routing: Arc<dyn ContentRouting> = concrete;
        Some(routing)
    }

    /// Borrow the database pool.
    pub const fn db_pool(&self) -> &DbPool {
        &self.database
    }

    /// Borrow the database pool (alias of [`Self::db_pool`]).
    pub const fn database(&self) -> &DbPool {
        &self.database
    }

    /// Borrow the module API registry.
    pub fn api_registry(&self) -> &ModuleApiRegistry {
        &self.api_registry
    }

    /// Borrow the extension registry.
    pub fn extension_registry(&self) -> &ExtensionRegistry {
        &self.extension_registry
    }

    /// Format the configured `host:port` listen address.
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// JWT audiences provided by this server.
    pub fn get_provided_audiences() -> Vec<String> {
        vec!["a2a".to_string(), "api".to_string(), "mcp".to_string()]
    }

    /// JWT audiences accepted for `_module_name`. Currently identical
    /// to [`Self::get_provided_audiences`].
    pub fn get_valid_audiences(_module_name: &str) -> Vec<String> {
        Self::get_provided_audiences()
    }

    /// JWT audiences accepted for the named MCP server. Currently
    /// identical to [`Self::get_provided_audiences`].
    pub fn get_server_audiences(_server_name: &str, _port: u16) -> Vec<String> {
        Self::get_provided_audiences()
    }

    /// Borrow the optional `GeoIP2` reader.
    pub const fn geoip_reader(&self) -> Option<&GeoIpReader> {
        self.geoip_reader.as_ref()
    }

    /// Borrow the analytics service.
    pub const fn analytics_service(&self) -> &Arc<AnalyticsService> {
        &self.analytics_service
    }

    /// Borrow the route classifier.
    pub const fn route_classifier(&self) -> &Arc<RouteClassifier> {
        &self.route_classifier
    }

    /// Borrow the resolved [`AppPaths`].
    pub fn app_paths(&self) -> &AppPaths {
        &self.app_paths
    }

    /// Borrow the resolved [`AppPaths`] as an [`Arc`].
    pub const fn app_paths_arc(&self) -> &Arc<AppPaths> {
        &self.app_paths
    }
}
