use crate::registry::ModuleApiRegistry;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_analytics::{AnalyticsService, GeoIpReader};
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_extension::{Extension, ExtensionContext, ExtensionRegistry};
use systemprompt_models::{Config, ContentConfigRaw, ContentRouting, RouteClassifier, SystemPaths};
use systemprompt_traits::{AppContext as AppContextTrait, ConfigProvider, DatabaseHandle};

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
            .finish()
    }
}

impl AppContext {
    pub async fn new() -> Result<Self> {
        Self::builder().build().await
    }

    #[must_use]
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder::new()
    }

    async fn new_internal(extension_registry: Option<ExtensionRegistry>) -> Result<Self> {
        systemprompt_models::PathConfig::init()?;
        systemprompt_core_files::FilesConfig::init()?;
        let config = Arc::new(Config::get()?.clone());
        let database =
            Arc::new(Database::from_config(&config.database_type, &config.database_url).await?);

        let api_registry = Arc::new(ModuleApiRegistry::new());

        let registry = extension_registry.unwrap_or_else(ExtensionRegistry::discover);

        registry.validate()?;

        let extension_registry = Arc::new(registry);

        let geoip_reader = Self::load_geoip_database(&config);
        let content_config = Self::load_content_config(&config);

        #[allow(trivial_casts)]
        let content_routing: Option<Arc<dyn ContentRouting>> =
            content_config.clone().map(|c| c as Arc<dyn ContentRouting>);

        let route_classifier = Arc::new(RouteClassifier::new(content_routing.clone()));

        let analytics_service = Arc::new(AnalyticsService::new(
            Arc::clone(&database),
            geoip_reader.clone(),
            content_routing,
        ));

        Ok(Self {
            config,
            database,
            api_registry,
            extension_registry,
            geoip_reader,
            content_config,
            route_classifier,
            analytics_service,
        })
    }

    fn load_geoip_database(config: &Config) -> Option<GeoIpReader> {
        let Some(geoip_path) = &config.geoip_database_path else {
            CliService::warning(
                "GeoIP database not configured - geographic data will not be available",
            );
            CliService::info("  To enable geographic data:");
            CliService::info("  1. Download MaxMind GeoLite2-City database from: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data");
            CliService::info(
                "  2. Add paths.geoip_database to your profile pointing to the .mmdb file",
            );
            return None;
        };

        match maxminddb::Reader::open_readfile(geoip_path) {
            Ok(reader) => Some(Arc::new(reader)),
            Err(e) => {
                CliService::warning(&format!(
                    "Could not load GeoIP database from {geoip_path}: {e}"
                ));
                CliService::info("  Geographic data (country/region/city) will not be available.");
                CliService::info(
                    "  To fix: Ensure the path is correct and the file is a valid MaxMind .mmdb \
                     database",
                );
                None
            },
        }
    }

    fn load_content_config(config: &Config) -> Option<Arc<ContentConfigRaw>> {
        let content_config_path = SystemPaths::content_config(config);

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
            Ok(content_cfg) => Some(Arc::new(content_cfg)),
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

impl AppContextTrait for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        let config = Arc::clone(&self.config);
        config
    }

    fn database_handle(&self) -> Arc<dyn DatabaseHandle> {
        let db = Arc::clone(&self.database);
        db
    }
}

impl ExtensionContext for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        let config = Arc::clone(&self.config);
        config
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        let db = Arc::clone(&self.database);
        db
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extension_registry.get(id).cloned()
    }
}

#[derive(Debug, Default)]
pub struct AppContextBuilder {
    extension_registry: Option<ExtensionRegistry>,
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

    pub async fn build(self) -> Result<AppContext> {
        AppContext::new_internal(self.extension_registry).await
    }
}
