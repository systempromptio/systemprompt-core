use super::{AgentInfo, AgentOrchestrator};
use crate::services::agent_orchestration::{
    monitor, AgentStatus, OrchestrationResult, ValidationReport,
};

impl AgentOrchestrator {
    /// Get detailed status of all agents for display by presentation layer
    pub async fn get_detailed_status(&self) -> OrchestrationResult<Vec<AgentInfo>> {
        self.get_comprehensive_agent_info().await
    }

    pub(super) async fn get_comprehensive_agent_info(&self) -> OrchestrationResult<Vec<AgentInfo>> {
        let agents = self.db_service.list_all_agents().await?;
        let mut agent_info = Vec::new();

        for (agent_id, status) in agents {
            match self.db_service.get_agent_config(&agent_id).await {
                Ok(config) => {
                    agent_info.push(AgentInfo {
                        id: agent_id,
                        name: config.name,
                        status,
                        port: config.port,
                    });
                },
                Err(_) => {
                    let port = match status {
                        AgentStatus::Running { port, .. } => port,
                        _ => 8000,
                    };
                    agent_info.push(AgentInfo {
                        id: agent_id.clone(),
                        name: "Unknown".to_string(),
                        status,
                        port,
                    });
                },
            }
        }

        Ok(agent_info)
    }

    pub async fn list_all(&self) -> OrchestrationResult<Vec<(String, AgentStatus)>> {
        self.db_service.list_all_agents().await
    }

    pub async fn validate_agent(&self, agent_id: &str) -> OrchestrationResult<ValidationReport> {
        let mut report = ValidationReport::new();

        let exists = self.db_service.agent_exists(agent_id).await?;
        if !exists {
            report.add_issue("Agent not found in database".to_string());
            return Ok(report);
        }

        match self.db_service.get_agent_config(agent_id).await {
            Ok(_) => {},
            Err(e) => {
                report.add_issue(format!("Configuration error: {e}"));
                return Ok(report);
            },
        }

        let status = self.db_service.get_status(agent_id).await?;
        match status {
            AgentStatus::Running { .. } => match self.health_check(agent_id).await {
                Ok(health) => {
                    if !health.healthy {
                        report.add_issue(format!("Health check failed: {}", health.message));
                    }
                },
                Err(e) => {
                    report.add_issue(format!("Health check error: {e}"));
                },
            },
            AgentStatus::Failed { reason, .. } => {
                report.add_issue(format!("Agent is in failed state: {reason}"));
            },
        }

        Ok(report)
    }

    pub async fn health_check_all(&self) -> OrchestrationResult<Vec<monitor::HealthCheckResult>> {
        let running_agents = self.db_service.list_running_agents().await?;
        let mut results = Vec::new();

        for agent_id in running_agents {
            match self.health_check(&agent_id).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!(agent_id = %agent_id, error = %e, "Health check failed");
                },
            }
        }

        Ok(results)
    }
}
