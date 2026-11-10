//! Owner-scoped artifact reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ArtifactRepository;
use crate::models::a2a::Artifact;
use crate::repository::{OwnedArtifact, OwnedTask};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

impl ArtifactRepository {
    pub async fn owned_artifact(
        &self,
        artifact_id: &ArtifactId,
        user_id: &UserId,
    ) -> Result<OwnedArtifact, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT
                a.artifact_id as "artifact_id!: ArtifactId",
                t.task_id as "task_id!: TaskId",
                t.context_id as "context_id!: ContextId"
            FROM task_artifacts a
            JOIN agent_tasks t ON a.task_id = t.task_id
            JOIN user_contexts c ON t.context_id = c.context_id
            WHERE a.artifact_id = $1 AND c.user_id = $2"#,
            artifact_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or_else(|| RepositoryError::not_found("artifact", artifact_id.as_str()))?;

        Ok(OwnedArtifact::new(
            row.artifact_id,
            row.task_id,
            row.context_id,
            user_id.clone(),
        ))
    }

    pub async fn find_owned_artifact(
        &self,
        owned: &OwnedArtifact,
    ) -> Result<Artifact, RepositoryError> {
        self.find_artifact_by_id(owned.artifact_id())
            .await?
            .ok_or_else(|| RepositoryError::not_found("artifact", owned.artifact_id().as_str()))
    }

    pub async fn artifacts_for_owned_task(
        &self,
        owned: &OwnedTask,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        self.get_artifacts_by_task(owned.task_id()).await
    }
}
