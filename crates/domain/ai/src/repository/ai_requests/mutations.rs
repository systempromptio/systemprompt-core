use crate::error::RepositoryError;
use crate::models::{AiRequest, AiRequestRecord, RequestStatus};
use systemprompt_identifiers::{
    AiRequestId, ContextId, GatewayConversationId, ProviderRequestId, SessionId, TaskId, TraceId,
    UserId,
};

use super::AiRequestRepository;

#[derive(Debug)]
pub struct UpdateCompletionParams {
    pub id: AiRequestId,
    pub tokens_used: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub cache_hit: bool,
    pub cache_read_tokens: i32,
    pub cache_creation_tokens: i32,
}

impl AiRequestRepository {
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
                cost_microdollars = $4, latency_ms = $5,
                cache_hit = $6, cache_read_tokens = $7, cache_creation_tokens = $8,
                status = $9,
                completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
            WHERE id = $10
            RETURNING id as "id!: AiRequestId",
                      request_id as "request_id!: AiRequestId",
                      user_id as "user_id!: UserId",
                      session_id as "session_id: SessionId",
                      task_id as "task_id: TaskId",
                      context_id as "context_id: ContextId",
                      gateway_conversation_id as "gateway_conversation_id: GatewayConversationId",
                      provider_request_id as "provider_request_id: ProviderRequestId",
                      trace_id as "trace_id: TraceId",
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
            params.cache_hit,
            params.cache_read_tokens,
            params.cache_creation_tokens,
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
                latency_ms = GREATEST(
                    0,
                    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) * 1000
                )::int,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING id as "id!: AiRequestId",
                      request_id as "request_id!: AiRequestId",
                      user_id as "user_id!: UserId",
                      session_id as "session_id: SessionId",
                      task_id as "task_id: TaskId",
                      context_id as "context_id: ContextId",
                      gateway_conversation_id as "gateway_conversation_id: GatewayConversationId",
                      provider_request_id as "provider_request_id: ProviderRequestId",
                      trace_id as "trace_id: TraceId",
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
    pub async fn update_model(&self, id: &AiRequestId, model: &str) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"UPDATE ai_requests SET model = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2"#,
            model,
            id.as_str()
        )
        .execute(self.write_pool())
        .await?;
        Ok(())
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn update_system_prompt_override(
        &self,
        id: &AiRequestId,
        descriptor: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"UPDATE ai_requests SET system_prompt_override = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2"#,
            descriptor,
            id.as_str()
        )
        .execute(self.write_pool())
        .await?;
        Ok(())
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn insert(&self, record: &AiRequestRecord) -> Result<AiRequestId, RepositoryError> {
        self.insert_with_id(&AiRequestId::generate(), record).await
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn insert_with_id(
        &self,
        id: &AiRequestId,
        record: &AiRequestRecord,
    ) -> Result<AiRequestId, RepositoryError> {
        use systemprompt_identifiers::{
            ContextId, GatewayConversationId, McpExecutionId, ProviderRequestId, SessionId, TaskId,
            TraceId,
        };

        let status = record.status.as_str();

        let use_completed_at = matches!(
            record.status,
            RequestStatus::Completed | RequestStatus::Failed
        );

        let (actor_kind, actor_id) = record.actor.audit_columns();

        sqlx::query!(
            r#"
            INSERT INTO ai_requests (
                id, request_id, user_id, session_id, task_id, context_id,
                gateway_conversation_id, provider_request_id, trace_id,
                mcp_execution_id, provider, model, max_tokens, tokens_used, input_tokens, output_tokens,
                cache_hit, cache_read_tokens, cache_creation_tokens, is_streaming,
                cost_microdollars, latency_ms, status, error_message,
                actor_kind, actor_id, requested_model,
                created_at, updated_at, completed_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                $25, $26, $27,
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
                CASE WHEN $28 THEN CURRENT_TIMESTAMP ELSE NULL END
            )
            ON CONFLICT (id) DO NOTHING
            "#,
            id.as_str(),
            record.request_id.as_str(),
            record.user_id.as_str(),
            record.session_id.as_ref().map(SessionId::as_str),
            record.task_id.as_ref().map(TaskId::as_str),
            record.context_id.as_ref().map(ContextId::as_str),
            record.gateway_conversation_id.as_ref().map(GatewayConversationId::as_str),
            record.provider_request_id.as_ref().map(ProviderRequestId::as_str),
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
            actor_kind,
            actor_id,
            record.requested_model.as_deref(),
            use_completed_at
        )
        .execute(self.write_pool())
        .await?;
        Ok(id.clone())
    }
}
