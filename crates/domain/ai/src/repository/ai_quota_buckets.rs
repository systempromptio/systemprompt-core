use crate::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiQuotaBucketId, TenantId, UserId};

#[must_use]
#[derive(Debug, Clone)]
pub struct AiQuotaBucketRepository {
    write_pool: Arc<PgPool>,
}

#[derive(Debug, Clone, Copy)]
pub struct QuotaBucketDelta {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct QuotaBucketState {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

impl AiQuotaBucketRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }

    pub async fn increment(
        &self,
        tenant_id: Option<&TenantId>,
        user_id: &UserId,
        window_seconds: i32,
        window_start: DateTime<Utc>,
        delta: QuotaBucketDelta,
    ) -> Result<QuotaBucketState, RepositoryError> {
        let id = AiQuotaBucketId::generate();
        let row = sqlx::query!(
            r#"
            INSERT INTO ai_quota_buckets (
                id, tenant_id, user_id, window_seconds, window_start,
                requests, input_tokens, output_tokens, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, user_id, window_seconds, window_start) DO UPDATE
            SET requests = ai_quota_buckets.requests + EXCLUDED.requests,
                input_tokens = ai_quota_buckets.input_tokens + EXCLUDED.input_tokens,
                output_tokens = ai_quota_buckets.output_tokens + EXCLUDED.output_tokens,
                updated_at = CURRENT_TIMESTAMP
            RETURNING requests, input_tokens, output_tokens
            "#,
            id.as_str(),
            tenant_id.map(TenantId::as_str),
            user_id.as_str(),
            window_seconds,
            window_start,
            delta.requests,
            delta.input_tokens,
            delta.output_tokens,
        )
        .fetch_one(self.write_pool.as_ref())
        .await?;

        Ok(QuotaBucketState {
            requests: row.requests,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
        })
    }
}
