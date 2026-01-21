use std::sync::Arc;
use std::time::{Duration, Instant};
use systemprompt_database::DbPool;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use crate::services::agent_orchestration::database::AgentDatabaseService;
use crate::services::agent_orchestration::event_bus::AgentEventBus;
use crate::services::agent_orchestration::events::AgentEvent;
use crate::services::agent_orchestration::{
    process, AgentStatus, OrchestrationError, OrchestrationResult,
};

#[derive(Debug)]
pub struct AgentLifecycle {
    db_service: AgentDatabaseService,
    event_bus: Option<Arc<AgentEventBus>>,
}

impl AgentLifecycle {
    pub async fn new(db_pool: DbPool) -> anyhow::Result<Self> {
        use crate::repository::agent_service::AgentServiceRepository;

        let agent_service_repo = AgentServiceRepository::new(db_pool.clone());
        let db_service = AgentDatabaseService::new(agent_service_repo).await?;

        Ok(Self {
            db_service,
            event_bus: None,
        })
    }

    pub fn with_event_bus(mut self, event_bus: Arc<AgentEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    fn publish_event(&self, event: AgentEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.publish(event);
        }
    }

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

    async fn validate_prerequisites(&self, port: u16) -> OrchestrationResult<()> {
        use super::port_manager::PortManager;

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

    async fn spawn_detached_process(
        &self,
        agent_name: &str,
        port: u16,
    ) -> OrchestrationResult<u32> {
        process::spawn_detached_process(agent_name, port).await
    }

    async fn verify_startup(&self, agent_name: &str, port: u16) -> OrchestrationResult<()> {
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

    async fn log_startup_failure(&self, agent_name: &str, port: u16) {
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

pub async fn start_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.start_agent(agent_name, events).await
}

pub async fn enable_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.enable_agent(agent_name, events).await
}

pub async fn disable_agent(pool: &DbPool, agent_name: &str) -> OrchestrationResult<()> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.disable_agent(agent_name).await
}

pub async fn restart_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.restart_agent(agent_name, events).await
}
