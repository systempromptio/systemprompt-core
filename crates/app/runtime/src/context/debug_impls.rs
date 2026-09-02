//! Hand-written `Debug` implementations for the application context planes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppContext, ConfigPlane, DataPlane, Plugins, Subsystems};

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
            .field("a2a_repositories", &"A2ARepositories")
            .field("content_repositories", &"ContentRepositories")
            .field("oauth_repositories", &"OAuthRepositories")
            .field("user_repository", &"UserRepository")
            .field("service_repository", &"ServiceRepository")
            .field("ai_repositories", &"AiRepositories")
            .field("analytics_repositories", &"AnalyticsRepositories")
            .field("file_repository", &"FileRepository")
            .field("mcp_session_repository", &"McpSessionRepository")
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
            .field("file_storage", &"FileStorage")
            .finish()
    }
}
