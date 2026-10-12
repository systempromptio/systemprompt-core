//! Shared `AgentState` handle for the A2A server. It optionally carries the
//! `AgentRunLauncher` an agent configured with `metadata.agent_run` queues
//! its messages through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_manifest::Config;
use systemprompt_traits::{DynAgentRunLauncher, DynJwtValidationProvider};

use crate::repository::A2ARepositories;
use crate::services::a2a_server::streaming::webhook_client::DynWebhookBroadcaster;

#[derive(Clone)]
pub struct AgentState {
    db_pool: DbPool,
    config: Arc<Config>,
    jwt_provider: DynJwtValidationProvider,
    repositories: Arc<A2ARepositories>,
    webhooks: DynWebhookBroadcaster,
    agent_run_launcher: Option<DynAgentRunLauncher>,
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
            agent_run_launcher: None,
        }
    }

    #[must_use]
    pub fn with_agent_run_launcher(mut self, launcher: DynAgentRunLauncher) -> Self {
        self.agent_run_launcher = Some(launcher);
        self
    }

    #[must_use]
    pub const fn agent_run_launcher(&self) -> Option<&DynAgentRunLauncher> {
        self.agent_run_launcher.as_ref()
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
            .field("agent_run_launcher", &self.agent_run_launcher.is_some())
            .finish()
    }
}
