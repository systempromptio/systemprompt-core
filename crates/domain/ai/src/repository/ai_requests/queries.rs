use crate::error::RepositoryError;
use crate::models::{AiRequest, ProviderUsage, UserAiUsage};
use chrono::Utc;
use systemprompt_identifiers::{AiRequestId, SessionId, UserId};

use super::AiRequestRepository;

impl AiRequestRepository {
    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn get_by_id(&self, id: &AiRequestId) -> Result<Option<AiRequest>, RepositoryError> {
        sqlx::query_as!(
            AiRequest,
            r#"
            SELECT id, request_id, user_id, session_id, task_id, context_id, trace_id,
                   provider, model, temperature, top_p, max_tokens, tokens_used,
                   input_tokens, output_tokens, cost_microdollars, latency_ms, cache_hit,
                   cache_read_tokens, cache_creation_tokens, is_streaming, status,
                   error_message, created_at, updated_at, completed_at
            FROM ai_requests
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn get_provider_usage(
        &self,
        days: i32,
    ) -> Result<Vec<ProviderUsage>, RepositoryError> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        sqlx::query_as!(
            ProviderUsage,
            r#"
            SELECT
                provider,
                model,
                COUNT(*)::bigint as "request_count!",
                COALESCE(SUM(tokens_used), 0)::bigint as "total_tokens!",
                COALESCE(SUM(cost_microdollars), 0)::float8 / 1000000.0 as "total_cost!",
                AVG(latency_ms)::bigint as "avg_latency_ms"
            FROM ai_requests
            WHERE created_at > $1 AND status = 'completed'
            GROUP BY provider, model
            ORDER BY COUNT(*) DESC
            "#,
            cutoff
        )
        .fetch_all(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn get_user_usage(&self, user_id: &UserId) -> Result<UserAiUsage, RepositoryError> {
        sqlx::query_as!(
            UserAiUsage,
            r#"
            SELECT
                user_id as "user_id!: UserId",
                COUNT(*)::bigint as "request_count!",
                COALESCE(SUM(tokens_used), 0)::bigint as "total_tokens!",
                COALESCE(SUM(cost_microdollars), 0)::float8 / 1000000.0 as "total_cost!",
                AVG(tokens_used)::float8 as "avg_tokens_per_request"
            FROM ai_requests
            WHERE user_id = $1
            GROUP BY user_id
            "#,
            user_id.as_str()
        )
        .fetch_one(self.pool())
        .await
        .map_err(RepositoryError::from)
    }

    pub async fn get_session_usage(
        &self,
        session_id: &SessionId,
    ) -> Result<UserAiUsage, RepositoryError> {
        sqlx::query_as!(
            UserAiUsage,
            r#"
            SELECT
                user_id as "user_id!: UserId",
                COUNT(*)::bigint as "request_count!",
                COALESCE(SUM(tokens_used), 0)::bigint as "total_tokens!",
                COALESCE(SUM(cost_microdollars), 0)::float8 / 1000000.0 as "total_cost!",
                AVG(tokens_used)::float8 as "avg_tokens_per_request"
            FROM ai_requests
            WHERE session_id = $1
            GROUP BY user_id
            "#,
            session_id.as_str()
        )
        .fetch_one(self.pool())
        .await
        .map_err(RepositoryError::from)
    }
}
