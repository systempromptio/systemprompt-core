//! Agent reconciliation during server startup.
//!
//! [`reconcile_agents`] starts every enabled agent into a known-clean state,
//! retrying failed agents once after cleanup, and fails server startup if any
//! required agent cannot be brought up — agents are a hard dependency of the
//! API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use futures_util::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use systemprompt_models::AgentConfig;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

const AGENT_STOP_GRACE: Duration = Duration::from_secs(5);

pub async fn reconcile_agents(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<usize> {
    let orchestrator = build_orchestrator(ctx, events).await?;

    let agent_registry = match AgentRegistry::new() {
        Ok(registry) => registry,
        Err(e) => {
            events.error(format!("Failed to load agent registry: {e}"), true);
            return Err(e.into());
        },
    };

    let enabled_agents = match agent_registry.list_enabled_agents().await {
        Ok(agents) => agents,
        Err(e) => {
            events.error(format!("Failed to list enabled agents: {e}"), true);
            return Err(e.into());
        },
    };

    let required_count = enabled_agents.len();
    let (started, failed_agents) =
        start_enabled_agents(&orchestrator, &enabled_agents, events).await;

    let started = if failed_agents.is_empty() {
        started
    } else {
        handle_failed_agents(
            started,
            &failed_agents,
            &agent_registry,
            &orchestrator,
            events,
        )
        .await?
    };

    if started < required_count {
        return Err(anyhow::anyhow!(
            "FATAL: Only {}/{} required agents started successfully\n\nAll enabled agents must be \
             running for API to start.",
            started,
            required_count
        ));
    }

    Ok(started)
}

async fn build_orchestrator(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<AgentOrchestrator> {
    let jwt_provider = Arc::new(JwtValidationProviderImpl::from_config()?);
    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
        Arc::clone(ctx.a2a_repositories()),
        Arc::new(HttpWebhookBroadcaster::from_config(ctx.config())?),
    ));

    match AgentOrchestrator::new(agent_state, Arc::clone(ctx.app_paths_arc()), events).await {
        Ok(orch) => Ok(orch),
        Err(e) => {
            events.error(
                format!("Failed to initialize agent orchestrator: {e}"),
                true,
            );
            Err(e.into())
        },
    }
}

async fn start_enabled_agents(
    orchestrator: &AgentOrchestrator,
    enabled_agents: &[AgentConfig],
    events: Option<&StartupEventSender>,
) -> (usize, Vec<(String, anyhow::Error)>) {
    let start_futures: Vec<_> = enabled_agents
        .iter()
        .map(|agent_config| {
            let name = agent_config.name.clone();
            let port = agent_config.port;
            async move {
                enforce_clean_agent_state(orchestrator, &name, port, events)
                    .await
                    .map(|_| name.clone())
                    .map_err(|e| (name.clone(), e))
            }
        })
        .collect();

    let results = join_all(start_futures).await;

    let (succeeded, failed): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    let failed_agents: Vec<(String, anyhow::Error)> =
        failed.into_iter().filter_map(Result::err).collect();

    (succeeded.len(), failed_agents)
}

async fn handle_failed_agents(
    mut started: usize,
    failed_agents: &[(String, anyhow::Error)],
    agent_registry: &AgentRegistry,
    orchestrator: &AgentOrchestrator,
    events: Option<&StartupEventSender>,
) -> Result<usize> {
    events.warning_with_context(
        format!(
            "{} agent(s) failed to start on first attempt",
            failed_agents.len()
        ),
        "Attempting cleanup and retry",
    );

    let mut retry_failed: Vec<(String, String)> = Vec::new();

    for (agent_name, first_error) in failed_agents {
        tracing::warn!(agent = %agent_name, error = ?first_error, "Agent failed on first start attempt");
        let agent_config = match agent_registry.get_agent(agent_name).await {
            Ok(config) => config,
            Err(e) => {
                events.agent_failed(agent_name.clone(), format!("Agent config not found: {e}"));
                retry_failed.push((agent_name.clone(), format!("Agent config not found: {e}")));
                continue;
            },
        };

        match enforce_clean_agent_state(orchestrator, agent_name, agent_config.port, events).await {
            Ok(_) => {
                started += 1;
            },
            Err(e) => {
                retry_failed.push((agent_name.clone(), e.to_string()));
            },
        }
    }

    if !retry_failed.is_empty() {
        let agent_names: Vec<String> = retry_failed.iter().map(|(name, _)| name.clone()).collect();
        return Err(anyhow::anyhow!(
            "FATAL: {} required agent(s) failed to start after retry: {}\n\nsystemprompt.io OS \
             cannot operate without all enabled agents.\nAgents are the core service \
             layer.\n\nFailures:\n{}\n\nPossible causes:\n  - Agent binaries not built (run: \
             cargo build)\n  - Ports occupied by non-agent processes (check with: lsof -i:PORT)\n  \
             - Missing environment variables (check .env file)\n  - File permission \
             issues\n\nBuild agents with: cargo build",
            retry_failed.len(),
            agent_names.join(", "),
            retry_failed
                .iter()
                .map(|(name, err)| format!("  - {name}: {err}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Ok(started)
}

async fn enforce_clean_agent_state(
    orchestrator: &AgentOrchestrator,
    agent: &str,
    desired_port: u16,
    events: Option<&StartupEventSender>,
) -> Result<bool> {
    use systemprompt_agent::services::agent_orchestration::{AgentStatus, PortService};

    let agent_name = systemprompt_identifiers::AgentName::new(agent);
    if let Ok(status) = orchestrator.get_status(&agent_name).await {
        match status {
            AgentStatus::Running { pid, port } => {
                let reason = if port == desired_port {
                    format!("Restarting agent to ensure fresh state (pid {pid})")
                } else {
                    format!(
                        "On wrong port {port} (expected {desired_port}), killing and restarting"
                    )
                };
                events.agent_cleanup(agent.to_owned(), reason);
                stop_recorded_agent(pid, &agent_name, events).await?;
                if let Err(e) = orchestrator.delete_agent(&agent_name).await {
                    tracing::warn!(error = %e, agent = %agent, "Failed to delete agent during cleanup");
                }
            },
            AgentStatus::Failed { .. } => {
                events.agent_cleanup(agent.to_owned(), "Previously failed, restarting");
            },
        }
    }

    let port_manager = PortService::new();
    if let Err(e) = port_manager
        .cleanup_port_if_needed(desired_port, &agent_name)
        .await
    {
        events.error(
            format!("Failed to cleanup port {desired_port} for agent {agent}: {e}"),
            false,
        );
        return Err(e.into());
    }

    match orchestrator.start_agent(&agent_name, events).await {
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

async fn stop_recorded_agent(
    pid: u32,
    agent_name: &systemprompt_identifiers::AgentName,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let service = systemprompt_identifiers::ServiceName::of_agent(agent_name);
    match subprocess::stop_owned(pid, ChildKind::Agent, &service, AGENT_STOP_GRACE).await {
        Ok(StopOutcome::NotRunning | StopOutcome::Stopped(_)) => Ok(()),
        Ok(StopOutcome::NotOurs) => {
            tracing::warn!(
                pid,
                agent = %agent_name,
                "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
            );
            Ok(())
        },
        Err(e) => {
            events.error(
                format!("Failed to stop agent {agent_name} (pid {pid}): {e}"),
                false,
            );
            Err(e.into())
        },
    }
}
