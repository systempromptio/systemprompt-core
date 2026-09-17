//! Reads from `mcp_artifacts`, by artifact id, execution, client call id,
//! server, and session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{McpArtifactRecord, McpArtifactRepository};
use crate::error::McpDomainResult;
use systemprompt_identifiers::{
    AiToolCallId, ArtifactId, ContextId, McpExecutionId, SessionId, TraceId, UserId,
};

impl McpArtifactRepository {
    pub async fn find_by_id(
        &self,
        artifact_id: &ArtifactId,
    ) -> McpDomainResult<Option<McpArtifactRecord>> {
        Ok(sqlx::query_as!(
            McpArtifactRecord,
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!: ArtifactId",
                mcp_execution_id as "mcp_execution_id!: McpExecutionId",
                context_id as "context_id: ContextId",
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                trace_id as "trace_id: TraceId",
                ai_tool_call_id as "ai_tool_call_id: AiToolCallId",
                server_name as "server_name!",
                tool_name,
                artifact_type as "artifact_type!",
                title,
                source as "source!",
                last_seen_source,
                data as "data!",
                metadata,
                payload_sha256,
                payload_bytes,
                is_structured as "is_structured!",
                has_ui_resource as "has_ui_resource!",
                is_error as "is_error!",
                secret_redactions as "secret_redactions!",
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE (expires_at IS NULL OR expires_at > NOW())
              AND artifact_id = $1
            "#,
            artifact_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?)
    }

    pub async fn find_by_execution_id(
        &self,
        mcp_execution_id: &McpExecutionId,
    ) -> McpDomainResult<Option<McpArtifactRecord>> {
        Ok(sqlx::query_as!(
            McpArtifactRecord,
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!: ArtifactId",
                mcp_execution_id as "mcp_execution_id!: McpExecutionId",
                context_id as "context_id: ContextId",
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                trace_id as "trace_id: TraceId",
                ai_tool_call_id as "ai_tool_call_id: AiToolCallId",
                server_name as "server_name!",
                tool_name,
                artifact_type as "artifact_type!",
                title,
                source as "source!",
                last_seen_source,
                data as "data!",
                metadata,
                payload_sha256,
                payload_bytes,
                is_structured as "is_structured!",
                has_ui_resource as "has_ui_resource!",
                is_error as "is_error!",
                secret_redactions as "secret_redactions!",
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE (expires_at IS NULL OR expires_at > NOW())
              AND mcp_execution_id = $1
            "#,
            mcp_execution_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?)
    }

    /// The artifact of the execution a client `tool_use_id` names, whichever
    /// vantage point recorded it.
    pub async fn find_by_ai_tool_call_id(
        &self,
        ai_tool_call_id: &AiToolCallId,
    ) -> McpDomainResult<Option<McpArtifactRecord>> {
        Ok(sqlx::query_as!(
            McpArtifactRecord,
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!: ArtifactId",
                mcp_execution_id as "mcp_execution_id!: McpExecutionId",
                context_id as "context_id: ContextId",
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                trace_id as "trace_id: TraceId",
                ai_tool_call_id as "ai_tool_call_id: AiToolCallId",
                server_name as "server_name!",
                tool_name,
                artifact_type as "artifact_type!",
                title,
                source as "source!",
                last_seen_source,
                data as "data!",
                metadata,
                payload_sha256,
                payload_bytes,
                is_structured as "is_structured!",
                has_ui_resource as "has_ui_resource!",
                is_error as "is_error!",
                secret_redactions as "secret_redactions!",
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE (expires_at IS NULL OR expires_at > NOW())
              AND ai_tool_call_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            ai_tool_call_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?)
    }

    pub async fn list_by_server(
        &self,
        server_name: &str,
        limit: i64,
    ) -> McpDomainResult<Vec<McpArtifactRecord>> {
        Ok(sqlx::query_as!(
            McpArtifactRecord,
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!: ArtifactId",
                mcp_execution_id as "mcp_execution_id!: McpExecutionId",
                context_id as "context_id: ContextId",
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                trace_id as "trace_id: TraceId",
                ai_tool_call_id as "ai_tool_call_id: AiToolCallId",
                server_name as "server_name!",
                tool_name,
                artifact_type as "artifact_type!",
                title,
                source as "source!",
                last_seen_source,
                data as "data!",
                metadata,
                payload_sha256,
                payload_bytes,
                is_structured as "is_structured!",
                has_ui_resource as "has_ui_resource!",
                is_error as "is_error!",
                secret_redactions as "secret_redactions!",
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE (expires_at IS NULL OR expires_at > NOW())
              AND server_name = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            server_name,
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub async fn list_by_session(
        &self,
        session_id: &SessionId,
        limit: i64,
    ) -> McpDomainResult<Vec<McpArtifactRecord>> {
        Ok(sqlx::query_as!(
            McpArtifactRecord,
            r#"
            SELECT
                id as "id!",
                artifact_id as "artifact_id!: ArtifactId",
                mcp_execution_id as "mcp_execution_id!: McpExecutionId",
                context_id as "context_id: ContextId",
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                trace_id as "trace_id: TraceId",
                ai_tool_call_id as "ai_tool_call_id: AiToolCallId",
                server_name as "server_name!",
                tool_name,
                artifact_type as "artifact_type!",
                title,
                source as "source!",
                last_seen_source,
                data as "data!",
                metadata,
                payload_sha256,
                payload_bytes,
                is_structured as "is_structured!",
                has_ui_resource as "has_ui_resource!",
                is_error as "is_error!",
                secret_redactions as "secret_redactions!",
                created_at as "created_at!",
                expires_at
            FROM mcp_artifacts
            WHERE (expires_at IS NULL OR expires_at > NOW())
              AND session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            session_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
