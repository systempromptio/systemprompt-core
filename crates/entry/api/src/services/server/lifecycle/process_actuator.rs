//! Starts and stops the processes this worker's leases cover, one command at
//! a time and off the lease loop, so a slow start never delays a renewal.
//!
//! Commands run in the order the lease loop sends them, so a handover's stop
//! always finishes before that process could be started here again. A
//! handover releases the lease only after the process has stopped, so the new
//! owner never runs it beside this one. A process that fails to start has its
//! lease released and is reported back, and the lease loop holds off
//! reclaiming it for [`super::process_ownership::FAILED_START_BACKOFF`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_database::ServiceModule;
use systemprompt_identifiers::AgentName;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;
use systemprompt_traits::LeaseRepository;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::agents::{build_orchestrator, enforce_clean_agent_state};
use super::process_ownership::ProcessKey;

#[derive(Debug)]
pub(super) enum ProcessCommand {
    Start(ProcessKey),
    Stop { key: ProcessKey, release: bool },
}

struct Orchestrators {
    mcp: Option<Arc<McpOrchestrator>>,
    agents: Option<AgentOrchestrator>,
}

pub(super) fn spawn_process_actuator(
    ctx: &AppContext,
    failed: UnboundedSender<ProcessKey>,
) -> UnboundedSender<ProcessCommand> {
    let (commands, mut inbox) = unbounded_channel();
    let ctx = ctx.clone();
    ctx.background_tasks().clone().spawn_cancellable(
        "process_actuator",
        move |cancel| async move {
            let orchestrators = Orchestrators {
                mcp: crate::services::server::runner::create_mcp_orchestrator(&ctx)
                    .inspect_err(|error| {
                        tracing::error!(error = %error, "MCP orchestrator unavailable");
                    })
                    .ok(),
                agents: build_orchestrator(&ctx, None)
                    .await
                    .inspect_err(|error| {
                        tracing::error!(error = %error, "agent orchestrator unavailable");
                    })
                    .ok(),
            };
            let leases = Arc::clone(&ctx.coordination_repositories().leases);
            loop {
                let command = tokio::select! {
                    () = cancel.cancelled() => break,
                    command = inbox.recv() => match command {
                        Some(command) => command,
                        None => break,
                    },
                };
                apply(&ctx, &orchestrators, leases.as_ref(), &failed, command).await;
            }
        },
    );
    commands
}

async fn apply(
    ctx: &AppContext,
    orchestrators: &Orchestrators,
    leases: &dyn LeaseRepository,
    failed: &UnboundedSender<ProcessKey>,
    command: ProcessCommand,
) {
    {
        match command {
            ProcessCommand::Start(key) => {
                if let Err(error) = start(ctx, orchestrators, &key).await {
                    tracing::error!(
                        service = %key.name(),
                        module = %key.module(),
                        error = %error,
                        "owned process failed to start; releasing its lease"
                    );
                    if let Err(error) = leases.release(&key.lease_key()).await {
                        tracing::warn!(service = %key.name(), error = %error, "lease release failed");
                    }
                    if failed.send(key).is_err() {
                        tracing::debug!("process lease loop has stopped");
                    }
                }
            },
            ProcessCommand::Stop { key, release } => {
                if let Err(error) = stop(orchestrators, &key).await {
                    tracing::error!(
                        service = %key.name(),
                        module = %key.module(),
                        error = %error,
                        "owned process failed to stop; leaving its lease to expire"
                    );
                    return;
                }
                tracing::info!(service = %key.name(), module = %key.module(), "owned process stopped");
                if release && let Err(error) = leases.release(&key.lease_key()).await {
                    tracing::warn!(service = %key.name(), error = %error, "lease release failed");
                }
            },
        }
    }
}

async fn start(
    ctx: &AppContext,
    orchestrators: &Orchestrators,
    key: &ProcessKey,
) -> anyhow::Result<()> {
    match key.module() {
        ServiceModule::Mcp => {
            let mcp = orchestrators
                .mcp
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("MCP orchestrator unavailable"))?;
            mcp.reconcile_owned(&BTreeSet::from([key.name().clone()]), None)
                .await?;
        },
        ServiceModule::Agent => {
            let agents = orchestrators
                .agents
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("agent orchestrator unavailable"))?;
            let config =
                AgentRegistry::new(&ctx.services_state().view()).get_agent(key.name().as_str())?;
            enforce_clean_agent_state(agents, key.name().as_str(), config.port, None).await?;
        },
    }
    tracing::info!(service = %key.name(), module = %key.module(), "owned process started");
    Ok(())
}

async fn stop(orchestrators: &Orchestrators, key: &ProcessKey) -> anyhow::Result<()> {
    match key.module() {
        ServiceModule::Mcp => {
            if let Some(mcp) = &orchestrators.mcp {
                mcp.stop_services(Some(key.name().clone())).await?;
            }
        },
        ServiceModule::Agent => {
            if let Some(agents) = &orchestrators.agents {
                agents
                    .delete_agent(&AgentName::new(key.name().as_str()))
                    .await?;
            }
        },
    }
    Ok(())
}
