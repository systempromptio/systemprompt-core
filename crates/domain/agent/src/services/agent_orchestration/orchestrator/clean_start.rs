//! Starting one agent into a known-clean state.
//!
//! [`AgentOrchestrator::start_clean`] stops a running agent through its
//! recorded pid (signalled only when the pid is provably this installation's
//! agent child), drops its service record, frees its port and starts it
//! again; an agent recorded as failed is restarted the same way. The API's
//! startup reconciliation and its leased process actuator drive it through
//! the agent member's `ServiceProcessSet::restart`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};

use super::AgentOrchestrator;
use crate::services::agent_orchestration::{AgentStatus, OrchestrationResult, PortService};

const AGENT_STOP_GRACE: Duration = Duration::from_secs(5);

impl AgentOrchestrator {
    pub async fn start_clean(&self, agent: &AgentName, port: u16) -> OrchestrationResult<()> {
        if let Ok(status) = self.get_status(agent).await {
            match status {
                AgentStatus::Running { pid, port: running } => {
                    tracing::info!(
                        agent = %agent,
                        pid,
                        running_port = running,
                        desired_port = port,
                        "stopping the running agent before a clean start"
                    );
                    stop_recorded_agent(pid, agent).await?;
                    if let Err(e) = self.delete_agent(agent).await {
                        tracing::warn!(error = %e, agent = %agent, "Failed to delete agent during cleanup");
                    }
                },
                AgentStatus::Failed { .. } => {
                    tracing::info!(agent = %agent, "restarting a previously failed agent");
                },
            }
        }

        PortService::new()
            .cleanup_port_if_needed(port, agent)
            .await?;
        self.start_agent(agent, None).await.map(drop)
    }
}

async fn stop_recorded_agent(pid: u32, agent: &AgentName) -> OrchestrationResult<()> {
    let service = ServiceName::of_agent(agent);
    match subprocess::stop_owned(pid, ChildKind::Agent, &service, AGENT_STOP_GRACE).await? {
        StopOutcome::NotRunning | StopOutcome::Stopped(_) => Ok(()),
        StopOutcome::NotOurs => {
            tracing::warn!(
                pid,
                agent = %agent,
                "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
            );
            Ok(())
        },
    }
}
