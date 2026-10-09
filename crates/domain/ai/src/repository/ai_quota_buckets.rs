//! Repository for `ai_quota_buckets` accounting rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiQuotaBucketId;
use systemprompt_traits::RepositoryError;

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
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
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
        let ids: Vec<String> = merged
            .iter()
            .map(|_| AiQuotaBucketId::generate().as_str().to_owned())
            .collect();
        let kinds: Vec<String> = merged.iter().map(|m| m.key.0.clone()).collect();
        let subjects: Vec<String> = merged.iter().map(|m| m.key.1.clone()).collect();
        let windows: Vec<i32> = merged.iter().map(|m| m.key.2).collect();
        let starts: Vec<DateTime<Utc>> = merged.iter().map(|m| m.key.3).collect();
        let requests: Vec<i64> = merged.iter().map(|m| m.delta.requests).collect();
        let inputs: Vec<i64> = merged.iter().map(|m| m.delta.input_tokens).collect();
        let outputs: Vec<i64> = merged.iter().map(|m| m.delta.output_tokens).collect();
        let costs: Vec<i64> = merged.iter().map(|m| m.delta.cost_microdollars).collect();
        let rows = sqlx::query!(
            r#"
            INSERT INTO ai_quota_buckets (
                id, subject_kind, subject_id, window_seconds, window_start,
                requests, input_tokens, output_tokens, cost_microdollars, updated_at
            )
            SELECT t.id, t.subject_kind, t.subject_id, t.window_seconds, t.window_start,
                   t.requests, t.input_tokens, t.output_tokens, t.cost_microdollars,
                   CURRENT_TIMESTAMP
            FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::int4[], $5::timestamptz[],
                $6::int8[], $7::int8[], $8::int8[], $9::int8[]
            ) AS t(id, subject_kind, subject_id, window_seconds, window_start,
                   requests, input_tokens, output_tokens, cost_microdollars)
            ORDER BY t.subject_kind, t.subject_id, t.window_seconds, t.window_start
            ON CONFLICT (subject_kind, subject_id, window_seconds, window_start) DO UPDATE
            SET requests = ai_quota_buckets.requests + EXCLUDED.requests,
                input_tokens = ai_quota_buckets.input_tokens + EXCLUDED.input_tokens,
                output_tokens = ai_quota_buckets.output_tokens + EXCLUDED.output_tokens,
                cost_microdollars = ai_quota_buckets.cost_microdollars + EXCLUDED.cost_microdollars,
                updated_at = CURRENT_TIMESTAMP
            RETURNING subject_kind, subject_id, window_seconds, window_start,
                      requests, input_tokens, output_tokens, cost_microdollars
            "#,
            &ids,
            &kinds,
            &subjects,
            &windows,
            &starts,
            &requests,
            &inputs,
            &outputs,
            &costs,
        )
        .fetch_all(self.write_pool.as_ref())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let state = QuotaBucketState {
                    requests: row.requests,
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    cost_microdollars: row.cost_microdollars,
                };
                let key = (row.subject_kind, row.subject_id, row.window_seconds, row.window_start);
                (key, state)
            })
            .collect())
    }
}

type BucketKey = (String, String, i32, DateTime<Utc>);

struct MergedBucket {
    key: BucketKey,
    delta: QuotaBucketDelta,
}

fn bucket_key(p: &IncrementParams<'_>) -> BucketKey {
    (
        p.subject_kind.to_owned(),
        p.subject_id.to_owned(),
        p.window_seconds,
        p.window_start,
    )
}

const fn add_delta(a: QuotaBucketDelta, b: QuotaBucketDelta) -> QuotaBucketDelta {
    QuotaBucketDelta {
        requests: a.requests + b.requests,
        input_tokens: a.input_tokens + b.input_tokens,
        output_tokens: a.output_tokens + b.output_tokens,
        cost_microdollars: a.cost_microdollars + b.cost_microdollars,
    }
}

// Why: Postgres refuses an INSERT ... ON CONFLICT DO UPDATE that touches the
// same row twice, so windows that share a bucket are summed into one row.
fn merge_by_bucket(params: &[IncrementParams<'_>]) -> Vec<MergedBucket> {
    let mut merged: Vec<MergedBucket> = Vec::with_capacity(params.len());
    for p in params {
        let key = bucket_key(p);
        if let Some(existing) = merged.iter_mut().find(|m| m.key == key) {
            existing.delta = add_delta(existing.delta, p.delta);
        } else {
            merged.push(MergedBucket {
                key,
                delta: p.delta,
            });
        }
    }
    merged
}

fn states_in_input_order(
    params: &[IncrementParams<'_>],
    finals: &HashMap<BucketKey, QuotaBucketState>,
) -> Vec<QuotaBucketState> {
    params
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let key = bucket_key(p);
            let later = params[index + 1..]
                .iter()
                .filter(|q| bucket_key(q) == key)
                .fold(QuotaBucketDelta::ZERO, |acc, q| add_delta(acc, q.delta));
            let total = finals.get(&key).copied().unwrap_or(QuotaBucketState {
                requests: 0,
                input_tokens: 0,
                output_tokens: 0,
                cost_microdollars: 0,
            });
            QuotaBucketState {
                requests: total.requests - later.requests,
                input_tokens: total.input_tokens - later.input_tokens,
                output_tokens: total.output_tokens - later.output_tokens,
                cost_microdollars: total.cost_microdollars - later.cost_microdollars,
            }
        })
        .collect()
}
