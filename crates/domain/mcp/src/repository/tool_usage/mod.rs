//! Tool-usage repository — persists each MCP tool execution and answers the
//! cross-domain [`ToolExecutionLookup`] seam over the same ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
mod intent;
mod queries;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId};
use systemprompt_models::mcp::Correlation;
use uuid::Uuid;

use crate::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_models::RequestContext;

fn extract_trace_id(ctx: &RequestContext) -> Option<String> {
    let trace_id = ctx.trace_id();
    (!trace_id.as_str().is_empty()).then(|| trace_id.to_string())
}

#[derive(Debug, Clone)]
pub struct ToolUsageRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ToolUsageRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let pool = db.pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        let write_pool = db.write_pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        Ok(Self { pool, write_pool })
    }

    pub async fn start_execution(
        &self,
        mcp_execution_id: &McpExecutionId,
        request: &ToolExecutionRequest,
        correlation: Correlation,
    ) -> McpDomainResult<()> {
        let id = mcp_execution_id.as_str();
        let context_id = request.context.context_id().to_string();
        let user_id = request.context.user_id().to_string();
        let ai_tool_call_id = request.ai_tool_call_id.as_ref().map(ToString::to_string);
        let input_str = serde_json::to_string(&request.input)?;
        let task_id = request.context.task_id().map(ToString::to_string);
        let session_id = request.context.session_id().to_string();
        let trace_id = extract_trace_id(&request.context);
        let status = ExecutionStatus::Pending.as_str();
        let (actor_kind, actor_id) = request.context.auth.actor.audit_columns();
        sqlx::query!(
            r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, context_id, ai_tool_call_id,
                user_id, task_id, session_id, trace_id, status, input, started_at,
                request_method, request_source, actor_kind, actor_id, source, correlation
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
            id,
            request.tool_name,
            request.server_name,
            context_id,
            ai_tool_call_id,
            user_id,
            task_id,
            session_id,
            trace_id,
            status,
            input_str,
            request.started_at,
            request.request_method,
            request.request_source,
            actor_kind,
            actor_id,
            request.source.as_str(),
            correlation.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn complete_execution(
        &self,
        mcp_execution_id: &McpExecutionId,
        result: &ToolExecutionResult,
    ) -> McpDomainResult<()> {
        let id = mcp_execution_id.as_str();
        let duration_ms = (result.completed_at - result.started_at).num_milliseconds() as i32;
        let output_str = result.output.as_ref().and_then(|v| {
            serde_json::to_string(v)
                .map_err(|e| {
                    tracing::error!(
                        mcp_execution_id = %id,
                        error = %e,
                        "Failed to serialize tool execution output"
                    );
                    e
                })
                .ok()
        });

        sqlx::query!(
            r#"
            UPDATE mcp_tool_executions
            SET status = $1, output = $2, error_message = $3, execution_time_ms = $4, completed_at = $5
            WHERE mcp_execution_id = $6
            "#,
            result.status,
            output_str,
            result.error_message,
            duration_ms,
            result.completed_at,
            id
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn log_execution_sync(
        &self,
        request: &ToolExecutionRequest,
        result: &ToolExecutionResult,
    ) -> McpDomainResult<McpExecutionId> {
        let id = McpExecutionId::new(Uuid::new_v4().to_string());
        self.log_execution_sync_with_id(&id, request, result, Correlation::Exact)
            .await?;
        Ok(id)
    }

    pub async fn log_execution_sync_with_id(
        &self,
        mcp_execution_id: &McpExecutionId,
        request: &ToolExecutionRequest,
        result: &ToolExecutionResult,
        correlation: Correlation,
    ) -> McpDomainResult<()> {
        let status = ExecutionStatus::from_error(result.error_message.is_some()).as_str();
        let context_id = request.context.context_id().to_string();
        let user_id = request.context.user_id().to_string();
        let task_id = request.context.task_id().map(ToString::to_string);
        let session_id = request.context.session_id().to_string();
        let trace_id = extract_trace_id(&request.context);
        let ai_tool_call_id = request.ai_tool_call_id.as_ref().map(ToString::to_string);
        let duration_ms = (result.completed_at - request.started_at).num_milliseconds() as i32;
        let input_str = serde_json::to_string(&request.input)?;
        let output_str = result
            .output
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let (actor_kind, actor_id) = request.context.auth.actor.audit_columns();

        sqlx::query!(
            r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, context_id, user_id, task_id,
                session_id, trace_id, status, input, output, error_message, execution_time_ms,
                started_at, completed_at, request_method, request_source, actor_kind, actor_id,
                ai_tool_call_id, source, correlation
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                    $18, $19, $20, $21, $22)
            "#,
            mcp_execution_id.as_str(),
            request.tool_name,
            request.server_name,
            context_id,
            user_id,
            task_id,
            session_id,
            trace_id,
            status,
            input_str,
            output_str,
            result.error_message,
            duration_ms,
            request.started_at,
            result.completed_at,
            request.request_method,
            request.request_source,
            actor_kind,
            actor_id,
            ai_tool_call_id,
            request.source.as_str(),
            correlation.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn mark_correlated(
        &self,
        mcp_execution_id: &McpExecutionId,
        ai_tool_call_id: Option<&AiToolCallId>,
        correlation: Correlation,
        payload_sha256: Option<&str>,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_tool_executions
            SET ai_tool_call_id = COALESCE(ai_tool_call_id, $2),
                correlation = $3,
                payload_sha256 = COALESCE($4, payload_sha256)
            WHERE mcp_execution_id = $1
            "#,
            mcp_execution_id.as_str(),
            ai_tool_call_id.map(AiToolCallId::as_str),
            correlation.as_str(),
            payload_sha256
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
