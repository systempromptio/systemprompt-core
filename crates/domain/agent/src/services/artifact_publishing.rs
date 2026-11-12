//! Artifact persistence for agent and MCP tool output.
//!
//! Persists the artifacts of the task the agent runtime is executing (named
//! by its [`RuntimeTask`]), enriches them with skill metadata, and verifies
//! MCP execution ids against the owning domain's ledger through
//! [`ToolExecutionLookup`](systemprompt_traits::ToolExecutionLookup).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};

use std::sync::Arc;

use crate::models::a2a::Artifact;
use crate::repository::content::ArtifactRepository;
use crate::repository::{A2ARepositories, RuntimeTask};
use crate::services::SkillService;
use systemprompt_identifiers::UserId;
use systemprompt_traits::DynToolExecutionLookup;

pub struct ArtifactPublishingService {
    artifact_repo: ArtifactRepository,
    skill_service: Arc<SkillService>,
    tool_executions: DynToolExecutionLookup,
}

impl std::fmt::Debug for ArtifactPublishingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactPublishingService")
            .finish_non_exhaustive()
    }
}

impl ArtifactPublishingService {
    #[must_use]
    pub fn new(repositories: &A2ARepositories, skill_service: Arc<SkillService>) -> Self {
        Self {
            artifact_repo: repositories.artifacts.clone(),
            skill_service,
            tool_executions: repositories.tool_executions(),
        }
    }

    // Why: an unknown execution id is dropped so the artifact never points at
    // a ledger row that does not exist, but an unreachable ledger is an error
    // — treating it as "unknown" would silently detach every artifact.
    async fn validate_execution_id(&self, artifact: &Artifact) -> Result<Artifact> {
        let mut validated = artifact.clone();

        if let Some(exec_id) = &validated.metadata.mcp_execution_id {
            let exec_id = exec_id.clone();
            let exists = self
                .tool_executions
                .execution_exists(&exec_id)
                .await
                .map_err(|e| {
                    AgentServiceError::operation(
                        format!("Failed to check mcp_execution_id {exec_id}"),
                        e,
                    )
                })?;
            if !exists {
                tracing::warn!(
                    mcp_execution_id = %exec_id,
                    artifact_id = %artifact.id,
                    "mcp_execution_id not found in the tool-execution ledger, setting to NULL"
                );
                validated.metadata.mcp_execution_id = None;
            }
        }

        Ok(validated)
    }

    async fn enrich_artifact_with_skill(&self, artifact: &Artifact, owner: &UserId) -> Artifact {
        let mut enriched = artifact.clone();

        if let Some(skill_id) = &enriched.metadata.skill_id
            && enriched.metadata.skill_name.is_none()
        {
            match self
                .skill_service
                .load_skill_metadata(skill_id, owner)
                .await
            {
                Ok(meta) => enriched.metadata.skill_name = Some(meta.name),
                Err(e) => tracing::debug!(
                    skill_id = %skill_id,
                    error = %e,
                    "skill metadata not available; leaving skill_name empty"
                ),
            }
        }

        enriched
    }

    pub async fn publish_from_a2a(
        &self,
        artifact: &Artifact,
        task: &RuntimeTask,
        owner: &UserId,
    ) -> Result<()> {
        let task_id = task.task_id();
        let context_id = task.context_id();
        let enriched_artifact = self.enrich_artifact_with_skill(artifact, owner).await;
        let validated_artifact = self.validate_execution_id(&enriched_artifact).await?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            artifact_type = %validated_artifact.metadata.artifact_type,
            task_id = %task_id,
            context_id = %context_id,
            source = "a2a_agent",
            "Publishing artifact from A2A agent"
        );

        self.artifact_repo
            .create_artifact(task, &validated_artifact)
            .await
            .map_err(|e| AgentServiceError::operation("Failed to persist artifact", e))?;

        tracing::info!(
            artifact_id = %validated_artifact.id,
            "Artifact persisted to database"
        );

        Ok(())
    }
}
