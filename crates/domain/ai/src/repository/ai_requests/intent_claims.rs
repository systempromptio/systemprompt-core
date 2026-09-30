//! `ToolCallIntentClaims` over the tool-call intents this crate owns: an MCP
//! execution stamps itself onto the `ai_request_tool_calls` row it fulfils.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, SessionId};
use systemprompt_traits::{RepositoryError, ToolCallIntentClaims};

use super::AiRequestRepository;

fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[async_trait]
impl ToolCallIntentClaims for AiRequestRepository {
    // Why: the newest matching intent is locked and stamped in one statement,
    // so two concurrent calls of the same tool in one session can never both
    // take it; `SKIP LOCKED` hands the second caller the next intent, or none.
    async fn claim_newest_unclaimed(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        execution: &McpExecutionId,
        window_seconds: i64,
    ) -> Result<Option<AiToolCallId>, RepositoryError> {
        let suffix = format!("%\\_\\_{}", escape_like(tool_name));
        let claimed = sqlx::query_scalar!(
            r#"
            UPDATE ai_request_tool_calls c
            SET mcp_execution_id = $5, updated_at = NOW()
            WHERE c.id = (
                SELECT i.id
                FROM ai_request_tool_calls i
                JOIN ai_requests r ON r.id = i.request_id
                WHERE r.session_id = $1
                  AND i.ai_tool_call_id IS NOT NULL
                  AND i.mcp_execution_id IS NULL
                  AND (i.tool_name = $2 OR i.tool_name LIKE $3)
                  AND i.created_at > NOW() - make_interval(secs => $4::double precision)
                ORDER BY i.created_at DESC
                LIMIT 1
                FOR UPDATE OF i SKIP LOCKED
            )
              AND c.mcp_execution_id IS NULL
            RETURNING c.ai_tool_call_id AS "ai_tool_call_id!"
            "#,
            session_id.as_str(),
            tool_name,
            suffix,
            window_seconds as f64,
            execution.as_str()
        )
        .fetch_optional(self.write_pool())
        .await
        .map_err(RepositoryError::database)?;
        Ok(claimed.map(AiToolCallId::new))
    }

    async fn claim(
        &self,
        call: &AiToolCallId,
        execution: &McpExecutionId,
    ) -> Result<bool, RepositoryError> {
        let claimed = sqlx::query!(
            r#"
            UPDATE ai_request_tool_calls
            SET mcp_execution_id = $2, updated_at = NOW()
            WHERE ai_tool_call_id = $1 AND mcp_execution_id IS NULL
            "#,
            call.as_str(),
            execution.as_str()
        )
        .execute(self.write_pool())
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();
        Ok(claimed > 0)
    }

    async fn release(
        &self,
        call: &AiToolCallId,
        execution: &McpExecutionId,
    ) -> Result<bool, RepositoryError> {
        let released = sqlx::query!(
            r#"
            UPDATE ai_request_tool_calls
            SET mcp_execution_id = NULL, updated_at = NOW()
            WHERE ai_tool_call_id = $1 AND mcp_execution_id = $2
            "#,
            call.as_str(),
            execution.as_str()
        )
        .execute(self.write_pool())
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();
        Ok(released > 0)
    }
}
