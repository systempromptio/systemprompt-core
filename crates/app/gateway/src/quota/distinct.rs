//! Collapsing a request's quota windows onto the bucket rows they charge.
//!
//! Two windows can resolve to one bucket row — a calendar day and an 86400s
//! window for the same subject share a key — and that row is charged once
//! per request, not once per window. Each window keeps a slot naming the
//! distinct row whose state its ceiling is checked against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::repository::IncrementParams;

fn same_bucket(a: &IncrementParams<'_>, b: &IncrementParams<'_>) -> bool {
    a.subject_kind == b.subject_kind
        && a.subject_id == b.subject_id
        && a.window_seconds == b.window_seconds
        && a.window_start == b.window_start
        && a.shard == b.shard
}

pub(super) fn distinct_increments<'a>(
    per_window: Vec<IncrementParams<'a>>,
) -> (Vec<IncrementParams<'a>>, Vec<usize>) {
    let mut distinct: Vec<IncrementParams<'a>> = Vec::with_capacity(per_window.len());
    let mut slots = Vec::with_capacity(per_window.len());
    for params in per_window {
        let slot = distinct
            .iter()
            .position(|seen| same_bucket(seen, &params))
            .unwrap_or_else(|| {
                distinct.push(params);
                distinct.len() - 1
            });
        slots.push(slot);
    }
    (distinct, slots)
}
