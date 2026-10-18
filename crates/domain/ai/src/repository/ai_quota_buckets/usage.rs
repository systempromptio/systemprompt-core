//! [`QuotaUsageRepository`]: read-side window usage summed over every shard
//! row, so a consumer never sums `ai_quota_buckets` itself and miscounts a
//! sharded window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_traits::RepositoryError;

use super::QuotaBucketState;

/// One quota window: its subject kind, bucket length and start, as a gateway
/// policy's window span resolves them.
#[derive(Debug, Clone, Copy)]
pub struct QuotaWindowKey<'a> {
    pub subject_kind: &'a str,
    pub window_seconds: i32,
    pub window_start: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct QuotaSubjectUsage {
    pub subject_id: String,
    pub usage: QuotaBucketState,
    pub updated_at: DateTime<Utc>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct QuotaUsageRepository {
    pool: Arc<PgPool>,
}

impl QuotaUsageRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }

    pub async fn usage(
        &self,
        window: QuotaWindowKey<'_>,
        subject_id: &str,
    ) -> Result<QuotaBucketState, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(requests), 0)::int8 AS "requests!",
                   COALESCE(SUM(input_tokens), 0)::int8 AS "input_tokens!",
                   COALESCE(SUM(output_tokens), 0)::int8 AS "output_tokens!",
                   COALESCE(SUM(cost_microdollars), 0)::int8 AS "cost_microdollars!"
            FROM ai_quota_buckets
            WHERE subject_kind = $1 AND subject_id = $2
              AND window_seconds = $3 AND window_start = $4
            "#,
            window.subject_kind,
            subject_id,
            window.window_seconds,
            window.window_start
        )
        .fetch_one(self.pool.as_ref())
        .await?;
        Ok(QuotaBucketState {
            requests: row.requests,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cost_microdollars: row.cost_microdollars,
        })
    }

    pub async fn window_usage(
        &self,
        window: QuotaWindowKey<'_>,
        limit: i64,
    ) -> Result<Vec<QuotaSubjectUsage>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT subject_id AS "subject_id!",
                   SUM(requests)::int8 AS "requests!",
                   SUM(input_tokens)::int8 AS "input_tokens!",
                   SUM(output_tokens)::int8 AS "output_tokens!",
                   SUM(cost_microdollars)::int8 AS "cost_microdollars!",
                   MAX(updated_at) AS "updated_at!"
            FROM ai_quota_buckets
            WHERE subject_kind = $1 AND window_seconds = $2 AND window_start = $3
            GROUP BY subject_id
            ORDER BY 5 DESC, 2 DESC, 1 ASC
            LIMIT $4
            "#,
            window.subject_kind,
            window.window_seconds,
            window.window_start,
            limit.max(0)
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| QuotaSubjectUsage {
                subject_id: row.subject_id,
                usage: QuotaBucketState {
                    requests: row.requests,
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    cost_microdollars: row.cost_microdollars,
                },
                updated_at: row.updated_at,
            })
            .collect())
    }
}
