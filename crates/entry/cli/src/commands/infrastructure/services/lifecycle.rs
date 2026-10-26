//! Orchestrator composition for the `infra services` commands.
//!
//! Entry is the composition root: the agent and MCP orchestrators are wired
//! here from the [`AppContext`] and handed to the start/stop/restart
//! commands, which otherwise reduce to plans computed in
//! `systemprompt-scheduler` plus progress rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::agent_orchestration::process::SpawnEnvironment;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_agent::{AgentAuth, AgentState};
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;

pub(crate) struct OrchestratorHandles {
    pub agents: AgentOrchestrator,
    pub mcp: McpOrchestrator,
}

impl OrchestratorHandles {
    pub(crate) async fn build(ctx: &Arc<AppContext>) -> Result<Self> {
        Ok(Self {
            agents: agent_orchestrator(ctx).await?,
            mcp: mcp_orchestrator(ctx)?,
        })
    }
}

pub(crate) async fn agent_orchestrator(ctx: &Arc<AppContext>) -> Result<AgentOrchestrator> {
    let jwt_provider = Arc::new(JwtValidationProviderImpl::from_config(
        ctx.config(),
        ctx.token_authority(),
    ));
    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        AgentAuth {
            jwt_provider,
            signing_keys: ctx.signing_keys().clone(),
        },
        Arc::clone(ctx.a2a_repositories()),
        Arc::new(HttpWebhookBroadcaster::from_config(ctx.config())?),
        ctx.services_state().view(),
    ));
    AgentOrchestrator::new(
        agent_state,
        Arc::clone(ctx.app_paths_arc()),
        Arc::clone(ctx.spawner()),
        Arc::new(SpawnEnvironment {
            config: Arc::clone(ctx.config_arc()),
            secrets: Arc::clone(ctx.secrets()),
            profile_path: ctx.boot().profile_path().to_path_buf(),
        }),
        None,
    )
    .await
    .context("Failed to initialize agent orchestrator")
}

pub(crate) fn mcp_orchestrator(ctx: &Arc<AppContext>) -> Result<McpOrchestrator> {
    McpOrchestrator::new(
        (**ctx.service_repository()).clone(),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
        Arc::clone(ctx.spawner()),
        Arc::clone(ctx.secrets()),
    )
    .context("Failed to initialize MCP manager")
}

pub(crate) fn resolve_agent_name(
    services: &systemprompt_loader::ServicesState,
    agent_identifier: &str,
) -> Result<systemprompt_identifiers::AgentName> {
    let registry = AgentRegistry::new(&services.view());
    let agent = registry.get_agent(agent_identifier)?;
    Ok(systemprompt_identifiers::AgentName::new(agent.name))
}
