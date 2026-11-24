//! Port management — reclaim an agent's port from a stale copy of that same
//! agent and verify it is free before a spawn.
//!
//! A port holder is signalled only when its agent marker and boot nonce, read
//! back from the live process, match the agent being started and the stamp
//! its service row recorded; any other holder is reported and left running.
//! The stop waits the profile's `server.shutdown.child_grace_secs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildClaim, ChildKind, SpawnStamp, StopOutcome};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult, process};

const PORT_RELEASE_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Copy, Clone)]
pub struct PortService {
    stop_grace: Duration,
}

impl PortService {
    #[must_use]
    pub const fn new(stop_grace: Duration) -> Self {
        Self { stop_grace }
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
        stamp: &SpawnStamp,
    ) -> OrchestrationResult<()> {
        if !process::is_port_in_use(port) {
            return Ok(());
        }

        let claim = ChildClaim::spawned(
            ChildKind::Agent,
            &ServiceName::of_agent(agent_name),
            stamp.clone(),
        );
        let holders = subprocess::pids_listening_on(port).await?;
        if holders.is_empty() {
            return Err(OrchestrationError::ProcessSpawnFailed(format!(
                "Port {port} is in use but no listening process can be identified"
            )));
        }
        for holder in holders {
            if !subprocess::owns(holder, &claim).await {
                return Err(OrchestrationError::PortHeldByForeignProcess {
                    port,
                    pid: holder,
                    agent: agent_name.clone(),
                });
            }
            tracing::warn!(pid = holder, port, agent = %agent_name, "Stopping stale agent process holding its port");
            match subprocess::stop_owned(holder, &claim, self.stop_grace).await? {
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
