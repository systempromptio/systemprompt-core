//! Message-row operations linked to AI requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::rows::AiRequestToolCallRow;
use crate::models::{AiRequestMessage, AiRequestToolCall};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, McpExecutionId, McpToolName};
use systemprompt_traits::RepositoryError;
use uuid::Uuid;

use super::repository::AiRequestRepository;

#[derive(Debug, Clone, Copy)]
pub struct RequestMessageRow<'a> {
    pub role: &'a str,
    pub content: &'a str,
    pub sequence_number: i32,
}

#[derive(Debug)]
pub struct InsertToolCallParams<'a> {
    pub request_id: &'a AiRequestId,
    pub ai_tool_call_id: &'a AiToolCallId,
    pub tool_name: &'a McpToolName,
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

    pub async fn insert_messages(
        &self,
        request_id: &AiRequestId,
        messages: &[RequestMessageRow<'_>],
    ) -> Result<u64, RepositoryError> {
        insert_messages_with(self.write_pool(), request_id, messages).await
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
            AiRequestToolCallRow,
            r#"
            INSERT INTO ai_request_tool_calls (id, request_id, ai_tool_call_id, tool_name, tool_input, sequence_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id, request_id as "request_id!: AiRequestId", tool_name, tool_input, mcp_execution_id as "mcp_execution_id: McpExecutionId", sequence_number, ai_tool_call_id as "ai_tool_call_id: AiToolCallId", created_at, updated_at
            "#,
            id,
            request_id_str,
            params.ai_tool_call_id.as_str(),
            params.tool_name.as_str(),
            params.tool_input,
            params.sequence_number
        )
        .fetch_one(self.write_pool())
        .await
        .map(AiRequestToolCall::from)
        .map_err(RepositoryError::from)
    }

    pub async fn list_tool_calls(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<AiRequestToolCall>, RepositoryError> {
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestToolCallRow,
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
        .map(|rows| rows.into_iter().map(AiRequestToolCall::from).collect())
        .map_err(RepositoryError::from)
    }

    pub async fn find_tool_call_by_ai_id(
        &self,
        ai_tool_call_id: &AiToolCallId,
    ) -> Result<Option<AiRequestToolCall>, RepositoryError> {
        sqlx::query_as!(
            AiRequestToolCallRow,
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
        .map(|row| row.map(AiRequestToolCall::from))
        .map_err(RepositoryError::from)
    }
}

pub(crate) async fn insert_messages_with<'e, E>(
    executor: E,
    request_id: &AiRequestId,
    messages: &[RequestMessageRow<'_>],
) -> Result<u64, RepositoryError>
where
    E: sqlx::PgExecutor<'e>,
{
    if messages.is_empty() {
        return Ok(0);
    }
    let mut ids = Vec::with_capacity(messages.len());
    let mut roles = Vec::with_capacity(messages.len());
    let mut contents = Vec::with_capacity(messages.len());
    let mut sequences = Vec::with_capacity(messages.len());
    for message in messages {
        ids.push(Uuid::new_v4().to_string());
        roles.push(message.role.to_owned());
        contents.push(message.content.to_owned());
        sequences.push(message.sequence_number);
    }
    let result = sqlx::query!(
            r#"
            INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number, created_at, updated_at)
            SELECT t.id, $1, t.role, t.content, t.sequence_number, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            FROM UNNEST($2::text[], $3::text[], $4::text[], $5::int4[]) AS t(id, role, content, sequence_number)
            "#,
            request_id.as_str(),
            &ids,
            &roles,
            &contents,
            &sequences
        )
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
