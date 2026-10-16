use chrono::{DateTime, Utc};
use std::collections::HashMap;
use systemprompt_identifiers::AiQuotaBucketId;

use super::{IncrementParams, QuotaBucketDelta, QuotaBucketState};

pub(super) type BucketKey = (String, String, i32, DateTime<Utc>, i16);

pub(super) struct MergedBucket {
    pub(super) key: BucketKey,
    pub(super) delta: QuotaBucketDelta,
}

pub(super) struct BucketColumns {
    pub(super) ids: Vec<String>,
    pub(super) kinds: Vec<String>,
    pub(super) subjects: Vec<String>,
    pub(super) windows: Vec<i32>,
    pub(super) starts: Vec<DateTime<Utc>>,
    pub(super) shards: Vec<i16>,
    pub(super) requests: Vec<i64>,
    pub(super) inputs: Vec<i64>,
    pub(super) outputs: Vec<i64>,
    pub(super) costs: Vec<i64>,
}

impl BucketColumns {
    pub(super) fn from_merged(merged: &[MergedBucket]) -> Self {
        Self {
            ids: merged
                .iter()
                .map(|_| AiQuotaBucketId::generate().as_str().to_owned())
                .collect(),
            kinds: merged.iter().map(|m| m.key.0.clone()).collect(),
            subjects: merged.iter().map(|m| m.key.1.clone()).collect(),
            windows: merged.iter().map(|m| m.key.2).collect(),
            starts: merged.iter().map(|m| m.key.3).collect(),
            shards: merged.iter().map(|m| m.key.4).collect(),
            requests: merged.iter().map(|m| m.delta.requests).collect(),
            inputs: merged.iter().map(|m| m.delta.input_tokens).collect(),
            outputs: merged.iter().map(|m| m.delta.output_tokens).collect(),
            costs: merged.iter().map(|m| m.delta.cost_microdollars).collect(),
        }
    }
}

pub(super) struct WindowTotalRow {
    pub(super) subject_kind: String,
    pub(super) subject_id: String,
    pub(super) window_seconds: i32,
    pub(super) window_start: DateTime<Utc>,
    pub(super) shard: i16,
    pub(super) requests: i64,
    pub(super) input_tokens: i64,
    pub(super) output_tokens: i64,
    pub(super) cost_microdollars: i64,
}

impl WindowTotalRow {
    pub(super) fn into_entry(self) -> (BucketKey, QuotaBucketState) {
        let state = QuotaBucketState {
            requests: self.requests,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cost_microdollars: self.cost_microdollars,
        };
        let key = (
            self.subject_kind,
            self.subject_id,
            self.window_seconds,
            self.window_start,
            self.shard,
        );
        (key, state)
    }
}

fn bucket_key(p: &IncrementParams<'_>) -> BucketKey {
    (
        p.subject_kind.to_owned(),
        p.subject_id.to_owned(),
        p.window_seconds,
        p.window_start,
        p.shard,
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
pub(super) fn merge_by_bucket(params: &[IncrementParams<'_>]) -> Vec<MergedBucket> {
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

pub(super) fn states_in_input_order(
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
