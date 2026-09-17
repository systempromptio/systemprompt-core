//! Persistence for MCP tool-result artifacts.
//!
//! Defines [`McpArtifactRepository`] and its row/insert models
//! ([`McpArtifactRecord`], [`CreateMcpArtifact`]) over the `mcp_artifacts`
//! table. An artifact row is one typed result of one execution; its body is
//! content-addressed through `payload_sha256` (see the payload repository) and
//! its correlation keys — session, trace, client `tool_use_id` — are columns,
//! not JSON. Reads go through the read pool and writes through the write pool;
//! expired artifacts are filtered on read and reaped via
//! [`McpArtifactRepository::cleanup_expired`]. Every delete sweeps
//! `artifact_payloads` bodies no artifact references any more, in the same
//! transaction, sparing bodies seen within the last hour that an in-flight
//! ingest may be about to link.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod queries;
mod save;

use crate::error::McpDomainResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiToolCallId, ArtifactId, ContextId, McpExecutionId, SessionId, TraceId, UserId,
};
use systemprompt_models::mcp::ExecutionSource;

#[derive(Debug, Clone)]
pub struct McpArtifactRecord {
    pub id: uuid::Uuid,
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub context_id: Option<ContextId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub server_name: String,
    pub tool_name: Option<String>,
    pub artifact_type: String,
    pub title: Option<String>,
    pub source: String,
    pub last_seen_source: Option<String>,
    // JSON: the stored `ToolResponse` envelope, whose artifact half is typed
    // per `artifact_type`.
    pub data: serde_json::Value,
    // JSON: `ExecutionMetadata` as persisted; the keyed columns are canonical.
    pub metadata: Option<serde_json::Value>,
    pub payload_sha256: Option<String>,
    pub payload_bytes: Option<i32>,
    pub is_structured: bool,
    pub has_ui_resource: bool,
    pub is_error: bool,
    pub secret_redactions: i32,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl McpArtifactRecord {
    #[must_use]
    pub fn source(&self) -> ExecutionSource {
        ExecutionSource::parse(&self.source).unwrap_or(ExecutionSource::InProcess)
    }
}

/// Classification of a result decided at ingestion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArtifactShape {
    pub is_structured: bool,
    pub has_ui_resource: bool,
    pub is_error: bool,
    pub secret_redactions: i32,
}

#[derive(Debug, Clone)]
pub struct CreateMcpArtifact {
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub context_id: Option<ContextId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub server_name: String,
    pub tool_name: Option<String>,
    pub artifact_type: String,
    pub title: Option<String>,
    pub source: ExecutionSource,
    // JSON: the stored `ToolResponse` envelope.
    pub data: serde_json::Value,
    // JSON: `ExecutionMetadata` object.
    pub metadata: Option<serde_json::Value>,
    pub payload_sha256: Option<String>,
    pub payload_bytes: Option<i32>,
    pub shape: ArtifactShape,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateMcpArtifact {
    #[must_use]
    pub fn new(
        artifact_id: ArtifactId,
        mcp_execution_id: McpExecutionId,
        server_name: impl Into<String>,
        artifact_type: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            artifact_id,
            mcp_execution_id,
            context_id: None,
            user_id: None,
            session_id: None,
            trace_id: None,
            ai_tool_call_id: None,
            server_name: server_name.into(),
            tool_name: None,
            artifact_type: artifact_type.into(),
            title: None,
            source: ExecutionSource::InProcess,
            data,
            metadata: None,
            payload_sha256: None,
            payload_bytes: None,
            shape: ArtifactShape::default(),
            expires_at: None,
        }
    }
}

/// Keys learned about an existing artifact from a later vantage point.
#[derive(Debug, Clone, Default)]
pub struct ArtifactCorrelation {
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub last_seen_source: Option<ExecutionSource>,
}

#[derive(Debug)]
pub struct McpArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl McpArtifactRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let pool = db.pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        let write_pool = db.write_pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        Ok(Self { pool, write_pool })
    }

    pub async fn delete(&self, artifact_id: &ArtifactId) -> McpDomainResult<bool> {
        let mut tx = self.write_pool.begin().await?;
        let result = sqlx::query!(
            r#"DELETE FROM mcp_artifacts WHERE artifact_id = $1"#,
            artifact_id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            delete_orphan_payloads(&mut tx).await?;
        }
        tx.commit().await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired(&self) -> McpDomainResult<u64> {
        let mut tx = self.write_pool.begin().await?;
        let result = sqlx::query!(
            r#"DELETE FROM mcp_artifacts WHERE expires_at IS NOT NULL AND expires_at < NOW()"#,
        )
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            delete_orphan_payloads(&mut tx).await?;
        }
        tx.commit().await?;

        Ok(result.rows_affected())
    }
}

// Why: an ingest upserts the body before it saves the artifact that points
// at it, so a body seen within the grace window is presumed in flight and
// left for the next sweep rather than pulled out from under that save.
const ORPHAN_GRACE_SECONDS: f64 = 3600.0;

async fn delete_orphan_payloads(tx: &mut sqlx::PgTransaction<'_>) -> McpDomainResult<u64> {
    let result = sqlx::query!(
        r#"
        DELETE FROM artifact_payloads p
        WHERE NOT EXISTS (SELECT 1 FROM mcp_artifacts a WHERE a.payload_sha256 = p.sha256)
          AND p.last_seen_at < NOW() - make_interval(secs => $1::double precision)
        "#,
        ORPHAN_GRACE_SECONDS
    )
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}
