//! [`HostedAgentServer`]: the agent member's `DynAgentServer`, the seam the
//! kernel's `admin agent-host run` child-host command serves one agent
//! through.
//!
//! The member provides it sealed after governance: it holds the process's AI
//! provider, the verifying JWT provider and the repositories an agent writes
//! through. On a host that lacks any of them (the `ai` member disabled, no
//! verification keys, no managed-skill authority or tool-execution ledger) the
//! value is still provided, and `serve` refuses with
//! [`AgentServerUnavailable`] naming what is missing; other commands on that
//! host boot unaffected. After the server stops, the AI provider's pending
//! audit writes are drained.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::{ProvideError, Resolved};
use systemprompt_identifiers::{AgentName, InstanceId};
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{
    AgentServeError, AgentServer, DynAgentRunLauncher, DynJwtValidationProvider,
    DynManagedSkillResolver, DynSessionUsageCounters, DynToolExecutionLookup,
};

use super::run_standalone;
use super::streaming::webhook_client::HttpWebhookBroadcaster;
use crate::repository::{A2ARepositories, A2aDependencies};
use crate::{AgentAuth, AgentState};

/// Why this host cannot serve an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentServerUnavailable {
    #[error("no AI provider is admitted on this host (the `ai` member is disabled)")]
    NoAiProvider,
    #[error("this host holds no JWT verification keys")]
    NoVerification,
    #[error("no managed-skill authority is admitted on this host")]
    NoManagedSkills,
    #[error("no tool-execution ledger is admitted on this host (the `mcp` member is disabled)")]
    NoToolExecutions,
    #[error("no session-usage counters are admitted on this host")]
    NoSessionUsage,
}

#[derive(Clone)]
struct Inputs {
    ai: DynAiProvider,
    jwt: DynJwtValidationProvider,
    keyring: VerificationKeyring,
    managed_skills: DynManagedSkillResolver,
    tool_executions: DynToolExecutionLookup,
    session_usage: DynSessionUsageCounters,
    launcher: Option<DynAgentRunLauncher>,
    db: DbPool,
    config: Arc<Config>,
    instance: InstanceId,
    services: ServicesView,
}

/// The agent member's server for one spawned agent process.
pub struct HostedAgentServer {
    inputs: Result<Inputs, AgentServerUnavailable>,
}

impl std::fmt::Debug for HostedAgentServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedAgentServer")
            .field("available", &self.inputs.as_ref().err())
            .finish_non_exhaustive()
    }
}

impl HostedAgentServer {
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, ProvideError> {
        let inputs = Self::collect(r)?.ok_or_else(|| missing(r));
        Ok(Self { inputs })
    }

    fn collect(r: &Resolved<'_>) -> Result<Option<Inputs>, ProvideError> {
        let (Some(ai), Some(jwt), Some(keyring), Some(managed_skills), Some(tool_executions)) = (
            r.optional::<DynAiProvider>(),
            r.optional::<DynJwtValidationProvider>(),
            r.optional::<VerificationKeyring>(),
            r.optional::<DynManagedSkillResolver>(),
            r.optional::<DynToolExecutionLookup>(),
        ) else {
            return Ok(None);
        };
        let Some(session_usage) = r.optional::<DynSessionUsageCounters>() else {
            return Ok(None);
        };
        Ok(Some(Inputs {
            ai,
            jwt,
            keyring,
            managed_skills,
            tool_executions,
            session_usage,
            launcher: r.optional::<DynAgentRunLauncher>(),
            db: r.get::<DbPool>()?,
            config: r.get::<Arc<Config>>()?,
            instance: r.get::<InstanceId>()?,
            services: r.get::<ServicesView>()?,
        }))
    }

    fn state(inputs: &Inputs, agent: &AgentName) -> Result<AgentState, AgentServeError> {
        let repositories = A2ARepositories::new(
            &inputs.db,
            A2aDependencies {
                session_usage: Arc::clone(&inputs.session_usage),
                instance_id: inputs.instance.clone(),
                managed_skills: Arc::clone(&inputs.managed_skills),
                tool_executions: Arc::clone(&inputs.tool_executions),
            },
        );
        let webhooks =
            HttpWebhookBroadcaster::from_config(&inputs.config).map_err(|source| {
                AgentServeError::Serve {
                    agent: agent.clone(),
                    source: Box::new(source),
                }
            })?;
        let state = AgentState::new(
            Arc::clone(&inputs.db),
            Arc::clone(&inputs.config),
            AgentAuth {
                jwt_provider: Arc::clone(&inputs.jwt),
                verification_keys: inputs.keyring.clone(),
            },
            Arc::new(repositories),
            Arc::new(webhooks),
            inputs.services.clone(),
        );
        Ok(match &inputs.launcher {
            Some(launcher) => state.with_agent_run_launcher(Arc::clone(launcher)),
            None => {
                tracing::info!(
                    agent = %agent,
                    "no agent-run launcher; the agent processes every task itself"
                );
                state
            },
        })
    }
}

fn missing(r: &Resolved<'_>) -> AgentServerUnavailable {
    if r.optional::<DynAiProvider>().is_none() {
        AgentServerUnavailable::NoAiProvider
    } else if r.optional::<DynJwtValidationProvider>().is_none()
        || r.optional::<VerificationKeyring>().is_none()
    {
        AgentServerUnavailable::NoVerification
    } else if r.optional::<DynManagedSkillResolver>().is_none() {
        AgentServerUnavailable::NoManagedSkills
    } else if r.optional::<DynToolExecutionLookup>().is_none() {
        AgentServerUnavailable::NoToolExecutions
    } else {
        AgentServerUnavailable::NoSessionUsage
    }
}

#[async_trait]
impl AgentServer for HostedAgentServer {
    async fn serve(&self, agent: &AgentName, port: u16) -> Result<(), AgentServeError> {
        let inputs = self
            .inputs
            .as_ref()
            .map_err(|reason| AgentServeError::Serve {
                agent: agent.clone(),
                source: Box::new(*reason),
            })?;
        let state = Arc::new(Self::state(inputs, agent)?);
        let served = run_standalone(state, Arc::clone(&inputs.ai), agent, port).await;
        inputs.ai.drain_pending_writes().await;
        served.map_err(|source| AgentServeError::Serve {
            agent: agent.clone(),
            source: Box::new(source),
        })
    }
}
