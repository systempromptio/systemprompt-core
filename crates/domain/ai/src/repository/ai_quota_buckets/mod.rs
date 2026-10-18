//! Repository for `ai_quota_buckets` accounting rows.
//!
//! A window's usage may be spread over several shard rows that share the
//! `(subject_kind, subject_id, window_seconds, window_start)` key and differ
//! by `shard`. An increment writes and locks only its own shard row; the state
//! it returns is the window total: the post-update own row plus the other
//! shards as of the statement's snapshot, read without taking a lock. Shards
//! exist so that a subject shared by every request (an organization, a
//! project) is not one row that serialises every admission in the
//! installation.
//!
//! The price is a looser admission bound. The other shards are read from the
//! statement snapshot, so a reservation landing on another shard concurrently
//! is not counted. With `N` shards the window can be over-admitted by at most
//! one reservation (one request plus its estimate) per admission running
//! concurrently on another shard, in addition to the estimate-versus-actual
//! overshoot every reservation already carries. A single shard (`shard = 0`)
//! keeps the exact row-locked behaviour.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod usage;

pub use usage::{QuotaSubjectUsage, QuotaUsageRepository, QuotaWindowKey};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::RepositoryError;

use merge::{
    BucketColumns, BucketKey, MergedBucket, WindowTotalRow, merge_by_bucket, states_in_input_order,
};

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

impl QuotaBucketDelta {
    pub const ZERO: Self = Self {
        requests: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost_microdollars: 0,
    };
}

/// Bucket subjects are opaque strings, not typed user IDs: a subject may be a
/// user, an organization, or any dimension an extension registers.
///
/// `shard` selects the row within the window; settlement must reuse the shard
/// the reservation landed on.
#[derive(Debug, Clone, Copy)]
pub struct IncrementParams<'a> {
    pub subject_kind: &'a str,
    pub subject_id: &'a str,
    pub window_seconds: i32,
    pub window_start: DateTime<Utc>,
    pub shard: i16,
    pub delta: QuotaBucketDelta,
}

/// A window's usage summed over every shard.
#[derive(Debug, Clone, Copy)]
pub struct QuotaBucketState {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_microdollars: i64,
}

impl AiQuotaBucketRepository {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }

    pub async fn increment(
        &self,
        params: IncrementParams<'_>,
    ) -> Result<QuotaBucketState, RepositoryError> {
        self.increment_many(&[params])
            .await?
            .pop()
            .ok_or(RepositoryError::NotFound {
                entity: "ai_quota_bucket",
                key: None,
            })
    }

    pub async fn increment_many(
        &self,
        params: &[IncrementParams<'_>],
    ) -> Result<Vec<QuotaBucketState>, RepositoryError> {
        if params.is_empty() {
            return Ok(Vec::new());
        }
        let merged = merge_by_bucket(params);
        let finals = self.upsert_merged(&merged).await?;
        Ok(states_in_input_order(params, &finals))
    }

    async fn upsert_merged(
        &self,
        merged: &[MergedBucket],
    ) -> Result<HashMap<BucketKey, QuotaBucketState>, RepositoryError> {
        let columns = BucketColumns::from_merged(merged);
        let rows = sqlx::query_as!(
            WindowTotalRow,
            r#"
            WITH upserted AS (
                INSERT INTO ai_quota_buckets (
                    id, subject_kind, subject_id, window_seconds, window_start, shard,
                    requests, input_tokens, output_tokens, cost_microdollars, updated_at
                )
                SELECT t.id, t.subject_kind, t.subject_id, t.window_seconds, t.window_start,
                       t.shard, t.requests, t.input_tokens, t.output_tokens,
                       t.cost_microdollars, CURRENT_TIMESTAMP
                FROM UNNEST(
                    $1::text[], $2::text[], $3::text[], $4::int4[], $5::timestamptz[],
                    $6::int2[], $7::int8[], $8::int8[], $9::int8[], $10::int8[]
                ) AS t(id, subject_kind, subject_id, window_seconds, window_start, shard,
                       requests, input_tokens, output_tokens, cost_microdollars)
                ORDER BY t.subject_kind, t.subject_id, t.window_seconds, t.window_start, t.shard
                ON CONFLICT (subject_kind, subject_id, window_seconds, window_start, shard)
                DO UPDATE
                SET requests = ai_quota_buckets.requests + EXCLUDED.requests,
                    input_tokens = ai_quota_buckets.input_tokens + EXCLUDED.input_tokens,
                    output_tokens = ai_quota_buckets.output_tokens + EXCLUDED.output_tokens,
                    cost_microdollars =
                        ai_quota_buckets.cost_microdollars + EXCLUDED.cost_microdollars,
                    updated_at = CURRENT_TIMESTAMP
                RETURNING subject_kind, subject_id, window_seconds, window_start, shard,
                          requests, input_tokens, output_tokens, cost_microdollars
            )
            SELECT u.subject_kind AS "subject_kind!",
                   u.subject_id AS "subject_id!",
                   u.window_seconds AS "window_seconds!",
                   u.window_start AS "window_start!",
                   u.shard AS "shard!",
                   (u.requests + COALESCE(o.requests, 0))::int8 AS "requests!",
                   (u.input_tokens + COALESCE(o.input_tokens, 0))::int8 AS "input_tokens!",
                   (u.output_tokens + COALESCE(o.output_tokens, 0))::int8 AS "output_tokens!",
                   (u.cost_microdollars + COALESCE(o.cost_microdollars, 0))::int8
                       AS "cost_microdollars!"
            FROM upserted u
            LEFT JOIN LATERAL (
                SELECT SUM(b.requests) AS requests,
                       SUM(b.input_tokens) AS input_tokens,
                       SUM(b.output_tokens) AS output_tokens,
                       SUM(b.cost_microdollars) AS cost_microdollars
                FROM ai_quota_buckets b
                WHERE b.subject_kind = u.subject_kind
                  AND b.subject_id = u.subject_id
                  AND b.window_seconds = u.window_seconds
                  AND b.window_start = u.window_start
                  AND b.shard <> u.shard
            ) o ON TRUE
            "#,
            &columns.ids,
            &columns.kinds,
            &columns.subjects,
            &columns.windows,
            &columns.starts,
            &columns.shards,
            &columns.requests,
            &columns.inputs,
            &columns.outputs,
            &columns.costs,
        )
        .fetch_all(self.write_pool.as_ref())
        .await?;
        Ok(rows.into_iter().map(WindowTotalRow::into_entry).collect())
    }
}
