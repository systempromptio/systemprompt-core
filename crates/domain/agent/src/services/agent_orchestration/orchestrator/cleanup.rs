//! Stale agent-process cleanup in the orchestrator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::AgentOrchestrator;
use crate::services::agent_orchestration::{AgentStatus, OrchestrationResult};

impl AgentOrchestrator {
    pub async fn delete_agent(&self, agent_name: &str) -> OrchestrationResult<()> {
        tracing::debug!(agent_name = %agent_name, "Deleting agent");

        if let Ok(AgentStatus::Running { .. }) = self.get_status(agent_name).await {
            tracing::debug!(agent_name = %agent_name, "Stopping running agent before deletion");
            self.lifecycle.disable_agent(agent_name).await?;
        }

        self.db_service.remove_agent_service(agent_name).await?;

        tracing::debug!(agent_name = %agent_name, "Agent deleted successfully");
        Ok(())
    }

    pub async fn delete_all_agents(&self) -> OrchestrationResult<u64> {
        tracing::info!("Deleting all agents");

        let agents = self.list_all().await?;
        let total_count = agents.len() as u64;

        if total_count == 0 {
            tracing::debug!("No agents to delete");
            return Ok(0);
        }

        tracing::info!(count = %total_count, "Found agents to delete");

        tracing::debug!("Disabling all running agents");
        self.disable_all().await?;

        let mut deleted_count = 0;
        for (agent_id, _) in agents {
            match self.delete_agent(&agent_id).await {
                Ok(()) => {
                    deleted_count += 1;
                },
                Err(e) => {
                    tracing::error!(agent_id = %agent_id, error = %e, "Failed to delete agent");
                },
            }
        }

        tracing::info!(deleted = %deleted_count, total = %total_count, "Deleted agents");
        Ok(deleted_count)
    }
}
