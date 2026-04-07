use super::ArtifactRepository;
use super::converters::{row_to_artifact_with_parts, rows_to_artifacts_batch};
use super::parts::get_artifact_parts;
use crate::models::ArtifactRow;
use crate::models::a2a::Artifact;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{ArtifactId, ContextId, McpExecutionId, SkillId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

impl ArtifactRepository {
    pub async fn get_artifacts_by_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = Arc::clone(&self.pool);
        let task_id_str = task_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!: ArtifactId",
                task_id as "task_id!: TaskId",
                context_id as "context_id?: ContextId",
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id as "mcp_execution_id?: McpExecutionId",
                fingerprint,
                skill_id as "skill_id?: SkillId",
                skill_name,
                metadata,
                created_at as "created_at!"
            FROM task_artifacts
            WHERE task_id = $1
            ORDER BY created_at DESC"#,
            task_id_str
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        rows_to_artifacts_batch(&pool, rows).await
    }

    pub async fn get_artifacts_by_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = Arc::clone(&self.pool);
        let context_id_str = context_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!: ArtifactId",
                task_id as "task_id!: TaskId",
                context_id as "context_id?: ContextId",
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id as "mcp_execution_id?: McpExecutionId",
                fingerprint,
                skill_id as "skill_id?: SkillId",
                skill_name,
                metadata,
                created_at as "created_at!"
            FROM task_artifacts
            WHERE context_id = $1
            ORDER BY created_at DESC"#,
            context_id_str
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        rows_to_artifacts_batch(&pool, rows).await
    }

    pub async fn get_artifacts_by_user_id(
        &self,
        user_id: &UserId,
        limit: Option<i32>,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = Arc::clone(&self.pool);
        let limit = i64::from(limit.unwrap_or(100));
        let user_id_str = user_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                a.artifact_id as "artifact_id!: ArtifactId",
                a.task_id as "task_id!: TaskId",
                a.context_id as "context_id?: ContextId",
                a.name,
                a.description,
                a.artifact_type as "artifact_type!",
                a.source,
                a.tool_name,
                a.mcp_execution_id as "mcp_execution_id?: McpExecutionId",
                a.fingerprint,
                a.skill_id as "skill_id?: SkillId",
                a.skill_name,
                a.metadata,
                a.created_at as "created_at!"
            FROM task_artifacts a
            JOIN agent_tasks t ON a.task_id = t.task_id
            WHERE t.user_id = $1
            ORDER BY a.created_at DESC
            LIMIT $2"#,
            user_id_str,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        rows_to_artifacts_batch(&pool, rows).await
    }

    pub async fn get_artifact_by_id(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<Option<Artifact>, RepositoryError> {
        let pool = Arc::clone(&self.pool);
        let artifact_id_str = artifact_id.as_str();

        let row = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!: ArtifactId",
                task_id as "task_id!: TaskId",
                context_id as "context_id?: ContextId",
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id as "mcp_execution_id?: McpExecutionId",
                fingerprint,
                skill_id as "skill_id?: SkillId",
                skill_name,
                metadata,
                created_at as "created_at!"
            FROM task_artifacts
            WHERE artifact_id = $1"#,
            artifact_id_str
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        match row {
            Some(row) => {
                let artifact = row_to_artifact(&pool, row).await?;
                Ok(Some(artifact))
            },
            None => Ok(None),
        }
    }

    pub async fn get_all_artifacts(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = Arc::clone(&self.pool);
        let limit = i64::from(limit.unwrap_or(100));

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!: ArtifactId",
                task_id as "task_id!: TaskId",
                context_id as "context_id?: ContextId",
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id as "mcp_execution_id?: McpExecutionId",
                fingerprint,
                skill_id as "skill_id?: SkillId",
                skill_name,
                metadata,
                created_at as "created_at!"
            FROM task_artifacts
            ORDER BY created_at DESC
            LIMIT $1"#,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        rows_to_artifacts_batch(&pool, rows).await
    }
}

async fn row_to_artifact(
    pool: &Arc<PgPool>,
    row: ArtifactRow,
) -> Result<Artifact, RepositoryError> {
    let context_id = row
        .context_id
        .clone()
        .unwrap_or_else(|| ContextId::new(""));
    let parts = get_artifact_parts(pool, row.artifact_id.as_str(), context_id.as_str()).await?;
    Ok(row_to_artifact_with_parts(row, parts))
}
