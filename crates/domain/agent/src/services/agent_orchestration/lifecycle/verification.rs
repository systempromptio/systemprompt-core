//! Post-start agent verification with bounded readiness retries.
//!
//! An agent is ready when its port accepts a connection and the process
//! listening on it is the child this lifecycle spawned: the recorded pid,
//! carrying that agent's spawn marker. A port answered by any other process
//! refuses the start with `PortHeldByForeignProcess` rather than being taken
//! for the agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind};
use systemprompt_models::net::AGENT_READINESS_TCP_TIMEOUT;
use tokio::net::TcpStream;

use super::AgentLifecycle;
use crate::services::agent_orchestration::{
    AgentStatus, OrchestrationError, OrchestrationResult, process,
};

enum Readiness {
    Ready,
    NotListening,
}

impl AgentLifecycle {
    pub async fn validate_prerequisites(
        &self,
        agent_name: &AgentName,
        port: u16,
    ) -> OrchestrationResult<()> {
        use super::super::port_service::PortService;

        if process::is_port_in_use(port) {
            PortService::new()
                .cleanup_port_if_needed(port, agent_name)
                .await?;
            tracing::info!(port = %port, "Cleaned up port");
        }

        Ok(())
    }

    pub(super) async fn spawn_detached_process(
        &self,
        agent_name: &AgentName,
        port: u16,
    ) -> OrchestrationResult<u32> {
        process::spawn_detached(
            &self.app_paths,
            &self.spawner,
            &self.spawn_environment,
            agent_name,
            port,
        )
        .await
    }

    pub async fn verify_startup(
        &self,
        agent_name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<()> {
        const MAX_ATTEMPTS: u32 = 5;
        const SLEEP_MS: u64 = 1000;

        for attempt in 1..=MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;

            match self.readiness(agent_name, pid, port).await? {
                Readiness::Ready => return Ok(()),
                Readiness::NotListening => {
                    tracing::debug!(
                        agent = %agent_name,
                        port,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        "agent not yet listening on its port"
                    );
                },
            }
        }

        self.log_startup_failure(agent_name, port).await;
        Err(OrchestrationError::HealthCheckTimeout(
            agent_name.to_string(),
        ))
    }

    async fn readiness(
        &self,
        agent_name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<Readiness> {
        let address = format!("127.0.0.1:{port}");
        match tokio::time::timeout(AGENT_READINESS_TCP_TIMEOUT, TcpStream::connect(&address)).await
        {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => {
                tracing::trace!(error = %e, "TCP connection refused");
                return Ok(Readiness::NotListening);
            },
            Err(_) => return Ok(Readiness::NotListening),
        }
        let holders = subprocess::pids_listening_on(port).await?;
        if holders.contains(&pid)
            && subprocess::owns(pid, ChildKind::Agent, &ServiceName::of_agent(agent_name)).await
        {
            return Ok(Readiness::Ready);
        }
        match holders.into_iter().find(|holder| *holder != pid) {
            Some(foreign) => Err(OrchestrationError::PortHeldByForeignProcess {
                port,
                pid: foreign,
                agent: agent_name.clone(),
            }),
            None => Ok(Readiness::NotListening),
        }
    }

    pub async fn log_startup_failure(&self, agent_name: &AgentName, port: u16) {
        let log_path = self
            .app_paths
            .system()
            .logs()
            .join(format!("agent-{agent_name}.log"))
            .display()
            .to_string();

        match self.db_service.get_status(agent_name).await {
            Ok(AgentStatus::Running { pid, .. }) => {
                if systemprompt_loader::subprocess::is_running(pid).await {
                    tracing::error!(
                        agent = %agent_name,
                        pid,
                        port,
                        log_file = %log_path,
                        "Agent process exists but not responding on port"
                    );
                } else {
                    tracing::error!(
                        agent = %agent_name,
                        pid,
                        log_file = %log_path,
                        "Agent process died after spawn - check log file for errors"
                    );
                }
            },
            Ok(AgentStatus::Failed { reason, .. }) => {
                tracing::error!(
                    agent = %agent_name,
                    port,
                    reason = %reason,
                    log_file = %log_path,
                    "Agent in failed state"
                );
            },
            Err(e) => {
                tracing::error!(
                    agent = %agent_name,
                    port,
                    error = %e,
                    log_file = %log_path,
                    "Failed to retrieve agent status"
                );
            },
        }
    }
}
