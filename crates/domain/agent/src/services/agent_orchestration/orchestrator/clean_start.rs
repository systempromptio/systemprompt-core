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
use systemprompt_loader::subprocess::{self, ChildClaim, ChildKind, SpawnStamp, StopOutcome};

use super::AgentOrchestrator;
use crate::services::agent_orchestration::{AgentStatus, OrchestrationResult, PortService};

impl AgentOrchestrator {
    pub async fn start_clean(&self, agent: &AgentName, port: u16) -> OrchestrationResult<()> {
        let grace = self.lifecycle.child_grace();
        if let Ok(status) = self.get_status(agent).await {
            match status {
                AgentStatus::Running {
                    pid,
                    port: running,
                    stamp,
                } => {
                    tracing::info!(
                        agent = %agent,
                        pid,
                        running_port = running,
                        desired_port = port,
                        "stopping the running agent before a clean start"
                    );
                    stop_recorded_agent(pid, agent, stamp, grace).await?;
                    if let Err(e) = self.delete_agent(agent).await {
                        tracing::warn!(error = %e, agent = %agent, "Failed to delete agent during cleanup");
                    }
                },
                AgentStatus::Failed { .. } => {
                    tracing::info!(agent = %agent, "restarting a previously failed agent");
                },
            }
        }

        let stamp = self.db_service.recorded_stamp(agent).await?;
        PortService::new(grace)
            .cleanup_port_if_needed(port, agent, &stamp)
            .await?;
        self.start_agent(agent, None).await.map(drop)
    }
}

async fn stop_recorded_agent(
    pid: u32,
    agent: &AgentName,
    stamp: SpawnStamp,
    grace: Duration,
) -> OrchestrationResult<()> {
    let claim = ChildClaim::spawned(ChildKind::Agent, &ServiceName::of_agent(agent), stamp);
    match subprocess::stop_owned(pid, &claim, grace).await? {
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
