//! The brain pipeline's Prometheus series.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::FactCounts;

pub(crate) fn event(outcome: &'static str) {
    metrics::counter!("brain_events_total", "outcome" => outcome).increment(1);
}

pub(crate) fn candidate(effect: &'static str) {
    metrics::counter!("brain_candidates_total", "effect" => effect).increment(1);
}

pub(crate) fn extraction(extractor: &'static str, candidates: usize) {
    metrics::counter!("brain_extractions_total", "extractor" => extractor).increment(1);
    metrics::counter!("brain_extracted_candidates_total", "extractor" => extractor)
        .increment(u64::try_from(candidates).unwrap_or(u64::MAX));
}

pub(crate) fn budget_exhausted() {
    metrics::counter!("brain_budget_exhausted_total").increment(1);
}

pub(crate) fn batch_seconds(seconds: f64) {
    metrics::histogram!("brain_batch_seconds").record(seconds);
}

pub(crate) fn store(counts: &FactCounts, open_disputes: i64, tokens_today: i64) {
    for (status, count) in &counts.by_status {
        metrics::gauge!("brain_facts", "status" => status.as_str()).set(*count as f64);
    }
    metrics::gauge!("brain_disputes_open").set(open_disputes as f64);
    metrics::gauge!("brain_extraction_tokens_today").set(tokens_today as f64);
}
