use crate::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FilePart, FileWithBytes, Part, TextPart,
};
use crate::models::{ArtifactPartRow, ArtifactRow};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_traits::{Repository as RepositoryTrait, RepositoryError};

#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    db_pool: DbPool,
}

impl RepositoryTrait for ArtifactRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}

impl ArtifactRepository {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
        self.db_pool
            .as_ref()
            .get_postgres_pool()
            .ok_or_else(|| RepositoryError::Database("PostgreSQL pool not available".to_string()))
    }

    pub async fn create_artifact(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
        artifact: &Artifact,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        sqlx::query!(
            "DELETE FROM artifact_parts WHERE artifact_id = $1 AND context_id = $2",
            artifact.id.as_str(),
            context_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

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

    pub async fn delete_artifact(&self, artifact_id: &ArtifactId) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        let artifact_id_str = artifact_id.as_str();

        sqlx::query!(
            "DELETE FROM task_artifacts WHERE artifact_id = $1",
            artifact_id_str
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
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

    let rendering_hints =
        metadata.get("rendering_hints").and_then(
            |v| {
                if v.is_null() {
                    None
                } else {
                    Some(v.clone())
                }
            },
        );

    let mcp_schema =
        metadata
            .get("mcp_schema")
            .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });

    let is_internal = metadata.get("is_internal").and_then(|v| v.as_bool());

    let execution_index = metadata
        .get("execution_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    (rendering_hints, mcp_schema, is_internal, execution_index)
}

pub async fn get_artifact_parts(
    pool: &PgPool,
    artifact_id: &str,
    context_id: &str,
) -> Result<Vec<Part>, RepositoryError> {
    let part_rows = sqlx::query_as!(
        ArtifactPartRow,
        r#"SELECT
            id as "id!",
            artifact_id as "artifact_id!",
            context_id as "context_id!",
            part_kind as "part_kind!",
            sequence_number as "sequence_number!",
            text_content,
            file_name,
            file_mime_type,
            file_uri,
            file_bytes,
            data_content,
            metadata
        FROM artifact_parts
        WHERE artifact_id = $1 AND context_id = $2
        ORDER BY sequence_number ASC"#,
        artifact_id,
        context_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let mut parts = Vec::new();

    for row in part_rows {
        let part = match row.part_kind.as_str() {
            "text" => {
                let text = row
                    .text_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
                Part::Text(TextPart { text })
            },
            "file" => {
                let bytes = row
                    .file_bytes
                    .ok_or_else(|| RepositoryError::InvalidData("Missing file_bytes".into()))?;
                Part::File(FilePart {
                    file: FileWithBytes {
                        name: row.file_name,
                        mime_type: row.file_mime_type,
                        bytes,
                    },
                })
            },
            "data" => {
                let data_value = row
                    .data_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing data_content".into()))?;
                let data = if let serde_json::Value::Object(map) = data_value {
                    map
                } else {
                    return Err(RepositoryError::InvalidData(
                        "Data content must be a JSON object".into(),
                    ));
                };
                Part::Data(DataPart { data })
            },
            _ => {
                return Err(RepositoryError::InvalidData(format!(
                    "Unknown part kind: {}",
                    row.part_kind
                )));
            },
        };

        parts.push(part);
    }

    Ok(parts)
}

pub async fn persist_artifact_part(
    pool: &PgPool,
    part: &Part,
    artifact_id: &str,
    context_id: &str,
    sequence_number: i32,
) -> Result<(), RepositoryError> {
    match part {
        Part::Text(text_part) => {
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, text_content)
                VALUES ($1, $2, 'text', $3, $4)"#,
                artifact_id,
                context_id,
                sequence_number,
                text_part.text
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::File(file_part) => {
            let file_uri: Option<&str> = None;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7)"#,
                artifact_id,
                context_id,
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_uri,
                file_part.file.bytes
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::Data(data_part) => {
            let data_json = serde_json::to_value(&data_part.data)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, data_content)
                VALUES ($1, $2, 'data', $3, $4)"#,
                artifact_id,
                context_id,
                sequence_number,
                data_json
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
    }

    Ok(())
}
