//! Health monitoring of running agents via process and TCP probes.
//!
//! [`AgentMonitor`] performs per-agent and fleet-wide health checks, confirming
//! the agent's registered port accepts connections, and cleans up
//! unresponsive agents. Results are reported through [`HealthCheckResult`] and
//! [`MonitoringReport`]; [`check_a2a_agent_health`] is the A2A agent-card
//! probe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::AgentCard;
use crate::services::shared::{AgentServiceError, Result};
use std::time::Duration;
use systemprompt_models::net::AGENT_MONITOR_TCP_TIMEOUT;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::database::AgentDatabaseService;
use crate::services::agent_orchestration::{OrchestrationResult, process};

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

    pub async fn monitor_all_agents(&self) -> OrchestrationResult<MonitoringReport> {
        let agents = self.db_service.list_all_agents().await?;
        let mut report = MonitoringReport::new();

        for (agent_id, status) in agents {
            match status {
                crate::services::agent_orchestration::AgentStatus::Running { port, .. } => {
                    let health_result = perform_tcp_health_check("127.0.0.1", port).await?;
                    if health_result.healthy {
                        report.healthy.push(agent_id);
                    } else {
                        report.unhealthy.push(agent_id);
                    }
                },
                crate::services::agent_orchestration::AgentStatus::Failed { .. } => {
                    report.failed.push(agent_id);
                },
            }
        }

        Ok(report)
    }

    pub async fn cleanup_unresponsive_agents(&self) -> OrchestrationResult<u32> {
        tracing::debug!("Cleaning up unresponsive agents");

        let unresponsive_agents = self.db_service.get_unresponsive_agents().await?;
        let mut cleaned_up = 0;

        for (agent_id, pid_opt) in unresponsive_agents {
            if let Some(pid) = pid_opt {
                tracing::warn!(agent_id = %agent_id, pid = %pid, "Killing unresponsive agent");

                if process::kill_process_verified(pid, &agent_id) {
                    self.db_service.mark_failed(&agent_id).await?;
                    cleaned_up += 1;
                    tracing::info!(agent_id = %agent_id, "Cleaned up agent");
                } else {
                    tracing::error!(agent_id = %agent_id, pid = %pid, "Failed to kill agent");
                }
            }
        }

        if cleaned_up > 0 {
            tracing::info!(cleaned_up = %cleaned_up, "Cleaned up unresponsive agents");
        } else {
            tracing::debug!("No unresponsive agents found");
        }

        Ok(cleaned_up)
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub message: String,
    pub response_time_ms: u64,
}

#[derive(Debug)]
pub struct MonitoringReport {
    pub healthy: Vec<String>,
    pub unhealthy: Vec<String>,
    pub failed: Vec<String>,
}

impl Default for MonitoringReport {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitoringReport {
    pub const fn new() -> Self {
        Self {
            healthy: Vec::new(),
            unhealthy: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub const fn total_agents(&self) -> usize {
        self.healthy.len() + self.unhealthy.len() + self.failed.len()
    }

    pub fn healthy_percentage(&self) -> f64 {
        let total = self.total_agents();
        if total == 0 {
            0.0
        } else {
            (self.healthy.len() as f64 / total as f64) * 100.0
        }
    }
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

// Why: the A2A agent card is the liveness contract — a listener that does
// not answer `/.well-known/agent-card.json` with a card advertising at least
// one interface is not a working agent, whatever accepts the TCP connection.
pub async fn check_a2a_agent_health(port: u16, timeout_secs: u64) -> Result<bool> {
    let url = format!("http://localhost:{port}/.well-known/agent-card.json");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| AgentServiceError::Network(e.to_string()))?;

    let Ok(response) = client.get(&url).send().await else {
        return Ok(false);
    };
    if !response.status().is_success() {
        return Ok(false);
    }

    Ok(response
        .json::<AgentCard>()
        .await
        .is_ok_and(|card| !card.name.is_empty() && !card.supported_interfaces.is_empty()))
}
