//! Agent start/stop lifecycle operations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use super::AgentLifecycle;
use crate::services::agent_orchestration::{AgentStatus, OrchestrationError, OrchestrationResult};

const STOP_GRACE: Duration = Duration::from_secs(5);

impl AgentLifecycle {
    pub async fn start_agent(
        &self,
        agent_name: &AgentName,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        let start = Instant::now();

        let agent_config = self.db_service.get_agent_config(agent_name).await?;

        if let Some(tx) = events {
            tx.agent_starting(&agent_config.name, agent_config.port);
        }

        let result = async {
            let current_status = self.db_service.get_status(agent_name).await?;
            match current_status {
                AgentStatus::Running { .. } => {
                    return Err(OrchestrationError::AgentAlreadyRunning(
                        agent_name.to_string(),
                    ));
                },
                AgentStatus::Failed { .. } => {
                    tracing::debug!(agent_name = %agent_name, "Agent previously failed, attempting restart");
                },
            }

            self.validate_prerequisites(agent_name, agent_config.port)
                .await?;

            let pid = self
                .spawn_detached_process(agent_name, agent_config.port)
                .await?;

            if let Err(e) = self
                .confirm_spawned(agent_name, pid, agent_config.port)
                .await
            {
                self.reap_failed_spawn(agent_name, pid).await;
                return Err(e);
            }

            tracing::debug!(agent = %agent_config.name, port = agent_config.port, "agent started");

            if let Some(tx) = events {
                tx.agent_ready(&agent_config.name, agent_config.port, start.elapsed());
            }

            Ok(agent_config.name.clone())
        }
        .await;

        if let Err(ref e) = result {
            if let Some(tx) = events {
                tx.agent_failed(&agent_config.name, e.to_string());
            }

            tracing::error!(error = %e, agent_name = %agent_name, "Failed to start agent");
        }

        result
    }

    pub async fn disable_agent(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        tracing::debug!(agent_name = %agent_name, "disabling agent");

        let status = self.db_service.get_status(agent_name).await?;

        if let AgentStatus::Running { pid, .. } = status {
            let outcome = stop_agent_process(agent_name, pid).await?;
            tracing::debug!(agent_name = %agent_name, pid = %pid, outcome = ?outcome, "Stopped agent process");
        }

        self.db_service.remove_agent_service(agent_name).await?;

        tracing::debug!(agent_name = %agent_name, "agent disabled");
        Ok(())
    }

    pub async fn enable_agent(
        &self,
        agent_name: &AgentName,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        tracing::debug!(agent_name = %agent_name, "enabling agent");
        self.start_agent(agent_name, events).await
    }

    pub async fn restart_agent(
        &self,
        agent_name: &AgentName,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        tracing::debug!(agent_name = %agent_name, "Restarting agent");

        let status = self.db_service.get_status(agent_name).await?;
        if let AgentStatus::Running { pid, .. } = status {
            let outcome = stop_agent_process(agent_name, pid).await?;
            tracing::debug!(agent_name = %agent_name, pid = %pid, outcome = ?outcome, "Stopped agent process before restart");

            self.db_service.update_agent_stopped(agent_name).await?;
        }

        self.start_agent(agent_name, events).await
    }

    async fn confirm_spawned(
        &self,
        agent_name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<()> {
        self.db_service
            .register_agent_starting(agent_name, pid, port)
            .await?;
        self.verify_startup(agent_name, port).await?;
        self.db_service.mark_running(agent_name).await
    }

    async fn reap_failed_spawn(&self, agent_name: &AgentName, pid: u32) {
        match stop_agent_process(agent_name, pid).await {
            Ok(_) => {
                if let Err(e) = self.db_service.mark_failed(agent_name).await {
                    tracing::error!(
                        agent_name = %agent_name,
                        error = %e,
                        "Spawned agent was stopped but its row could not be marked failed"
                    );
                }
            },
            Err(e) => {
                tracing::error!(
                    agent_name = %agent_name,
                    pid,
                    error = %e,
                    "Spawned agent failed readiness and could not be stopped; PID kept"
                );
            },
        }
    }

    pub async fn cleanup_crashed_agent(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        let status = self.db_service.get_status(agent_name).await?;

        if let AgentStatus::Running { pid, .. } = status
            && !subprocess::is_running(pid).await
        {
            self.db_service.mark_failed(agent_name).await?;
            tracing::info!(agent_name = %agent_name, "Marked crashed agent as failed in database");
        }

        Ok(())
    }
}

async fn stop_agent_process(agent_name: &AgentName, pid: u32) -> OrchestrationResult<StopOutcome> {
    let outcome = subprocess::stop_owned(
        pid,
        ChildKind::Agent,
        &ServiceName::of_agent(agent_name),
        STOP_GRACE,
    )
    .await?;
    if outcome == StopOutcome::NotOurs {
        tracing::warn!(
            agent_name = %agent_name,
            pid,
            "Recorded pid is not this agent's process; it was left running and the row is cleared"
        );
    }
    Ok(outcome)
}
