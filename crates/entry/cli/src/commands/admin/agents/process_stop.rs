//! Stopping an agent's process before its configuration is deleted: through
//! the orchestrator first, then only the recorded, marker-verified pid that
//! holds the agent's port.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_agent::services::agent_orchestration::{AgentOrchestrator, AgentStatus};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use systemprompt_logging::CliService;
use systemprompt_scheduler::port_holders;

const AGENT_STOP_GRACE: Duration = Duration::from_secs(2);

pub async fn stop_agent_process(
    agent_name: &AgentName,
    agent_port: Option<u16>,
    orchestrator: &AgentOrchestrator,
) -> bool {
    let recorded_pid = match orchestrator.get_status(agent_name).await {
        Ok(AgentStatus::Running { pid, .. }) => Some(pid),
        Ok(AgentStatus::Failed { .. }) => None,
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                error = %e,
                "Could not read the agent's recorded process"
            );
            return false;
        },
    };

    match orchestrator.delete_agent(agent_name).await {
        Ok(()) => {
            tracing::debug!(agent = %agent_name, "Agent stopped via orchestrator");
            true
        },
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                error = %e,
                "Orchestrator termination failed; stopping only a verified agent process"
            );
            stop_verified_port_holder(agent_name, agent_port, recorded_pid).await
        },
    }
}

pub async fn stop_verified_port_holder(
    agent_name: &AgentName,
    agent_port: Option<u16>,
    recorded_pid: Option<u32>,
) -> bool {
    let Some(port) = agent_port else {
        return recorded_pid.is_none();
    };

    let holders = match port_holders(port).await {
        Ok(holders) => holders,
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                port,
                error = %e,
                "Could not read the port's holders"
            );
            return false;
        },
    };
    let Some(&holder) = holders.first() else {
        tracing::debug!(agent = %agent_name, port, "No process on port; agent is stopped");
        return true;
    };

    if let Some(&stranger) = holders.iter().find(|pid| Some(**pid) != recorded_pid) {
        CliService::warning(&format!(
            "Process {stranger} holds port {port} but is not the recorded process of agent \
             '{agent_name}'; refusing to kill it"
        ));
        return false;
    }

    CliService::info(&format!(
        "Stopping agent '{}' (pid {}) on port {}...",
        agent_name, holder, port
    ));
    let service = ServiceName::of_agent(agent_name);
    match subprocess::stop_owned(holder, ChildKind::Agent, &service, AGENT_STOP_GRACE).await {
        Ok(StopOutcome::Stopped(_) | StopOutcome::NotRunning) => true,
        Ok(StopOutcome::NotOurs) => {
            CliService::warning(&format!(
                "Process {holder} is agent '{agent_name}''s recorded pid but carries no matching \
                 spawn marker; refusing to kill it"
            ));
            false
        },
        Err(e) => {
            tracing::warn!(
                agent = %agent_name,
                port,
                pid = holder,
                error = %e,
                "Failed to stop agent process"
            );
            false
        },
    }
}
