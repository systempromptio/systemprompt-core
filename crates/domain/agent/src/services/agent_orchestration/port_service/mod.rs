//! Port management — reclaim an agent's port from a stale copy of that same
//! agent and verify it is free before a spawn.
//!
//! A port holder is signalled only when its agent marker, read back from the
//! live process, names the agent being started; any other holder is reported
//! and left running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult, process};

const STOP_GRACE: Duration = Duration::from_secs(5);
const PORT_RELEASE_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Copy, Clone)]
pub struct PortService;

impl Default for PortService {
    fn default() -> Self {
        Self::new()
    }
}

impl PortService {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn wait_for_port_available(
        &self,
        port: u16,
        timeout_secs: u64,
    ) -> OrchestrationResult<()> {
        let check_interval = Duration::from_millis(100);
        let max_checks = (timeout_secs * 1000) / 100;

        for _ in 0..max_checks {
            if !process::is_port_in_use(port) {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }

        Err(OrchestrationError::ProcessSpawnFailed(format!(
            "Port {} did not become available within {} seconds",
            port, timeout_secs
        )))
    }

    pub async fn cleanup_port_if_needed(
        &self,
        port: u16,
        agent_name: &AgentName,
    ) -> OrchestrationResult<()> {
        if !process::is_port_in_use(port) {
            return Ok(());
        }

        let service = ServiceName::of_agent(agent_name);
        let holders = subprocess::pids_listening_on(port).await?;
        if holders.is_empty() {
            return Err(OrchestrationError::ProcessSpawnFailed(format!(
                "Port {port} is in use but no listening process can be identified"
            )));
        }
        for holder in holders {
            if !subprocess::owns(holder, ChildKind::Agent, &service).await {
                return Err(OrchestrationError::PortHeldByForeignProcess {
                    port,
                    pid: holder,
                    agent: agent_name.clone(),
                });
            }
            tracing::warn!(pid = holder, port, agent = %agent_name, "Stopping stale agent process holding its port");
            match subprocess::stop_owned(holder, ChildKind::Agent, &service, STOP_GRACE).await? {
                StopOutcome::NotRunning | StopOutcome::Stopped(_) => {},
                StopOutcome::NotOurs => {
                    return Err(OrchestrationError::PortHeldByForeignProcess {
                        port,
                        pid: holder,
                        agent: agent_name.clone(),
                    });
                },
            }
        }

        self.wait_for_port_available(port, PORT_RELEASE_TIMEOUT_SECS)
            .await?;
        tracing::debug!(port, "Port is now available");
        Ok(())
    }
}
