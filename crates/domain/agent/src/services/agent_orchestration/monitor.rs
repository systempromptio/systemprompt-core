use anyhow::Result;
use std::time::Duration;
use systemprompt_core_database::DbPool;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::services::agent_orchestration::database::AgentDatabaseService;
use crate::services::agent_orchestration::{process, OrchestrationResult};

#[derive(Debug)]
pub struct AgentMonitor {
    db_service: AgentDatabaseService,
}

impl AgentMonitor {
    pub async fn new(db_pool: DbPool) -> OrchestrationResult<Self> {
        use crate::repository::agent_service::AgentServiceRepository;

        let agent_service_repo = AgentServiceRepository::new(db_pool);
        let db_service = AgentDatabaseService::new(agent_service_repo).await?;

        Ok(Self { db_service })
    }

    pub async fn comprehensive_health_check(
        &self,
        agent_id: &str,
    ) -> OrchestrationResult<HealthCheckResult> {
        let status = self.db_service.get_status(agent_id).await?;

        match status {
            crate::services::agent_orchestration::AgentStatus::Running { pid, port } => {
                if !process::process_exists(pid) {
                    return Ok(HealthCheckResult {
                        healthy: false,
                        message: format!("Process {} no longer exists", pid),
                        response_time_ms: 0,
                    });
                }

                match perform_tcp_health_check("127.0.0.1", port).await {
                    Ok(result) => Ok(result),
                    Err(e) => Ok(HealthCheckResult {
                        healthy: false,
                        message: format!("TCP check failed: {e}"),
                        response_time_ms: 0,
                    }),
                }
            },
            _ => Ok(HealthCheckResult {
                healthy: false,
                message: format!("Agent {} not in running state", agent_id),
                response_time_ms: 0,
            }),
        }
    }

    pub async fn monitor_all_agents(&self) -> OrchestrationResult<MonitoringReport> {
        let agents = self.db_service.list_all_agents().await?;
        let mut report = MonitoringReport::new();

        for (agent_id, status) in agents {
            match status {
                crate::services::agent_orchestration::AgentStatus::Running { pid, port } => {
                    if process::process_exists(pid) {
                        let health_result = perform_tcp_health_check("127.0.0.1", port).await?;
                        if health_result.healthy {
                            report.healthy_agents.push(agent_id);
                        } else {
                            report.unhealthy_agents.push(agent_id);
                        }
                    } else {
                        self.db_service
                            .mark_failed(&agent_id, "Process died")
                            .await?;
                        report.failed_agents.push(agent_id);
                    }
                },
                crate::services::agent_orchestration::AgentStatus::Failed { .. } => {
                    report.failed_agents.push(agent_id);
                },
            }
        }

        Ok(report)
    }

    pub async fn cleanup_unresponsive_agents(&self, max_failures: u32) -> OrchestrationResult<u32> {
        tracing::debug!("Cleaning up unresponsive agents");

        let unresponsive_agents = self
            .db_service
            .get_unresponsive_agents(max_failures)
            .await?;
        let mut cleaned_up = 0;

        for (agent_id, pid_opt) in unresponsive_agents {
            if let Some(pid) = pid_opt {
                tracing::warn!(agent_id = %agent_id, pid = %pid, "Killing unresponsive agent");

                if process::kill_process(pid) {
                    self.db_service.mark_crashed(&agent_id).await?;
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
    pub healthy_agents: Vec<String>,
    pub unhealthy_agents: Vec<String>,
    pub failed_agents: Vec<String>,
}

impl MonitoringReport {
    pub const fn new() -> Self {
        Self {
            healthy_agents: Vec::new(),
            unhealthy_agents: Vec::new(),
            failed_agents: Vec::new(),
        }
    }

    pub fn total_agents(&self) -> usize {
        self.healthy_agents.len() + self.unhealthy_agents.len() + self.failed_agents.len()
    }

    pub fn healthy_percentage(&self) -> f64 {
        let total = self.total_agents();
        if total == 0 {
            0.0
        } else {
            (self.healthy_agents.len() as f64 / total as f64) * 100.0
        }
    }
}

pub async fn check_agent_health(agent_id: &str) -> Result<HealthCheckResult> {
    let port = get_agent_port_simple(agent_id).await?;
    perform_tcp_health_check("127.0.0.1", port).await
}

async fn perform_tcp_health_check(host: &str, port: u16) -> Result<HealthCheckResult> {
    let start = std::time::Instant::now();
    let address = format!("{host}:{port}");

    tracing::trace!(address = %address, "Attempting TCP health check");

    match timeout(Duration::from_secs(15), TcpStream::connect(&address)).await {
        Ok(Ok(_)) => {
            let response_time = start.elapsed().as_millis() as u64;
            tracing::trace!(address = %address, response_time_ms = %response_time, "Health check passed");
            Ok(HealthCheckResult {
                healthy: true,
                message: "TCP connection successful".to_string(),
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
                message: "Connection timeout".to_string(),
                response_time_ms: 5000,
            })
        },
    }
}

async fn get_agent_port_simple(agent_id: &str) -> Result<u16> {
    let port_str = agent_id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();

    if port_str.is_empty() {
        return Ok(8000);
    }

    let port_num: u16 = port_str.parse().unwrap_or(8000);
    Ok(8000 + (port_num % 1000))
}

pub async fn check_agent_responsiveness(agent_id: &str, timeout_secs: u64) -> Result<bool> {
    let port = get_agent_port_simple(agent_id).await?;
    let address = format!("127.0.0.1:{port}");

    match timeout(
        Duration::from_secs(timeout_secs),
        TcpStream::connect(&address),
    )
    .await
    {
        Ok(Ok(_)) => {
            tracing::trace!(agent_id = %agent_id, "Agent is responsive");
            Ok(true)
        },
        Ok(Err(e)) => {
            tracing::debug!(agent_id = %agent_id, error = %e, "Agent connection failed");
            Ok(false)
        },
        Err(_) => {
            tracing::debug!(agent_id = %agent_id, timeout_secs = %timeout_secs, "Agent connection timeout");
            Ok(false)
        },
    }
}

pub async fn check_a2a_agent_health(port: u16, timeout_secs: u64) -> Result<bool> {
    let url = format!("http://localhost:{}/.well-known/agent-card.json", port);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            resp.json::<serde_json::Value>()
                .await
                .map_or(Ok(false), |json| {
                    let is_valid_card = json.get("name").is_some() && json.get("url").is_some();
                    Ok(is_valid_card)
                })
        },
        Ok(_) => Ok(false),
        Err(_) => Ok(false),
    }
}
