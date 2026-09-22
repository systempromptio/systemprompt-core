//! Read-side queries over `mcp_tool_executions`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AiToolCallId, ContextId, McpExecutionId, SessionId, UserId};
use systemprompt_models::mcp::{Correlation, ExecutionSource};
use systemprompt_traits::{RepositoryError, ToolExecutionLookup};

use super::ToolUsageRepository;
use crate::error::McpDomainResult;
use crate::models::ToolExecution;

impl ToolUsageRepository {
    pub async fn find_by_fingerprint(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        payload_sha256: &str,
        window_seconds: i64,
    ) -> McpDomainResult<Option<McpExecutionId>> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT mcp_execution_id as "mcp_execution_id!"
            FROM mcp_tool_executions
            WHERE session_id = $1
              AND tool_name = $2
              AND payload_sha256 = $3
              AND started_at > NOW() - make_interval(secs => $4::double precision)
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            session_id.as_str(),
            tool_name,
            payload_sha256,
            window_seconds as f64
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(result.map(McpExecutionId::new))
    }

    // Why: a client hook attests the call it just saw; the server observed
    // the same call moments earlier under the server's own session. Neither
    // shares a key with the other, so the newest unattested server-observed
    // execution of that tool by that user inside the window is the call.
    pub async fn find_unattested_by_proximity(
        &self,
        user_id: &UserId,
        server_name: &str,
        tool_name: &str,
        at: DateTime<Utc>,
        window_seconds: i64,
    ) -> McpDomainResult<Option<McpExecutionId>> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT mcp_execution_id as "mcp_execution_id!"
            FROM mcp_tool_executions
            WHERE source IN ('in_process', 'proxy')
              AND ai_tool_call_id IS NULL
              AND user_id = $1
              AND server_name = $2
              AND tool_name = $3
              AND started_at BETWEEN $4::timestamptz - make_interval(secs => $5::double precision)
                                 AND $4::timestamptz
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            user_id.as_str(),
            server_name,
            tool_name,
            at,
            window_seconds as f64
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(result.map(McpExecutionId::new))
    }

    pub async fn find_by_id(&self, id: &McpExecutionId) -> McpDomainResult<Option<ToolExecution>> {
        let id_str = id.as_str();
        let row = sqlx::query!(
            r#"SELECT
                mcp_execution_id as "mcp_execution_id!",
                tool_name as "tool_name!",
                server_name as "server_name!",
                context_id,
                ai_tool_call_id,
                user_id as "user_id!",
                status as "status!",
                input as "input!",
                output,
                error_message,
                execution_time_ms,
                started_at as "started_at!",
                completed_at,
                source as "source!",
                correlation as "correlation!"
            FROM mcp_tool_executions
            WHERE mcp_execution_id = $1"#,
            id_str
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| ToolExecution {
            mcp_execution_id: McpExecutionId::new(r.mcp_execution_id),
            tool_name: r.tool_name,
            server_name: r.server_name,
            context_id: r.context_id.and_then(|s| {
                ContextId::try_new(&s)
                    .map_err(|e| {
                        tracing::warn!(error = %e, raw = %s, "Skipping non-UUID context_id from mcp_tool_executions row");
                        e
                    })
                    .ok()
            }),
            ai_tool_call_id: r.ai_tool_call_id.map(AiToolCallId::new),
            user_id: UserId::new(r.user_id),
            status: r.status,
            input: r.input,
            output: r.output,
            error_message: r.error_message,
            execution_time_ms: r.execution_time_ms,
            started_at: r.started_at,
            completed_at: r.completed_at,
            source: ExecutionSource::parse(&r.source).unwrap_or(ExecutionSource::InProcess),
            correlation: Correlation::parse(&r.correlation).unwrap_or(Correlation::Exact),
        }))
    }

    pub async fn find_by_ai_call_id(
        &self,
        ai_tool_call_id: &AiToolCallId,
    ) -> McpDomainResult<Option<McpExecutionId>> {
        let id_str = ai_tool_call_id.as_str();
        let result = sqlx::query_scalar!(
            r#"SELECT mcp_execution_id as "mcp_execution_id!" FROM mcp_tool_executions WHERE ai_tool_call_id = $1"#,
            id_str
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(result.map(McpExecutionId::new))
    }
}

#[async_trait]
impl ToolExecutionLookup for ToolUsageRepository {
    async fn execution_exists(&self, id: &McpExecutionId) -> Result<bool, RepositoryError> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1) as "exists!""#,
            id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(RepositoryError::database)
    }
}
