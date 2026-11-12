//! `admin agents run` command hosting one agent process.
//!
//! A spawned agent boots an [`AgentHost`] from the environment the API spawned
//! it with: it never loads the profile's secrets source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::run_standalone;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
#[cfg(feature = "agent-runs")]
use systemprompt_agent_runs::AgentRunsExtension;
use systemprompt_ai::AiService;
use systemprompt_identifiers::AgentName;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_runtime::AgentHost;

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run", value_parser = crate::shared::parse_agent_name)]
    pub agent_name: AgentName,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub(super) async fn execute(args: RunArgs) -> Result<()> {
    let agent = AgentHost::from_process_env()
        .await
        .context("Failed to boot the agent host")?;
    let host = agent.host();
    let db_pool = Arc::clone(host.db_pool());

    let agent_state = AgentState::new(
        Arc::clone(&db_pool),
        Arc::new(host.config().clone()),
        agent.agent_auth(),
        Arc::clone(agent.a2a_repositories()),
        Arc::new(HttpWebhookBroadcaster::from_config(host.config())?),
        host.services_state().view(),
    );
    #[cfg(feature = "agent-runs")]
    let agent_state = agent_state.with_agent_run_launcher(AgentRunsExtension::launcher(&db_pool));
    let agent_state = Arc::new(agent_state);

    let ai_service = Arc::clone(agent.ai_service());
    let provider: DynAiProvider = Arc::<AiService>::clone(&ai_service);
    let served = run_standalone(agent_state, provider, &args.agent_name, args.port)
        .await
        .context("Failed to run agent server");
    ai_service.audit_tasks().close();
    ai_service.audit_tasks().wait().await;
    served
}
