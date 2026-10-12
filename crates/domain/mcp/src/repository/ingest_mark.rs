//! High-water marks for gateway `tool_result` ingestion
//! (`tool_result_ingest_marks`).
//!
//! One row per replayed conversation: the last message whose results were
//! ingested and the `tool_use_id` that pins it. The gateway reads the mark
//! before a turn and writes it after every result in the turn was ingested.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiToolCallId, UserId};

use crate::error::McpDomainResult;

/// Which replayed conversation a mark belongs to.
#[derive(Debug, Clone)]
pub struct IngestMarkKey {
    pub user_id: UserId,
    pub scope_id: String,
    pub gateway_conversation_id: String,
}

/// The boundary a later turn resumes after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestMark {
    pub last_message_index: usize,
    pub boundary_tool_use_id: AiToolCallId,
}

#[derive(Debug, Clone)]
pub struct ToolResultIngestMarkRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ToolResultIngestMarkRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.pool(),
            write_pool: db.write_pool(),
        }
    }

    pub async fn find_mark(&self, key: &IngestMarkKey) -> McpDomainResult<Option<IngestMark>> {
        let row = sqlx::query!(
            r#"
            SELECT last_message_index, boundary_tool_use_id
            FROM tool_result_ingest_marks
            WHERE user_id = $1 AND scope_id = $2 AND gateway_conversation_id = $3
            "#,
            key.user_id.as_str(),
            key.scope_id,
            key.gateway_conversation_id
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.and_then(|row| {
            Some(IngestMark {
                last_message_index: usize::try_from(row.last_message_index).ok()?,
                boundary_tool_use_id: AiToolCallId::new(row.boundary_tool_use_id),
            })
        }))
    }

    pub async fn set_mark(&self, key: &IngestMarkKey, mark: &IngestMark) -> McpDomainResult<()> {
        let index = i32::try_from(mark.last_message_index).unwrap_or(i32::MAX);
        sqlx::query!(
            r#"
            INSERT INTO tool_result_ingest_marks (
                user_id, scope_id, gateway_conversation_id,
                last_message_index, boundary_tool_use_id, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
            ON CONFLICT (user_id, scope_id, gateway_conversation_id) DO UPDATE
            SET last_message_index = EXCLUDED.last_message_index,
                boundary_tool_use_id = EXCLUDED.boundary_tool_use_id,
                updated_at = EXCLUDED.updated_at
            "#,
            key.user_id.as_str(),
            key.scope_id,
            key.gateway_conversation_id,
            index,
            mark.boundary_tool_use_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    /// The call ids among `call_ids` whose gateway ingest has already settled:
    /// the artifact exists, the gateway was the last vantage point to see it,
    /// its session and trace are stamped and its execution is exactly
    /// correlated. Re-ingesting one of these would change no row.
    pub async fn list_settled_gateway_call_ids(
        &self,
        call_ids: &[String],
    ) -> McpDomainResult<Vec<String>> {
        if call_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_scalar!(
            r#"
            SELECT a.ai_tool_call_id AS "ai_tool_call_id!"
            FROM mcp_artifacts a
            JOIN mcp_tool_executions e ON e.mcp_execution_id = a.mcp_execution_id
            WHERE a.ai_tool_call_id = ANY($1)
              AND e.ai_tool_call_id = a.ai_tool_call_id
              AND e.correlation = 'exact'
              AND a.session_id IS NOT NULL
              AND a.trace_id IS NOT NULL
              AND (
                  a.last_seen_source = 'gateway'
                  OR (a.last_seen_source IS NULL AND a.source = 'gateway')
              )
            "#,
            call_ids
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }
}
