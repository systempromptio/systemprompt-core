//! Writes to `mcp_artifacts`: insert, and enrichment of an existing row's
//! correlation keys from a later vantage point.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ArtifactCorrelation, CreateMcpArtifact, McpArtifactRepository};
use crate::error::McpDomainResult;
use systemprompt_identifiers::{AiToolCallId, ArtifactId, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::mcp::ExecutionSource;

impl McpArtifactRepository {
    /// Inserts an artifact. A repeat of the same `artifact_id` refreshes the
    /// body and title — the in-process builder's own re-emission — and notes
    /// the vantage point it was last seen from.
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
            artifact.artifact_id.as_str(),
            artifact.mcp_execution_id.as_str(),
            artifact.context_id.as_ref().map(ContextId::as_str),
            artifact.user_id.as_ref().map(UserId::as_str),
            artifact.session_id.as_ref().map(SessionId::as_str),
            artifact.trace_id.as_ref().map(TraceId::as_str),
            artifact.ai_tool_call_id.as_ref().map(AiToolCallId::as_str),
            &artifact.server_name,
            artifact.tool_name.as_deref(),
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

    /// Fills correlation keys the original vantage point did not have. Never
    /// overwrites a key that is already set and never touches the body: a
    /// client's copy of a result can be reshaped, the server's cannot.
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
            artifact_id.as_str(),
            keys.session_id.as_ref().map(SessionId::as_str),
            keys.trace_id.as_ref().map(TraceId::as_str),
            keys.ai_tool_call_id.as_ref().map(AiToolCallId::as_str),
            keys.last_seen_source.map(ExecutionSource::as_str),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
