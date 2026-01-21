use std::time::Instant;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use super::AgentLifecycle;
use crate::services::agent_orchestration::events::AgentEvent;
use crate::services::agent_orchestration::{
    process, AgentStatus, OrchestrationError, OrchestrationResult,
};

impl AgentLifecycle {
    pub async fn start_agent(
        &self,
        agent_name: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        let start = Instant::now();

        self.publish_event(AgentEvent::AgentStartRequested {
            agent_id: agent_name.to_string(),
        });

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

            self.validate_prerequisites(agent_config.port).await?;

            let pid = self
                .spawn_detached_process(agent_name, agent_config.port)
                .await?;

            let service_id = self
                .db_service
                .register_agent_starting(&agent_config.name, pid, agent_config.port)
                .await?;

            self.verify_startup(agent_name, agent_config.port).await?;

            self.db_service.mark_running(agent_name).await?;

            tracing::info!("Agent started: {} :{}", agent_config.name, agent_config.port);

            self.publish_event(AgentEvent::AgentStarted {
                agent_id: agent_name.to_string(),
                pid,
                port: agent_config.port,
            });

            if let Some(tx) = events {
                tx.agent_ready(&agent_config.name, agent_config.port, start.elapsed());
            }

            Ok(service_id)
        }
        .await;

        if let Err(ref e) = result {
            self.publish_event(AgentEvent::AgentFailed {
                agent_id: agent_name.to_string(),
                error: e.to_string(),
            });

            if let Some(tx) = events {
                tx.agent_failed(&agent_config.name, e.to_string());
            }

            tracing::error!(error = %e, agent_name = %agent_name, "Failed to start agent");
        }

        result
    }

    pub async fn disable_agent(&self, agent_name: &str) -> OrchestrationResult<()> {
        tracing::debug!("Disabling agent: {}", agent_name);

        let status = self.db_service.get_status(agent_name).await?;

        if let AgentStatus::Running { pid, .. } = status {
            if process::kill_process(pid) {
                tracing::debug!(agent_name = %agent_name, pid = %pid, "Killed process");
                self.publish_event(AgentEvent::AgentStopped {
                    agent_id: agent_name.to_string(),
                    exit_code: None,
                });
            } else {
                tracing::warn!(agent_name = %agent_name, pid = %pid, "Failed to kill process");
            }
        }

        self.db_service.remove_agent_service(agent_name).await?;

        self.publish_event(AgentEvent::AgentDisabled {
            agent_id: agent_name.to_string(),
        });

        tracing::debug!("Agent disabled: {}", agent_name);
        Ok(())
    }

    pub async fn enable_agent(
        &self,
        agent_name: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        tracing::debug!("Enabling agent: {}", agent_name);
        self.start_agent(agent_name, events).await
    }

    pub async fn restart_agent(
        &self,
        agent_name: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        tracing::debug!(agent_name = %agent_name, "Restarting agent");

        self.publish_event(AgentEvent::AgentRestartRequested {
            agent_id: agent_name.to_string(),
            reason: "User requested restart".to_string(),
        });

        let status = self.db_service.get_status(agent_name).await?;
        if let AgentStatus::Running { pid, .. } = status {
            match process::terminate_gracefully(pid, 5).await {
                Ok(()) => {
                    tracing::debug!(agent_name = %agent_name, pid = %pid, "Gracefully terminated process");
                },
                Err(e) => {
                    tracing::warn!(agent_name = %agent_name, pid = %pid, error = %e, "Failed to gracefully terminate");
                },
            }

            self.publish_event(AgentEvent::AgentStopped {
                agent_id: agent_name.to_string(),
                exit_code: None,
            });

            self.db_service.update_agent_stopped(agent_name).await?;
        }

        self.start_agent(agent_name, events).await
    }

    pub async fn cleanup_crashed_agent(&self, agent_name: &str) -> OrchestrationResult<()> {
        let status = self.db_service.get_status(agent_name).await?;

        if let AgentStatus::Running { pid, .. } = status {
            if !process::process_exists(pid) {
                self.db_service.mark_crashed(agent_name).await?;
                tracing::info!(agent_name = %agent_name, "Marked crashed agent as failed in database");
            }
        }

        Ok(())
    }
}
