//! Shared `AgentState` handle for the A2A server. It optionally carries the
//! `AgentRunLauncher` an agent configured with `metadata.agent_run` queues
//! its messages through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{DynAgentRunLauncher, DynJwtValidationProvider};

use crate::repository::A2ARepositories;
use crate::services::a2a_server::streaming::webhook_client::DynWebhookBroadcaster;

#[derive(Clone)]
pub struct AgentAuth {
    pub jwt_provider: DynJwtValidationProvider,
    pub signing_keys: SigningKeys,
}

impl std::fmt::Debug for AgentAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentAuth")
            .field("jwt_provider", &"<DynJwtValidationProvider>")
            .field("signing_keys", &self.signing_keys)
            .finish()
    }
}

#[derive(Clone)]
pub struct AgentState {
    db_pool: DbPool,
    config: Arc<Config>,
    auth: AgentAuth,
    repositories: Arc<A2ARepositories>,
    webhooks: DynWebhookBroadcaster,
    services: ServicesView,
    agent_run_launcher: Option<DynAgentRunLauncher>,
}

impl AgentState {
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is an independently injected collaborator; the constructor is the one composition point"
    )]
    pub fn new(
        db_pool: DbPool,
        config: Arc<Config>,
        auth: AgentAuth,
        repositories: Arc<A2ARepositories>,
        webhooks: DynWebhookBroadcaster,
        services: ServicesView,
    ) -> Self {
        Self {
            db_pool,
            config,
            auth,
            repositories,
            webhooks,
            services,
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
        &self.auth.jwt_provider
    }

    #[must_use]
    pub const fn signing_keys(&self) -> &SigningKeys {
        &self.auth.signing_keys
    }

    #[must_use]
    pub const fn services(&self) -> &ServicesView {
        &self.services
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
            .field("auth", &"<AgentAuth>")
            .field("repositories", &"<A2ARepositories>")
            .field("webhooks", &self.webhooks)
            .field("services", &self.services)
            .field("agent_run_launcher", &self.agent_run_launcher.is_some())
            .finish()
    }
}
