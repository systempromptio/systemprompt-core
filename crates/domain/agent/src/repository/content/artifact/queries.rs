use super::parts::get_artifact_parts;
use super::ArtifactRepository;
use crate::models::a2a::{Artifact, ArtifactMetadata};
use crate::models::ArtifactRow;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

impl ArtifactRepository {
    pub async fn get_artifacts_by_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let task_id_str = task_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!",
                task_id as "task_id!",
                context_id,
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id,
                fingerprint,
                skill_id,
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut artifacts = Vec::new();
        for row in rows {
            let artifact = row_to_artifact(&pool, row).await?;
            artifacts.push(artifact);
        }

        Ok(artifacts)
    }

    pub async fn get_artifacts_by_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let context_id_str = context_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!",
                task_id as "task_id!",
                context_id,
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id,
                fingerprint,
                skill_id,
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut artifacts = Vec::new();
        for row in rows {
            let artifact = row_to_artifact(&pool, row).await?;
            artifacts.push(artifact);
        }

        Ok(artifacts)
    }

    pub async fn get_artifacts_by_user_id(
        &self,
        user_id: &UserId,
        limit: Option<i32>,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let limit = i64::from(limit.unwrap_or(100));
        let user_id_str = user_id.as_str();

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                a.artifact_id as "artifact_id!",
                a.task_id as "task_id!",
                a.context_id,
                a.name,
                a.description,
                a.artifact_type as "artifact_type!",
                a.source,
                a.tool_name,
                a.mcp_execution_id,
                a.fingerprint,
                a.skill_id,
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut artifacts = Vec::new();
        for row in rows {
            let artifact = row_to_artifact(&pool, row).await?;
            artifacts.push(artifact);
        }

        Ok(artifacts)
    }

    pub async fn get_artifact_by_id(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<Option<Artifact>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let artifact_id_str = artifact_id.as_str();

        let row = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!",
                task_id as "task_id!",
                context_id,
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id,
                fingerprint,
                skill_id,
                skill_name,
                metadata,
                created_at as "created_at!"
            FROM task_artifacts
            WHERE artifact_id = $1"#,
            artifact_id_str
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

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
        let pool = self.get_pg_pool()?;
        let limit = i64::from(limit.unwrap_or(100));

        let rows = sqlx::query_as!(
            ArtifactRow,
            r#"SELECT
                artifact_id as "artifact_id!",
                task_id as "task_id!",
                context_id,
                name,
                description,
                artifact_type as "artifact_type!",
                source,
                tool_name,
                mcp_execution_id,
                fingerprint,
                skill_id,
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut artifacts = Vec::new();
        for row in rows {
            let artifact = row_to_artifact(&pool, row).await?;
            artifacts.push(artifact);
        }

        Ok(artifacts)
    }
}

async fn row_to_artifact(
    pool: &Arc<PgPool>,
    row: ArtifactRow,
) -> Result<Artifact, RepositoryError> {
    let context_id_str = row.context_id.clone().unwrap_or_else(String::new);
    let parts = get_artifact_parts(pool, &row.artifact_id, &context_id_str).await?;

    let (rendering_hints, mcp_schema, is_internal, execution_index) =
        extract_metadata_fields(&row.metadata);

    Ok(Artifact {
        id: row.artifact_id.into(),
        name: row.name,
        description: row.description,
        parts,
        extensions: vec![],
        metadata: ArtifactMetadata {
            artifact_type: row.artifact_type,
            context_id: ContextId::new(row.context_id.unwrap_or_else(String::new)),
            created_at: row.created_at.to_rfc3339(),
            task_id: TaskId::new(row.task_id),
            rendering_hints,
            source: row.source,
            mcp_execution_id: row.mcp_execution_id,
            mcp_schema,
            is_internal,
            fingerprint: row.fingerprint,
            tool_name: row.tool_name,
            execution_index,
            skill_id: row.skill_id,
            skill_name: row.skill_name,
        },
    })
}

fn extract_metadata_fields(
    metadata: &Option<serde_json::Value>,
) -> (
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<bool>,
    Option<usize>,
) {
    let Some(metadata) = metadata else {
        return (None, None, None, None);
    };

    let rendering_hints = metadata.get("rendering_hints").and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(v.clone())
        }
    });

    let mcp_schema = metadata
        .get("mcp_schema")
        .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });

    let is_internal = metadata.get("is_internal").and_then(|v| v.as_bool());

    let execution_index = metadata
        .get("execution_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    (rendering_hints, mcp_schema, is_internal, execution_index)
}
