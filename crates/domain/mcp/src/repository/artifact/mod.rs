use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};

#[derive(Debug, Clone)]
pub struct McpArtifactRecord {
    pub id: uuid::Uuid,
    pub artifact_id: String,
    pub mcp_execution_id: String,
    pub context_id: Option<String>,
    pub user_id: Option<String>,
    pub server_name: String,
    pub artifact_type: String,
    pub title: Option<String>,
    pub data: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateMcpArtifact {
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub context_id: Option<String>,
    pub user_id: Option<String>,
    pub server_name: String,
    pub artifact_type: String,
    pub title: Option<String>,
    pub data: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct McpArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl McpArtifactRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db
            .pool_arc()
            .map_err(|e| anyhow::anyhow!("Database must be PostgreSQL: {e}"))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| anyhow::anyhow!("Database must be PostgreSQL: {e}"))?;
        Ok(Self { pool, write_pool })
    }

    pub async fn save(&self, artifact: &CreateMcpArtifact) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_artifacts (
                artifact_id, mcp_execution_id, context_id, user_id,
                server_name, artifact_type, title, data, metadata, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (artifact_id) DO UPDATE SET
                data = EXCLUDED.data,
                metadata = EXCLUDED.metadata,
                title = EXCLUDED.title
            "#,
            artifact.artifact_id.as_str(),
            artifact.mcp_execution_id.as_str(),
            artifact.context_id.as_deref(),
            artifact.user_id.as_deref(),
            &artifact.server_name,
            &artifact.artifact_type,
            artifact.title.as_deref(),
            &artifact.data,
            artifact.metadata.as_ref(),
            artifact.expires_at,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(&self, artifact_id: &ArtifactId) -> Result<Option<McpArtifactRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!",
                mcp_execution_id as "mcp_execution_id!",
                context_id,
                user_id,
                server_name as "server_name!",
                artifact_type as "artifact_type!",
                title,
                data as "data!",
                metadata,
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE artifact_id = $1
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            artifact_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| McpArtifactRecord {
            id: r.id,
            artifact_id: r.artifact_id,
            mcp_execution_id: r.mcp_execution_id,
            context_id: r.context_id,
            user_id: r.user_id,
            server_name: r.server_name,
            artifact_type: r.artifact_type,
            title: r.title,
            data: r.data,
            metadata: r.metadata,
            created_at: r.created_at,
            expires_at: r.expires_at,
        }))
    }

    pub async fn find_by_id_str(&self, artifact_id: &str) -> Result<Option<McpArtifactRecord>> {
        self.find_by_id(&ArtifactId::new(artifact_id)).await
    }

    pub async fn list_by_server(
        &self,
        server_name: &str,
        limit: i64,
    ) -> Result<Vec<McpArtifactRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!",
                mcp_execution_id as "mcp_execution_id!",
                context_id,
                user_id,
                server_name as "server_name!",
                artifact_type as "artifact_type!",
                title,
                data as "data!",
                metadata,
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE server_name = $1
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            server_name,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| McpArtifactRecord {
                id: r.id,
                artifact_id: r.artifact_id,
                mcp_execution_id: r.mcp_execution_id,
                context_id: r.context_id,
                user_id: r.user_id,
                server_name: r.server_name,
                artifact_type: r.artifact_type,
                title: r.title,
                data: r.data,
                metadata: r.metadata,
                created_at: r.created_at,
                expires_at: r.expires_at,
            })
            .collect())
    }

    pub async fn delete(&self, artifact_id: &ArtifactId) -> Result<bool> {
        let result = sqlx::query!(
            r#"DELETE FROM mcp_artifacts WHERE artifact_id = $1"#,
            artifact_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query!(
            r#"DELETE FROM mcp_artifacts WHERE expires_at IS NOT NULL AND expires_at < NOW()"#,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
