//! Stamping an execution with the model's tool-call intent it fulfils.
//!
//! The intent itself is claimed through `ToolCallIntentClaims`; this side
//! only records the claimed call id on the execution row, and says which
//! execution already holds the call id when the stamp is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AiToolCallId, McpExecutionId};
use systemprompt_models::mcp::Correlation;

use super::ToolUsageRepository;
use crate::error::McpDomainResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentStamp {
    Stamped,
    HeldBy(McpExecutionId),
    Refused,
}

impl ToolUsageRepository {
    pub async fn stamp_inferred_intent(
        &self,
        mcp_execution_id: &McpExecutionId,
        ai_tool_call_id: &AiToolCallId,
    ) -> McpDomainResult<IntentStamp> {
        let stamped = sqlx::query!(
            r#"
            UPDATE mcp_tool_executions
            SET ai_tool_call_id = $2, correlation = $3
            WHERE mcp_execution_id = $1
              AND ai_tool_call_id IS NULL
              AND NOT EXISTS (
                  SELECT 1 FROM mcp_tool_executions o WHERE o.ai_tool_call_id = $2
              )
            "#,
            mcp_execution_id.as_str(),
            ai_tool_call_id.as_str(),
            Correlation::Inferred.as_str()
        )
        .execute(&*self.write_pool)
        .await;
        match stamped {
            Ok(done) if done.rows_affected() > 0 => return Ok(IntentStamp::Stamped),
            Ok(_) => {},
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {},
            Err(e) => return Err(e.into()),
        }
        let holder = sqlx::query_scalar!(
            r#"SELECT mcp_execution_id AS "mcp_execution_id!" FROM mcp_tool_executions WHERE ai_tool_call_id = $1"#,
            ai_tool_call_id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(match holder {
            Some(holder) if holder == mcp_execution_id.as_str() => IntentStamp::Stamped,
            Some(holder) => IntentStamp::HeldBy(McpExecutionId::new(holder)),
            None => IntentStamp::Refused,
        })
    }
}
