use crate::error::RepositoryError;
use crate::models::{AiRequestMessage, AiRequestToolCall};
use systemprompt_identifiers::AiRequestId;
use uuid::Uuid;

use super::repository::AiRequestRepository;

#[derive(Debug)]
pub struct InsertToolCallParams<'a> {
    pub request_id: &'a AiRequestId,
    pub ai_tool_call_id: &'a str,
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
            RETURNING id, request_id, role, content, sequence_number, name, tool_call_id, created_at, updated_at
            "#,
            id,
            request_id_str,
            role,
            content,
            sequence_number
        )
        .fetch_one(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn get_messages(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<AiRequestMessage>, RepositoryError> {
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestMessage,
            r#"
            SELECT id, request_id, role, content, sequence_number, name, tool_call_id, created_at, updated_at
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

    pub async fn get_max_sequence(&self, request_id: &AiRequestId) -> Result<i32, RepositoryError> {
        let request_id_str = request_id.as_str();

        let result = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence_number), 0) as "max!" FROM ai_request_messages WHERE request_id = $1"#,
            request_id_str
        )
        .fetch_one(self.pool())
        .await?;
        Ok(result)
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
            RETURNING id, request_id, tool_name, tool_input, mcp_execution_id, sequence_number, ai_tool_call_id, created_at, updated_at
            "#,
            id,
            request_id_str,
            params.ai_tool_call_id,
            params.tool_name,
            params.tool_input,
            params.sequence_number
        )
        .fetch_one(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn get_tool_calls(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<AiRequestToolCall>, RepositoryError> {
        let request_id_str = request_id.as_str();

        sqlx::query_as!(
            AiRequestToolCall,
            r#"
            SELECT id, request_id, tool_name, tool_input, mcp_execution_id, sequence_number, ai_tool_call_id, created_at, updated_at
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

    pub async fn add_response_message(
        &self,
        request_id: &AiRequestId,
        content: &str,
    ) -> Result<(), RepositoryError> {
        let max_seq = self.get_max_sequence(request_id).await?;
        let id = Uuid::new_v4().to_string();
        let request_id_str = request_id.as_str();

        let seq = max_seq + 1;
        sqlx::query!(
            r#"
            INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number, created_at)
            VALUES ($1, $2, 'assistant', $3, $4, CURRENT_TIMESTAMP)
            "#,
            id,
            request_id_str,
            content,
            seq
        )
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn link_tool_calls_to_recent_executions(
        &self,
        ai_tool_call_ids: &[String],
    ) -> Result<u64, RepositoryError> {
        if ai_tool_call_ids.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query!(
            r#"
            UPDATE ai_request_tool_calls tc
            SET mcp_execution_id = ex.mcp_execution_id
            FROM mcp_tool_executions ex
            WHERE tc.ai_tool_call_id = ex.ai_tool_call_id
              AND tc.ai_tool_call_id = ANY($1)
              AND tc.mcp_execution_id IS NULL
            "#,
            ai_tool_call_ids
        )
        .execute(self.pool())
        .await?;

        Ok(result.rows_affected())
    }
}
