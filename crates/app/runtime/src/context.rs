//! [`AppContext`] — the application-wide runtime container.
//!
//! Holds shared handles (config, database pool, extension registry,
//! analytics, route classifier, etc.) cloned cheaply via [`Arc`].
//! Constructed via [`crate::AppContextBuilder`] or [`AppContext::new`].

use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository, GeoIpReader};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting, RouteClassifier};
use systemprompt_users::UserService;

use crate::builder::AppContextBuilder;
use crate::context_loaders;
use crate::error::RuntimeResult;
use crate::registry::ModuleApiRegistry;

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
    pub(crate) marketplace_filter: Arc<dyn MarketplaceFilter>,
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
            .field("marketplace_filter", &self.marketplace_filter)
            .finish()
    }
}

#[derive(Debug)]
pub struct AppContextParts {
    pub config: Arc<Config>,
    pub database: DbPool,
    pub api_registry: Arc<ModuleApiRegistry>,
    pub extension_registry: Arc<ExtensionRegistry>,
    pub geoip_reader: Option<GeoIpReader>,
    pub content_config: Option<Arc<ContentConfigRaw>>,
    pub route_classifier: Arc<RouteClassifier>,
    pub analytics_service: Arc<AnalyticsService>,
    pub fingerprint_repo: Option<Arc<FingerprintRepository>>,
    pub user_service: Option<Arc<UserService>>,
    pub app_paths: Arc<AppPaths>,
    pub marketplace_filter: Arc<dyn MarketplaceFilter>,
}

impl AppContext {
    pub async fn new() -> RuntimeResult<Self> {
        Self::builder().build().await
    }

    #[must_use]
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder::new()
    }

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
            marketplace_filter: parts.marketplace_filter,
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
        &self.config
    }

    pub fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.content_config.as_ref().map(AsRef::as_ref)
    }

    pub fn content_routing(&self) -> Option<Arc<dyn ContentRouting>> {
        let concrete = Arc::clone(self.content_config.as_ref()?);
        let routing: Arc<dyn ContentRouting> = concrete;
        Some(routing)
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.database
    }

    pub const fn database(&self) -> &DbPool {
        &self.database
    }

    pub fn api_registry(&self) -> &ModuleApiRegistry {
        &self.api_registry
    }

    pub fn extension_registry(&self) -> &ExtensionRegistry {
        &self.extension_registry
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    pub const fn geoip_reader(&self) -> Option<&GeoIpReader> {
        self.geoip_reader.as_ref()
    }

    pub const fn analytics_service(&self) -> &Arc<AnalyticsService> {
        &self.analytics_service
    }

    pub const fn route_classifier(&self) -> &Arc<RouteClassifier> {
        &self.route_classifier
    }

    pub fn app_paths(&self) -> &AppPaths {
        &self.app_paths
    }

    pub const fn app_paths_arc(&self) -> &Arc<AppPaths> {
        &self.app_paths
    }

    pub fn marketplace_filter(&self) -> &Arc<dyn MarketplaceFilter> {
        &self.marketplace_filter
    }
}
