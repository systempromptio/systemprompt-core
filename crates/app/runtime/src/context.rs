use crate::registry::ModuleApiRegistry;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_analytics::{AnalyticsService, GeoIpReader};
use systemprompt_database::{Database, DbPool};
use systemprompt_extension::{Extension, ExtensionContext, ExtensionRegistry};
use systemprompt_logging::CliService;
use systemprompt_models::{
    AppPaths, Config, ContentConfigRaw, ContentRouting, ProfileBootstrap, RouteClassifier,
};
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

    async fn new_internal(
        extension_registry: Option<ExtensionRegistry>,
        show_startup_warnings: bool,
    ) -> Result<Self> {
        let profile = ProfileBootstrap::get()?;
        AppPaths::init(&profile.paths)?;
        systemprompt_files::FilesConfig::init()?;
        let config = Arc::new(Config::get()?.clone());
        let database =
            Arc::new(Database::from_config(&config.database_type, &config.database_url).await?);

        let api_registry = Arc::new(ModuleApiRegistry::new());

        let injected = systemprompt_extension::runtime_config::get_injected_extensions();

        let registry = match extension_registry {
            Some(r) => r,
            None if injected.is_empty() => ExtensionRegistry::discover(),
            None => ExtensionRegistry::discover_and_merge(injected)?,
        };

        registry.validate()?;

        let extension_registry = Arc::new(registry);

        let geoip_reader = Self::load_geoip_database(&config, show_startup_warnings);
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

        systemprompt_logging::init_logging(Arc::clone(&database));

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

    fn load_geoip_database(config: &Config, show_warnings: bool) -> Option<GeoIpReader> {
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

    fn load_content_config(config: &Config) -> Option<Arc<ContentConfigRaw>> {
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

#[allow(clippy::clone_on_ref_ptr)]
impl AppContextTrait for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        self.config.clone()
    }

    fn database_handle(&self) -> Arc<dyn DatabaseHandle> {
        self.database.clone()
    }
}

#[allow(clippy::clone_on_ref_ptr)]
impl ExtensionContext for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        self.config.clone()
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        self.database.clone()
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extension_registry.get(id).cloned()
    }
}

#[derive(Debug, Default)]
pub struct AppContextBuilder {
    extension_registry: Option<ExtensionRegistry>,
    show_startup_warnings: bool,
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

    pub async fn build(self) -> Result<AppContext> {
        AppContext::new_internal(self.extension_registry, self.show_startup_warnings).await
    }
}
