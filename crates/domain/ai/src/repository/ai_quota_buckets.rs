//! Repository for `ai_quota_buckets` accounting rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiQuotaBucketId;

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
    pub cost_microdollars: i64,
}

/// Bucket subjects are opaque strings, not typed user IDs: a subject may be a
/// user, an organization, or any dimension an extension registers.
#[derive(Debug, Clone, Copy)]
pub struct IncrementParams<'a> {
    pub subject_kind: &'a str,
    pub subject_id: &'a str,
    pub window_seconds: i32,
    pub window_start: DateTime<Utc>,
    pub delta: QuotaBucketDelta,
}

#[derive(Debug, Clone, Copy)]
pub struct QuotaBucketState {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_microdollars: i64,
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
        params: IncrementParams<'_>,
    ) -> Result<QuotaBucketState, RepositoryError> {
        let IncrementParams {
            subject_kind,
            subject_id,
            window_seconds,
            window_start,
            delta,
        } = params;
        let id = AiQuotaBucketId::generate();
        let row = sqlx::query!(
            r#"
            INSERT INTO ai_quota_buckets (
                id, subject_kind, subject_id, window_seconds, window_start,
                requests, input_tokens, output_tokens, cost_microdollars, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, CURRENT_TIMESTAMP)
            ON CONFLICT (subject_kind, subject_id, window_seconds, window_start) DO UPDATE
            SET requests = ai_quota_buckets.requests + EXCLUDED.requests,
                input_tokens = ai_quota_buckets.input_tokens + EXCLUDED.input_tokens,
                output_tokens = ai_quota_buckets.output_tokens + EXCLUDED.output_tokens,
                cost_microdollars = ai_quota_buckets.cost_microdollars + EXCLUDED.cost_microdollars,
                updated_at = CURRENT_TIMESTAMP
            RETURNING requests, input_tokens, output_tokens, cost_microdollars
            "#,
            id.as_str(),
            subject_kind,
            subject_id,
            window_seconds,
            window_start,
            delta.requests,
            delta.input_tokens,
            delta.output_tokens,
            delta.cost_microdollars,
        )
        .fetch_one(self.write_pool.as_ref())
        .await?;

        Ok(QuotaBucketState {
            requests: row.requests,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cost_microdollars: row.cost_microdollars,
        })
    }
}
