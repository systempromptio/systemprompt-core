//! Writes to `mcp_artifacts`: insert, and enrichment of an existing row's
//! correlation keys from a later vantage point.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ArtifactCorrelation, CreateMcpArtifact, McpArtifactRepository};
use crate::error::McpDomainResult;
use crate::repository::is_unique_violation_on;

const EXECUTION_INDEX: &str = "idx_mcp_artifacts_execution";

/// What inserting an artifact row ended as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactSave {
    Saved,
    ExecutionHeld,
}
use systemprompt_identifiers::ArtifactId;
use systemprompt_models::mcp::ExecutionSource;

impl McpArtifactRepository {
    pub async fn save(&self, artifact: &CreateMcpArtifact) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_artifacts (
                artifact_id, mcp_execution_id, context_id, user_id, session_id, trace_id,
                ai_tool_call_id, server_name, tool_name, artifact_type, title, source,
                last_seen_source, data, metadata, payload_sha256, payload_bytes,
                is_structured, has_ui_resource, is_error, secret_redactions, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12, $13, $14, $15, $16,
                    $17, $18, $19, $20, $21)
            ON CONFLICT (artifact_id) DO UPDATE SET
                data = EXCLUDED.data,
                metadata = EXCLUDED.metadata,
                title = EXCLUDED.title,
                payload_sha256 = EXCLUDED.payload_sha256,
                payload_bytes = EXCLUDED.payload_bytes,
                last_seen_source = EXCLUDED.source
            "#,
            artifact.artifact_id as _,
            artifact.mcp_execution_id as _,
            artifact.context_id as _,
            artifact.user_id as _,
            artifact.session_id as _,
            artifact.trace_id as _,
            artifact.ai_tool_call_id as _,
            artifact.server_name as _,
            artifact.tool_name as _,
            &artifact.artifact_type,
            artifact.title.as_deref(),
            artifact.source.as_str(),
            &artifact.data,
            artifact.metadata.as_ref(),
            artifact.payload_sha256.as_deref(),
            artifact.payload_bytes,
            artifact.shape.is_structured,
            artifact.shape.has_ui_resource,
            artifact.shape.is_error,
            artifact.shape.secret_redactions,
            artifact.expires_at,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn save_for_execution(
        &self,
        artifact: &CreateMcpArtifact,
    ) -> McpDomainResult<ArtifactSave> {
        match self.save(artifact).await {
            Ok(()) => Ok(ArtifactSave::Saved),
            Err(e) if is_unique_violation_on(&e, EXECUTION_INDEX) => {
                Ok(ArtifactSave::ExecutionHeld)
            },
            Err(e) => Err(e),
        }
    }

    pub async fn enrich_correlation(
        &self,
        artifact_id: &ArtifactId,
        keys: &ArtifactCorrelation,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_artifacts
            SET session_id = COALESCE(session_id, $2),
                trace_id = COALESCE(trace_id, $3),
                ai_tool_call_id = COALESCE(ai_tool_call_id, $4),
                last_seen_source = COALESCE($5, last_seen_source)
            WHERE artifact_id = $1
            "#,
            artifact_id as _,
            keys.session_id as _,
            keys.trace_id as _,
            keys.ai_tool_call_id as _,
            keys.last_seen_source.map(ExecutionSource::as_str),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
