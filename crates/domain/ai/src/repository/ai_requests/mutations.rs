use crate::error::RepositoryError;
use crate::models::{AiRequest, AiRequestRecord, RequestStatus};
use systemprompt_identifiers::{AiRequestId, SessionId, TraceId};

use super::{AiRequestRepository, CreateAiRequest};

#[derive(Debug)]
pub struct UpdateCompletionParams {
    pub id: AiRequestId,
    pub tokens_used: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
}

impl AiRequestRepository {
    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn create(&self, request: CreateAiRequest<'_>) -> Result<AiRequest, RepositoryError> {
        let id = AiRequestId::generate();
        let logical_request_id = AiRequestId::generate();

        sqlx::query_as!(
            AiRequest,
            r#"
            INSERT INTO ai_requests (
                id, request_id, user_id, session_id, trace_id, provider, model,
                temperature, max_tokens, status, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, CURRENT_TIMESTAMP)
            RETURNING id, request_id, user_id, session_id, task_id, context_id, trace_id,
                      provider, model, temperature, top_p, max_tokens, tokens_used,
                      input_tokens, output_tokens, cost_microdollars, latency_ms, cache_hit,
                      cache_read_tokens, cache_creation_tokens, is_streaming, status,
                      error_message, created_at, updated_at, completed_at
            "#,
            id.as_str(),
            logical_request_id.as_str(),
            request.user_id.as_str(),
            request.session_id.map(SessionId::as_str),
            request.trace_id.map(TraceId::as_str),
            request.provider,
            request.model,
            request.temperature,
            request.max_tokens,
            RequestStatus::Pending.as_str()
        )
        .fetch_one(self.write_pool())
        .await
        .map_err(RepositoryError::from)
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn update_completion(
        &self,
        params: UpdateCompletionParams,
    ) -> Result<AiRequest, RepositoryError> {
        sqlx::query_as!(
            AiRequest,
            r#"
            UPDATE ai_requests
            SET tokens_used = $1, input_tokens = $2, output_tokens = $3,
                cost_microdollars = $4, latency_ms = $5, status = $6,
                completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
            WHERE id = $7
            RETURNING id, request_id, user_id, session_id, task_id, context_id, trace_id,
                      provider, model, temperature, top_p, max_tokens, tokens_used,
                      input_tokens, output_tokens, cost_microdollars, latency_ms, cache_hit,
                      cache_read_tokens, cache_creation_tokens, is_streaming, status,
                      error_message, created_at, updated_at, completed_at
            "#,
            params.tokens_used,
            params.input_tokens,
            params.output_tokens,
            params.cost_microdollars,
            params.latency_ms,
            RequestStatus::Completed.as_str(),
            params.id.as_str()
        )
        .fetch_one(self.write_pool())
        .await
        .map_err(RepositoryError::from)
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn update_error(
        &self,
        id: &AiRequestId,
        error_message: &str,
    ) -> Result<AiRequest, RepositoryError> {
        sqlx::query_as!(
            AiRequest,
            r#"
            UPDATE ai_requests
            SET status = $1, error_message = $2, completed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING id, request_id, user_id, session_id, task_id, context_id, trace_id,
                      provider, model, temperature, top_p, max_tokens, tokens_used,
                      input_tokens, output_tokens, cost_microdollars, latency_ms, cache_hit,
                      cache_read_tokens, cache_creation_tokens, is_streaming, status,
                      error_message, created_at, updated_at, completed_at
            "#,
            RequestStatus::Failed.as_str(),
            error_message,
            id.as_str()
        )
        .fetch_one(self.write_pool())
        .await
        .map_err(RepositoryError::from)
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn insert(&self, record: &AiRequestRecord) -> Result<AiRequestId, RepositoryError> {
        use systemprompt_identifiers::{ContextId, McpExecutionId, SessionId, TaskId, TraceId};

        let id = AiRequestId::generate();
        let status = record.status.as_str();

        let use_completed_at = matches!(
            record.status,
            RequestStatus::Completed | RequestStatus::Failed
        );

        sqlx::query!(
            r#"
            INSERT INTO ai_requests (
                id, request_id, user_id, session_id, task_id, context_id, trace_id,
                mcp_execution_id, provider, model, max_tokens, tokens_used, input_tokens, output_tokens,
                cache_hit, cache_read_tokens, cache_creation_tokens, is_streaming,
                cost_microdollars, latency_ms, status, error_message,
                created_at, updated_at, completed_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20, $21, $22,
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
                CASE WHEN $23 THEN CURRENT_TIMESTAMP ELSE NULL END
            )
            "#,
            id.as_str(),
            record.request_id,
            record.user_id.as_str(),
            record.session_id.as_ref().map(SessionId::as_str),
            record.task_id.as_ref().map(TaskId::as_str),
            record.context_id.as_ref().map(ContextId::as_str),
            record.trace_id.as_ref().map(TraceId::as_str),
            record.mcp_execution_id.as_ref().map(McpExecutionId::as_str),
            record.provider,
            record.model,
            record.max_tokens,
            record.tokens.tokens_used,
            record.tokens.input_tokens,
            record.tokens.output_tokens,
            record.cache.hit,
            record.cache.read_tokens,
            record.cache.creation_tokens,
            record.is_streaming,
            record.cost_microdollars,
            record.latency_ms,
            status,
            record.error_message.as_deref(),
            use_completed_at
        )
        .execute(self.write_pool())
        .await?;
        Ok(id)
    }
}
