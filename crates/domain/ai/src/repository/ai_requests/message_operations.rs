//! Message-row operations linked to AI requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use crate::models::{AiRequestMessage, AiRequestToolCall};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, McpExecutionId};
use uuid::Uuid;

use super::repository::AiRequestRepository;

#[derive(Debug)]
pub struct InsertToolCallParams<'a> {
    pub request_id: &'a AiRequestId,
    pub ai_tool_call_id: &'a AiToolCallId,
    pub tool_name: &'a str,
    pub tool_input: &'a str,
    pub sequence_number: i32,
}

impl AiRequestRepository {
    pub async fn insert_message(
        &self,
        request_id: &AiRequestId,
        role: &str,
        content: &str,
        sequence_number: i32,
    ) -> Result<AiRequestMessage, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestMessage,
            r#"
            INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id, request_id as "request_id!: AiRequestId", role, content, sequence_number, name, tool_call_id as "tool_call_id: AiToolCallId", created_at, updated_at
            "#,
            id,
            request_id_str,
            role,
            content,
            sequence_number
        )
        .fetch_one(self.write_pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn list_messages(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<AiRequestMessage>, RepositoryError> {
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestMessage,
            r#"
            SELECT id, request_id as "request_id!: AiRequestId", role, content, sequence_number, name, tool_call_id as "tool_call_id: AiToolCallId", created_at, updated_at
            FROM ai_request_messages
            WHERE request_id = $1
            ORDER BY sequence_number ASC
            "#,
            request_id_str
        )
        .fetch_all(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn insert_tool_call(
        &self,
        params: InsertToolCallParams<'_>,
    ) -> Result<AiRequestToolCall, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        let request_id_str = params.request_id.as_str();

        sqlx::query_as!(
            AiRequestToolCall,
            r#"
            INSERT INTO ai_request_tool_calls (id, request_id, ai_tool_call_id, tool_name, tool_input, sequence_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id, request_id as "request_id!: AiRequestId", tool_name, tool_input, mcp_execution_id as "mcp_execution_id: McpExecutionId", sequence_number, ai_tool_call_id as "ai_tool_call_id: AiToolCallId", created_at, updated_at
            "#,
            id,
            request_id_str,
            params.ai_tool_call_id.as_str(),
            params.tool_name,
            params.tool_input,
            params.sequence_number
        )
        .fetch_one(self.write_pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn list_tool_calls(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<AiRequestToolCall>, RepositoryError> {
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestToolCall,
            r#"
            SELECT id, request_id as "request_id!: AiRequestId", tool_name, tool_input, mcp_execution_id as "mcp_execution_id: McpExecutionId", sequence_number, ai_tool_call_id as "ai_tool_call_id: AiToolCallId", created_at, updated_at
            FROM ai_request_tool_calls
            WHERE request_id = $1
            ORDER BY sequence_number ASC
            "#,
            request_id_str
        )
        .fetch_all(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    /// The intent row a client `tool_use_id` names: the model's call whose
    /// result is being reported.
    pub async fn find_tool_call_by_ai_id(
        &self,
        ai_tool_call_id: &AiToolCallId,
    ) -> Result<Option<AiRequestToolCall>, RepositoryError> {
        sqlx::query_as!(
            AiRequestToolCall,
            r#"
            SELECT id, request_id as "request_id!: AiRequestId", tool_name, tool_input, mcp_execution_id as "mcp_execution_id: McpExecutionId", sequence_number, ai_tool_call_id as "ai_tool_call_id: AiToolCallId", created_at, updated_at
            FROM ai_request_tool_calls
            WHERE ai_tool_call_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            ai_tool_call_id.as_str()
        )
        .fetch_optional(self.pool())
        .await
        .map_err(RepositoryError::from)
    }
}
