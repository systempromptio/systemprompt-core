//! Joining an execution to the model's tool-call intent it fulfils.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AiToolCallId, McpExecutionId, SessionId};
use systemprompt_models::mcp::Correlation;

use super::ToolUsageRepository;
use crate::error::McpDomainResult;

impl ToolUsageRepository {
    // Why: the newest matching intent is locked, stamped with the execution
    // and mirrored onto the execution row in one transaction, so two
    // concurrent calls of the same tool in one session can never both take
    // it; `SKIP LOCKED` hands the second caller the next intent, or none.
    pub async fn claim_unclaimed_intent(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        mcp_execution_id: &McpExecutionId,
        window_seconds: i64,
    ) -> McpDomainResult<Option<AiToolCallId>> {
        let suffix = format!("%\\_\\_{}", escape_like(tool_name));
        let mut tx = self.write_pool.begin().await?;
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
                  AND NOT EXISTS (
                      SELECT 1 FROM mcp_tool_executions e
                      WHERE e.ai_tool_call_id = i.ai_tool_call_id
                  )
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
            mcp_execution_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(call_id) = &claimed {
            sqlx::query!(
                r#"
                UPDATE mcp_tool_executions
                SET ai_tool_call_id = $2, correlation = $3
                WHERE mcp_execution_id = $1
                "#,
                mcp_execution_id.as_str(),
                call_id,
                Correlation::Inferred.as_str()
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(claimed.map(AiToolCallId::new))
    }

    pub async fn claim_intent(
        &self,
        ai_tool_call_id: &AiToolCallId,
        mcp_execution_id: &McpExecutionId,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE ai_request_tool_calls
            SET mcp_execution_id = $2, updated_at = NOW()
            WHERE ai_tool_call_id = $1 AND mcp_execution_id IS NULL
            "#,
            ai_tool_call_id.as_str(),
            mcp_execution_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}

fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
