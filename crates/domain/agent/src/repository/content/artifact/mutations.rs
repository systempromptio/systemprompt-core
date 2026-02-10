use super::parts::persist_artifact_part;
use super::ArtifactRepository;
use crate::models::a2a::Artifact;
use chrono::Utc;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};
use systemprompt_traits::RepositoryError;

impl ArtifactRepository {
    pub async fn create_artifact(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
        artifact: &Artifact,
    ) -> Result<(), RepositoryError> {
        let pool = self.write_pool.clone();
        let now = Utc::now();

        let metadata_json = serde_json::json!({
            "rendering_hints": artifact.metadata.rendering_hints,
            "mcp_schema": artifact.metadata.mcp_schema,
            "is_internal": artifact.metadata.is_internal,
            "execution_index": artifact.metadata.execution_index,
        });

        sqlx::query!(
            r#"
            INSERT INTO task_artifacts (
                task_id, context_id, artifact_id, name, description,
                artifact_type, source, tool_name, mcp_execution_id,
                fingerprint, skill_id, skill_name, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)
            ON CONFLICT (task_id, artifact_id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                artifact_type = EXCLUDED.artifact_type,
                source = EXCLUDED.source,
                tool_name = EXCLUDED.tool_name,
                mcp_execution_id = EXCLUDED.mcp_execution_id,
                fingerprint = EXCLUDED.fingerprint,
                skill_id = EXCLUDED.skill_id,
                skill_name = EXCLUDED.skill_name,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at
            "#,
            task_id.as_str(),
            context_id.as_str(),
            artifact.id.as_str(),
            artifact.name.as_deref(),
            artifact.description.as_deref(),
            &artifact.metadata.artifact_type,
            artifact.metadata.source.as_deref(),
            artifact.metadata.tool_name.as_deref(),
            artifact.metadata.mcp_execution_id.as_deref(),
            artifact.metadata.fingerprint.as_deref(),
            artifact.metadata.skill_id.as_deref(),
            artifact.metadata.skill_name.as_deref(),
            metadata_json,
            now
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::database(e))?;

        sqlx::query!(
            "DELETE FROM artifact_parts WHERE artifact_id = $1 AND context_id = $2",
            artifact.id.as_str(),
            context_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::database(e))?;

        for (idx, part) in artifact.parts.iter().enumerate() {
            persist_artifact_part(
                pool.as_ref(),
                part,
                artifact.id.as_str(),
                context_id.as_str(),
                idx as i32,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn delete_artifact(&self, artifact_id: &ArtifactId) -> Result<(), RepositoryError> {
        let pool = self.write_pool.clone();
        let artifact_id_str = artifact_id.as_str();

        sqlx::query!(
            "DELETE FROM task_artifacts WHERE artifact_id = $1",
            artifact_id_str
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::database(e))?;

        Ok(())
    }
}
