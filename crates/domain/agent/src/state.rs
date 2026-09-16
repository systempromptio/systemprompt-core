//! Shared `AgentState` handle for the A2A server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::Config;
use systemprompt_traits::DynJwtValidationProvider;

use crate::repository::A2ARepositories;
use crate::services::a2a_server::streaming::webhook_client::DynWebhookBroadcaster;

#[derive(Clone)]
pub struct AgentState {
    db_pool: DbPool,
    config: Arc<Config>,
    jwt_provider: DynJwtValidationProvider,
    repositories: Arc<A2ARepositories>,
    webhooks: DynWebhookBroadcaster,
}

impl AgentState {
    #[must_use]
    pub fn new(
        db_pool: DbPool,
        config: Arc<Config>,
        jwt_provider: DynJwtValidationProvider,
        repositories: Arc<A2ARepositories>,
        webhooks: DynWebhookBroadcaster,
    ) -> Self {
        Self {
            db_pool,
            config,
            jwt_provider,
            repositories,
            webhooks,
        }
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn jwt_provider(&self) -> &DynJwtValidationProvider {
        &self.jwt_provider
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<A2ARepositories> {
        &self.repositories
    }

    #[must_use]
    pub fn webhooks(&self) -> DynWebhookBroadcaster {
        Arc::clone(&self.webhooks)
    }
}

impl std::fmt::Debug for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentState")
            .field("db_pool", &"<DbPool>")
            .field("config", &"<Arc<Config>>")
            .field("jwt_provider", &"<DynJwtValidationProvider>")
            .field("repositories", &"<A2ARepositories>")
            .field("webhooks", &self.webhooks)
            .finish()
    }
}
