//! The `with_*` overrides an embedder applies to an [`AppContextBuilder`]
//! before calling [`build`](AppContextBuilder::build).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_governance::authz::{AuthzDecisionHook, SharedAuthzHook};
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::BackgroundTasks;

use super::AppContextBuilder;
use crate::context::ShutdownRequest;

impl AppContextBuilder {
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

    #[must_use]
    pub fn with_marketplace_filter(mut self, filter: Arc<dyn MarketplaceFilter>) -> Self {
        self.marketplace_filter = Some(filter);
        self
    }

    #[must_use]
    pub const fn with_migrations(mut self, install: bool) -> Self {
        self.schema.install = install;
        self
    }

    #[must_use]
    pub const fn with_schema_verification(mut self, verify: bool) -> Self {
        self.schema.verify = verify;
        self
    }

    #[must_use]
    pub fn with_authz_hook<H>(mut self, hook: H) -> Self
    where
        H: AuthzDecisionHook + 'static,
    {
        self.core.authz_hook = Some(Arc::new(hook));
        self
    }

    #[must_use]
    pub fn with_signing_keys(mut self, keys: SigningKeys) -> Self {
        self.core.signing_keys = Some(keys);
        self
    }

    #[must_use]
    pub fn with_shared_authz_hook(mut self, hook: SharedAuthzHook) -> Self {
        self.core.authz_hook = Some(hook);
        self
    }

    #[must_use]
    pub fn with_shutdown(mut self, shutdown: ShutdownRequest) -> Self {
        self.shutdown = Some(shutdown);
        self
    }

    #[must_use]
    pub fn with_background_tasks(mut self, tasks: BackgroundTasks) -> Self {
        self.background_tasks = Some(tasks);
        self
    }

    #[must_use]
    pub fn with_event_router(mut self, router: EventRouter) -> Self {
        self.event_router = Some(router);
        self
    }

    #[must_use]
    pub fn with_database_url(mut self, url: impl Into<String>) -> Self {
        self.core.database_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn with_services(mut self, services: systemprompt_loader::ServicesState) -> Self {
        self.core.services = Some(services);
        self
    }

    #[must_use]
    pub const fn with_migration_config(mut self, config: MigrationConfig) -> Self {
        self.migration_config = config;
        self
    }
}
