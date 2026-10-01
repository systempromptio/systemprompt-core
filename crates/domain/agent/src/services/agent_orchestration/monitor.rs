//! Health checks of running agents via TCP probes.
//!
//! [`AgentMonitor`] performs per-agent health checks, confirming the agent's
//! registered port accepts connections, and reports the outcome as a
//! [`HealthCheckResult`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::Result;
use systemprompt_models::net::AGENT_MONITOR_TCP_TIMEOUT;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::OrchestrationResult;
use crate::services::agent_orchestration::database::AgentDatabaseService;

#[derive(Debug)]
pub struct AgentMonitor {
    db_service: AgentDatabaseService,
}

impl AgentMonitor {
    pub fn new(agent_service_repo: AgentServiceRepository) -> OrchestrationResult<Self> {
        let db_service = AgentDatabaseService::new(agent_service_repo)?;

        Ok(Self { db_service })
    }

    #[must_use]
    pub const fn with_db_service(db_service: AgentDatabaseService) -> Self {
        Self { db_service }
    }

    pub async fn comprehensive_health_check(
        &self,
        agent_name: &str,
    ) -> OrchestrationResult<HealthCheckResult> {
        let status = self.db_service.get_status(agent_name).await?;

        match status {
            crate::services::agent_orchestration::AgentStatus::Running { port, .. } => {
                match perform_tcp_health_check("127.0.0.1", port).await {
                    Ok(result) => Ok(result),
                    Err(e) => Ok(HealthCheckResult {
                        healthy: false,
                        message: format!("TCP check failed: {e}"),
                        response_time_ms: 0,
                    }),
                }
            },
            crate::services::agent_orchestration::AgentStatus::Failed { .. } => {
                Ok(HealthCheckResult {
                    healthy: false,
                    message: format!("Agent {agent_name} not in running state"),
                    response_time_ms: 0,
                })
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub message: String,
    pub response_time_ms: u64,
}

async fn perform_tcp_health_check(host: &str, port: u16) -> Result<HealthCheckResult> {
    let start = std::time::Instant::now();
    let address = format!("{host}:{port}");

    tracing::trace!(address = %address, "Attempting TCP health check");

    match timeout(AGENT_MONITOR_TCP_TIMEOUT, TcpStream::connect(&address)).await {
        Ok(Ok(_)) => {
            let response_time = start.elapsed().as_millis() as u64;
            tracing::trace!(address = %address, response_time_ms = %response_time, "Health check passed");
            Ok(HealthCheckResult {
                healthy: true,
                message: "TCP connection successful".to_owned(),
                response_time_ms: response_time,
            })
        },
        Ok(Err(e)) => {
            tracing::debug!(address = %address, error = %e, "Health check failed - connection error");
            Ok(HealthCheckResult {
                healthy: false,
                message: format!("Connection failed: {e}"),
                response_time_ms: 0,
            })
        },
        Err(_) => {
            tracing::debug!(address = %address, "Health check timeout");
            Ok(HealthCheckResult {
                healthy: false,
                message: "Connection timeout".to_owned(),
                response_time_ms: 5000,
            })
        },
    }
}
