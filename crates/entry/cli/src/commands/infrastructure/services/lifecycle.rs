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
use systemprompt_agent::AgentState;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
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
    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    ));
    AgentOrchestrator::new(agent_state, Arc::clone(ctx.app_paths_arc()), None)
        .await
        .context("Failed to initialize agent orchestrator")
}

pub(crate) fn mcp_orchestrator(ctx: &Arc<AppContext>) -> Result<McpOrchestrator> {
    McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )
    .context("Failed to initialize MCP manager")
}

pub(crate) async fn resolve_agent_name(agent_identifier: &str) -> Result<String> {
    let registry = AgentRegistry::new()?;
    let agent = registry.get_agent(agent_identifier).await?;
    Ok(agent.name)
}
