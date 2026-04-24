use crate::builder::AppContextBuilder;
use crate::registry::ModuleApiRegistry;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository, GeoIpReader};
use systemprompt_database::DbPool;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionRegistry, HasAnalytics, HasFingerprint,
    HasRouteClassifier, HasUserService,
};
use systemprompt_logging::CliService;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting, RouteClassifier};
use systemprompt_traits::{
    AnalyticsProvider, AppContext as AppContextTrait, ConfigProvider, DatabaseHandle,
    FingerprintProvider, UserProvider,
};
use systemprompt_users::UserService;

#[derive(Clone)]
pub struct AppContext {
    config: Arc<Config>,
    database: DbPool,
    api_registry: Arc<ModuleApiRegistry>,
    extension_registry: Arc<ExtensionRegistry>,
    geoip_reader: Option<GeoIpReader>,
    content_config: Option<Arc<ContentConfigRaw>>,
    route_classifier: Arc<RouteClassifier>,
    analytics_service: Arc<AnalyticsService>,
    fingerprint_repo: Option<Arc<FingerprintRepository>>,
    user_service: Option<Arc<UserService>>,
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
}

impl AppContext {
    pub async fn new() -> Result<Self> {
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
        }
    }

    #[cfg(feature = "geolocation")]
    pub fn load_geoip_database(config: &Config, show_warnings: bool) -> Option<GeoIpReader> {
        let Some(geoip_path) = &config.geoip_database_path else {
            if show_warnings {
                CliService::warning(
                    "GeoIP database not configured - geographic data will not be available",
                );
                CliService::info("  To enable geographic data:");
                CliService::info("  1. Download MaxMind GeoLite2-City database from: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data");
                CliService::info(
                    "  2. Add paths.geoip_database to your profile pointing to the .mmdb file",
                );
            }
            return None;
        };

        match maxminddb::Reader::open_readfile(geoip_path) {
            Ok(reader) => Some(Arc::new(reader)),
            Err(e) => {
                if show_warnings {
                    CliService::warning(&format!(
                        "Could not load GeoIP database from {geoip_path}: {e}"
                    ));
                    CliService::info(
                        "  Geographic data (country/region/city) will not be available.",
                    );
                    CliService::info(
                        "  To fix: Ensure the path is correct and the file is a valid MaxMind \
                         .mmdb database",
                    );
                }
                None
            },
        }
    }

    #[cfg(not(feature = "geolocation"))]
    pub fn load_geoip_database(_config: &Config, _show_warnings: bool) -> Option<GeoIpReader> {
        None
    }

    pub fn load_content_config(config: &Config) -> Option<Arc<ContentConfigRaw>> {
        let content_config_path = AppPaths::get()
            .ok()?
            .system()
            .content_config()
            .to_path_buf();

        if !content_config_path.exists() {
            CliService::warning(&format!(
                "Content config not found at: {}",
                content_config_path.display()
            ));
            CliService::info("  Landing page detection will not be available.");
            return None;
        }

        let yaml_content = match std::fs::read_to_string(&content_config_path) {
            Ok(c) => c,
            Err(e) => {
                CliService::warning(&format!(
                    "Could not read content config from {}: {}",
                    content_config_path.display(),
                    e
                ));
                CliService::info("  Landing page detection will not be available.");
                return None;
            },
        };

        match serde_yaml::from_str::<ContentConfigRaw>(&yaml_content) {
            Ok(mut content_cfg) => {
                let base_url = config.api_external_url.trim_end_matches('/');

                content_cfg.metadata.structured_data.organization.url = base_url.to_string();

                let logo = &content_cfg.metadata.structured_data.organization.logo;
                if logo.starts_with('/') {
                    content_cfg.metadata.structured_data.organization.logo =
                        format!("{base_url}{logo}");
                }

                Some(Arc::new(content_cfg))
            },
            Err(e) => {
                CliService::warning(&format!(
                    "Could not parse content config from {}: {}",
                    content_config_path.display(),
                    e
                ));
                CliService::info("  Landing page detection will not be available.");
                None
            },
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.content_config.as_ref().map(AsRef::as_ref)
    }

    #[allow(trivial_casts)]
    pub fn content_routing(&self) -> Option<Arc<dyn ContentRouting>> {
        self.content_config
            .clone()
            .map(|c| c as Arc<dyn ContentRouting>)
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

    pub fn get_provided_audiences() -> Vec<String> {
        vec!["a2a".to_string(), "api".to_string(), "mcp".to_string()]
    }

    pub fn get_valid_audiences(_module_name: &str) -> Vec<String> {
        Self::get_provided_audiences()
    }

    pub fn get_server_audiences(_server_name: &str, _port: u16) -> Vec<String> {
        Self::get_provided_audiences()
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
}

#[allow(trivial_casts)]
impl AppContextTrait for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::clone(&self.config) as _
    }

    fn database_handle(&self) -> Arc<dyn DatabaseHandle> {
        Arc::clone(&self.database) as _
    }

    fn analytics_provider(&self) -> Option<Arc<dyn AnalyticsProvider>> {
        Some(Arc::clone(&self.analytics_service) as _)
    }

    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>> {
        let repo = self.fingerprint_repo.as_ref()?;
        Some(Arc::clone(repo) as _)
    }

    fn user_provider(&self) -> Option<Arc<dyn UserProvider>> {
        let service = self.user_service.as_ref()?;
        Some(Arc::clone(service) as _)
    }
}

#[allow(trivial_casts)]
impl ExtensionContext for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        Arc::clone(&self.config) as _
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        Arc::clone(&self.database) as _
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extension_registry.get(id).cloned()
    }
}

impl HasAnalytics for AppContext {
    type Analytics = Arc<AnalyticsService>;

    fn analytics(&self) -> &Self::Analytics {
        &self.analytics_service
    }
}

impl HasFingerprint for AppContext {
    type Fingerprint = Arc<FingerprintRepository>;

    fn fingerprint(&self) -> Option<&Self::Fingerprint> {
        self.fingerprint_repo.as_ref()
    }
}

impl HasUserService for AppContext {
    type UserService = Arc<UserService>;

    fn user_service(&self) -> Option<&Self::UserService> {
        self.user_service.as_ref()
    }
}

impl HasRouteClassifier for AppContext {
    type RouteClassifier = Arc<RouteClassifier>;

    fn route_classifier(&self) -> &Self::RouteClassifier {
        &self.route_classifier
    }
}
