//! Aggregated agent status reporting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AgentInfo, AgentOrchestrator};
use crate::services::agent_orchestration::{
    AgentStatus, OrchestrationResult, ValidationReport, monitor,
};
use systemprompt_identifiers::AgentName;

impl AgentOrchestrator {
    pub async fn get_detailed_status(&self) -> OrchestrationResult<Vec<AgentInfo>> {
        self.get_comprehensive_agent_info().await
    }

    pub(super) async fn get_comprehensive_agent_info(&self) -> OrchestrationResult<Vec<AgentInfo>> {
        let agents = self.db_service.list_all_agents().await?;
        let mut agent_info = Vec::new();

        for (agent_name, status) in agents {
            let port = match self.db_service.get_agent_config(&agent_name).await {
                Ok(config) => config.port,
                Err(_) => match status {
                    AgentStatus::Running { port, .. } => port,
                    AgentStatus::Failed { .. } => 8000,
                },
            };
            agent_info.push(AgentInfo {
                name: agent_name,
                status,
                port,
            });
        }

        Ok(agent_info)
    }

    pub async fn list_all(&self) -> OrchestrationResult<Vec<(AgentName, AgentStatus)>> {
        self.db_service.list_all_agents().await
    }

    pub async fn validate_agent(
        &self,
        agent_name: &AgentName,
    ) -> OrchestrationResult<ValidationReport> {
        let mut report = ValidationReport::new();

        let exists = self.db_service.agent_exists(agent_name).await?;
        if !exists {
            report.add_issue("Agent not found in database".to_owned());
            return Ok(report);
        }

        match self.db_service.get_agent_config(agent_name).await {
            Ok(_) => {},
            Err(e) => {
                report.add_issue(format!("Configuration error: {e}"));
                return Ok(report);
            },
        }

        let status = self.db_service.get_status(agent_name).await?;
        match status {
            AgentStatus::Running { .. } => match self.health_check(agent_name).await {
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

        for agent_name in running_agents {
            match self.health_check(&agent_name).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!(agent_name = %agent_name, error = %e, "Health check failed");
                },
            }
        }

        Ok(results)
    }
}
