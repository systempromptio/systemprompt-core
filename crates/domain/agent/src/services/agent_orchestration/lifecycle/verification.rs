//! Post-start agent verification with bounded readiness retries.
//!
//! Each attempt gathers a [`StartupObservation`] and lets
//! [`startup_verdict`](super::readiness::startup_verdict) decide. Where the
//! port's listeners can be identified, only the spawned pid is readiness and
//! any other listener refuses the start with `PortHeldByForeignProcess`.
//! Where they cannot (no `lsof`, hidden sockets), the connect, the spawned
//! child being alive and its spawn marker (Linux, macOS; not readable on
//! Windows) decide. The `readiness` module states the guarantee per host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, SpawnNonce};
use systemprompt_loader::subprocess;
use systemprompt_models::net::AGENT_READINESS_TCP_TIMEOUT;
use tokio::net::TcpStream;

use super::AgentLifecycle;
use super::readiness::{MarkerCheck, StartupObservation, StartupVerdict, startup_verdict};
use crate::services::agent_orchestration::{
    AgentStatus, OrchestrationError, OrchestrationResult, process,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn marker_check(pid: u32, agent_name: &AgentName, nonce: &SpawnNonce) -> MarkerCheck {
    use systemprompt_identifiers::ServiceName;
    use systemprompt_loader::subprocess::{ChildClaim, ChildKind, SpawnStamp};

    let claim = ChildClaim::spawned(
        ChildKind::Agent,
        &ServiceName::of_agent(agent_name),
        SpawnStamp::Boot(nonce.clone()),
    );
    if subprocess::owns(pid, &claim).await {
        MarkerCheck::Verified
    } else {
        MarkerCheck::Absent
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[expect(
    clippy::unused_async,
    reason = "the Linux and macOS probe awaits the identity check; one call site serves both"
)]
async fn marker_check(_pid: u32, _agent_name: &AgentName, _nonce: &SpawnNonce) -> MarkerCheck {
    MarkerCheck::Unavailable
}

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
            let stamp = self.db_service.recorded_stamp(agent_name).await?;
            PortService::new(self.child_grace())
                .cleanup_port_if_needed(port, agent_name, &stamp)
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
        let connected = matches!(
            tokio::time::timeout(AGENT_READINESS_TCP_TIMEOUT, TcpStream::connect(&address)).await,
            Ok(Ok(_))
        );
        if !connected {
            return Ok(Readiness::NotListening);
        }
        let listeners = match subprocess::pids_listening_on(port).await {
            Ok(listeners) => Some(listeners),
            Err(error) => {
                tracing::debug!(
                    agent = %agent_name,
                    port,
                    %error,
                    "port listeners unidentifiable; readiness rests on the spawned child"
                );
                None
            },
        };
        let observed = StartupObservation {
            connected,
            listeners,
            child_alive: subprocess::is_running(pid).await,
            marker: marker_check(pid, agent_name, self.spawner.nonce()).await,
        };
        match startup_verdict(pid, &observed) {
            StartupVerdict::Ready => Ok(Readiness::Ready),
            StartupVerdict::NotYet => Ok(Readiness::NotListening),
            StartupVerdict::Foreign(foreign) => Err(OrchestrationError::PortHeldByForeignProcess {
                port,
                pid: foreign,
                agent: agent_name.clone(),
            }),
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
                if subprocess::is_running(pid).await {
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
