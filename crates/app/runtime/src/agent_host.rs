//! [`AgentHost`]: what a spawned agent runs on.
//!
//! An agent is a least-privilege child of the API, booted on the same
//! [`HostContext`] as a managed MCP server: the profile path, the database
//! URLs, the public verification key set and its granted secrets. On top of
//! that it composes the agent's own collaborators: the A2A repositories, the
//! MCP registry, an [`AiService`] whose provider and connector credentials
//! resolve through the granted secrets alone, and a verify-only JWT provider.
//! It holds no signing key, token authority, OAuth pepper or manifest seed, and
//! constructs no OAuth repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::AgentAuth;
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::services::ContextProviderService;
use systemprompt_ai::error::AiError;
use systemprompt_ai::repository::AiRequestRepositories;
use systemprompt_ai::{AiService, AiServiceProviders};
use systemprompt_config::GrantedSecrets;
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_mcp::McpToolProvider;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::{DynSessionUsageCounters, SecretLookup};
use systemprompt_users::{SessionRepository, UsersAiSessionProvider};

use crate::builder::composition::{a2a_repositories, organization_skills};
use crate::host::{HostBootError, HostContext};

#[derive(Debug, thiserror::Error)]
pub enum AgentHostError {
    #[error(transparent)]
    Boot(Box<HostBootError>),
    #[error("the agent's AI service could not be built")]
    Ai(#[source] Box<AiError>),
}

impl From<HostBootError> for AgentHostError {
    fn from(source: HostBootError) -> Self {
        Self::Boot(Box::new(source))
    }
}

/// The least-privilege composition a spawned agent serves from.
#[derive(Clone)]
pub struct AgentHost {
    host: HostContext,
    a2a: Arc<A2ARepositories>,
    ai: Arc<AiService>,
    auth: AgentAuth,
}

impl std::fmt::Debug for AgentHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHost")
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}

impl AgentHost {
    pub async fn from_process_env() -> Result<Self, AgentHostError> {
        Self::compose(HostContext::from_process_env().await?)
    }

    pub fn compose(host: HostContext) -> Result<Self, AgentHostError> {
        let database = host.db_pool();
        let config = Arc::new(host.config().clone());
        let system_admin = host.system_owner().clone();

        let managed = ManagedRepository::new(database)
            .with_publication_policy(host.publication_policy().clone());
        let session_usage: DynSessionUsageCounters = Arc::new(SessionRepository::new(database));
        let a2a = a2a_repositories(
            database,
            config.instance_id.clone(),
            &managed,
            session_usage,
        );
        let a2a = Arc::new(organization_skills(&a2a, &managed, &system_admin));

        let registry = RegistryService::new(
            system_admin,
            host.services_state().view(),
            Arc::clone(&config),
        );
        let services = host.services_state().config();
        let connector_tokens: Arc<TokenCache> = Arc::default();
        let secrets: Arc<dyn SecretLookup> = Arc::<GrantedSecrets>::clone(host.granted_secrets());
        let tools = Arc::new(McpToolProvider::new(
            Arc::clone(database),
            registry,
            &services.ai.mcp.resilience,
            Arc::clone(&connector_tokens),
            Arc::clone(&secrets),
        ));
        let ai = AiService::new(
            Arc::clone(host.services_state().catalog()),
            &services.ai,
            AiServiceProviders {
                tools,
                sessions: Arc::new(UsersAiSessionProvider::from_repository(
                    SessionRepository::new(database),
                )),
                provider_tokens: connector_tokens,
                secrets,
                session_ttl_seconds: config.jwt_access_token_expiration,
            },
            &AiRequestRepositories::new(database),
        )
        .map_err(|source| AgentHostError::Ai(Box::new(source)))?
        .with_context_materializer(Arc::new(ContextProviderService::new(a2a.contexts.clone())));

        let auth = AgentAuth {
            jwt_provider: Arc::new(JwtValidationProviderImpl::from_config(
                &config,
                host.verification_keys().clone(),
            )),
            verification_keys: host.verification_keys().clone(),
        };
        Ok(Self {
            host,
            a2a,
            ai: Arc::new(ai),
            auth,
        })
    }

    #[must_use]
    pub const fn host(&self) -> &HostContext {
        &self.host
    }

    #[must_use]
    pub const fn a2a_repositories(&self) -> &Arc<A2ARepositories> {
        &self.a2a
    }

    #[must_use]
    pub const fn ai_service(&self) -> &Arc<AiService> {
        &self.ai
    }

    #[must_use]
    pub fn agent_auth(&self) -> AgentAuth {
        self.auth.clone()
    }
}
