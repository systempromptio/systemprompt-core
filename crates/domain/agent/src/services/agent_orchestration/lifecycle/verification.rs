use std::time::Duration;

use super::AgentLifecycle;
use crate::services::agent_orchestration::{process, AgentStatus, OrchestrationError, OrchestrationResult};

impl AgentLifecycle {
    pub(crate) async fn validate_prerequisites(&self, port: u16) -> OrchestrationResult<()> {
        use super::super::port_manager::PortManager;

        let port_manager = PortManager::new();

        if process::is_port_in_use(port) {
            match port_manager.cleanup_port_if_needed(port).await {
                Ok(_) => {
                    tracing::info!(port = %port, "Cleaned up port");
                },
                Err(e) => {
                    tracing::error!(error = %e, port = %port, "Port is in use and cleanup failed");
                    return Err(e);
                },
            }
        }

        Ok(())
    }

    pub(crate) async fn spawn_detached_process(
        &self,
        agent_name: &str,
        port: u16,
    ) -> OrchestrationResult<u32> {
        process::spawn_detached_process(agent_name, port).await
    }

    pub(crate) async fn verify_startup(
        &self,
        agent_name: &str,
        port: u16,
    ) -> OrchestrationResult<()> {
        const MAX_ATTEMPTS: u32 = 5;
        const SLEEP_MS: u64 = 1000;
        const TCP_TIMEOUT_SECS: u64 = 2;

        for attempt in 1..=MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;

            match self.check_port_responsiveness(port, TCP_TIMEOUT_SECS).await {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    tracing::debug!(
                        agent = %agent_name,
                        port,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        "Health check returned false - agent not ready"
                    );
                },
                Err(e) => {
                    tracing::debug!(
                        agent = %agent_name,
                        port,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        error = %e,
                        "Health check connection error"
                    );
                },
            }
        }

        self.log_startup_failure(agent_name, port).await;
        self.db_service.mark_error(agent_name).await?;
        Err(OrchestrationError::HealthCheckTimeout(
            agent_name.to_string(),
        ))
    }

    async fn check_port_responsiveness(
        &self,
        port: u16,
        timeout_secs: u64,
    ) -> OrchestrationResult<bool> {
        use tokio::net::TcpStream;
        use tokio::time::timeout;

        let address = format!("127.0.0.1:{port}");
        match timeout(
            Duration::from_secs(timeout_secs),
            TcpStream::connect(&address),
        )
        .await
        {
            Ok(Ok(_)) => Ok(true),
            Ok(Err(e)) => {
                tracing::trace!(error = %e, "TCP connection refused");
                Ok(false)
            },
            Err(_) => Ok(false),
        }
    }

    pub(crate) async fn log_startup_failure(&self, agent_name: &str, port: u16) {
        let log_path = match systemprompt_models::Config::get() {
            Ok(config) => format!("{}/logs/agent-{}.log", config.system_path, agent_name),
            Err(e) => {
                tracing::debug!(error = %e, "Config not available, using fallback log path");
                format!("/tmp/logs/agent-{}.log", agent_name)
            },
        };

        match self.db_service.get_status(agent_name).await {
            Ok(AgentStatus::Running { pid, .. }) => {
                if process::process_exists(pid) {
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
