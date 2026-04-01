use anyhow::Result;
use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_database::Database;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, ContentRouting, ProfileBootstrap};
use systemprompt_users::UserService;

use crate::context::{AppContext, AppContextParts};
use crate::registry::ModuleApiRegistry;

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
        let profile = ProfileBootstrap::get()?;
        AppPaths::init(&profile.paths)?;
        systemprompt_files::FilesConfig::init()?;
        let config = Arc::new(Config::get()?.clone());

        let database = Arc::new(
            Database::from_config_with_write(
                &config.database_type,
                &config.database_url,
                config.database_write_url.as_deref(),
            )
            .await?,
        );

        systemprompt_logging::init_logging(Arc::clone(&database));

        if config.database_write_url.is_some() {
            tracing::info!(
                "Database read/write separation enabled: reads from replica, writes to primary"
            );
        }

        let api_registry = Arc::new(ModuleApiRegistry::new());

        let registry = self
            .extension_registry
            .unwrap_or_else(ExtensionRegistry::discover);
        registry.validate()?;
        let extension_registry = Arc::new(registry);

        let geoip_reader =
            AppContext::load_geoip_database(&config, self.show_startup_warnings);
        let content_config = AppContext::load_content_config(&config);
        let content_routing = content_routing_from(content_config.as_ref());
        let route_classifier =
            Arc::new(systemprompt_models::RouteClassifier::new(content_routing.clone()));
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
        }))
    }
}

#[allow(trivial_casts)]
fn content_routing_from(
    content_config: Option<&Arc<ContentConfigRaw>>,
) -> Option<Arc<dyn ContentRouting>> {
    content_config.cloned().map(|c| c as Arc<dyn ContentRouting>)
}
