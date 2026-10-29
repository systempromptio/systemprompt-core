//! The [`Debug`] view of an [`AppContextBuilder`]: which overrides are set,
//! never their values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::AppContextBuilder;

impl std::fmt::Debug for AppContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContextBuilder")
            .field("boot", &self.boot)
            .field("extension_registry", &self.extension_registry.is_some())
            .field("show_startup_warnings", &self.show_startup_warnings)
            .field("marketplace_filter", &self.marketplace_filter.is_some())
            .field("authz_hook", &self.core.authz_hook.is_some())
            .field("signing_keys", &self.core.signing_keys.is_some())
            .field("install_schemas", &self.schema.install)
            .field("verify_schema", &self.schema.verify)
            .field("database_url_override", &self.core.database_url.is_some())
            .field("migration_config", &self.migration_config)
            .field("shutdown", &self.shutdown.is_some())
            .field("background_tasks", &self.background_tasks.is_some())
            .field("event_router", &self.event_router)
            .field("services", &self.core.services.is_some())
            .finish()
    }
}
