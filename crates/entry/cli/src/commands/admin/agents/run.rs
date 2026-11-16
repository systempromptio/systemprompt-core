//! `admin agents run` command hosting one agent process.
//!
//! A spawned agent boots an [`AgentHost`] from the environment the API spawned
//! it with: it never loads the profile's secrets source.
//!
//! [`RunLauncher::resolve`] reads the host's extension state for a
//! [`DynAgentRunLauncher`], which the agent-runs capability provides when it is
//! linked and enabled. [`RunLauncher::Linked`] hands it to the A2A server,
//! which queues the tasks of an agent that declares a run template as agent
//! runs. [`RunLauncher::Absent`] is logged and leaves the server without one,
//! the behaviour of an install without agent-runs: every task, including one
//! for an agent that declares a run template, is processed by the agent itself,
//! `tasks/cancel` finds no run behind a task, and a streaming request receives
//! the task's own frames.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::run_standalone;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_ai::AiService;
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::AgentName;
use systemprompt_mcp_host::AgentHost;

use crate::kernel::KernelHost;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_traits::{Dependencies, DynAgentRunLauncher};

/// Whether the host's extension state carries an agent-run launcher.
#[derive(Clone)]
pub enum RunLauncher {
    Linked(DynAgentRunLauncher),
    Absent,
}

impl std::fmt::Debug for RunLauncher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linked(_) => f.write_str("RunLauncher::Linked"),
            Self::Absent => f.write_str("RunLauncher::Absent"),
        }
    }
}

impl RunLauncher {
    #[must_use]
    pub fn resolve(extensions: &Dependencies) -> Self {
        extensions
            .find::<DynAgentRunLauncher>()
            .map_or(Self::Absent, |launcher| Self::Linked(Arc::clone(launcher)))
    }

    #[must_use]
    pub const fn is_linked(&self) -> bool {
        matches!(self, Self::Linked(_))
    }

    #[must_use]
    pub fn apply(self, agent_state: AgentState, agent_name: &AgentName) -> AgentState {
        match self {
            Self::Linked(launcher) => agent_state.with_agent_run_launcher(launcher),
            Self::Absent => {
                tracing::info!(
                    agent = %agent_name,
                    "no agent-run launcher; the agent processes every task itself"
                );
                agent_state
            },
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run", value_parser = systemprompt_extension::cli::parsers::parse_agent_name)]
    pub agent_name: AgentName,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub(super) async fn execute(args: RunArgs, host: &dyn KernelHost) -> Result<()> {
    let agent = AgentHost::from_process_env(host.take_admit()?)
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
    let agent_state =
        RunLauncher::resolve(&host.extension_state()).apply(agent_state, &args.agent_name);
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
