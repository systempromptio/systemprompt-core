//! Resource metrics count each request and assessed conversation once within
//! a cohort. Related conversation spend remains non-additive across cohorts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_identifiers::{AiRequestId, ResourceInvocationId, SessionId, UserId};

#[derive(Debug, Clone)]
pub struct ResourceFact {
    pub invocation_id: ResourceInvocationId,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub invoked_at: DateTime<Utc>,
    pub request_id: Option<AiRequestId>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cost_microdollars: Option<i64>,
    pub latency_ms: Option<i64>,
    pub failed: bool,
    pub quality_score: Option<f64>,
    pub successful: Option<bool>,
    pub revision_verified: bool,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct ResourceMetrics {
    pub invocations: usize,
    pub users: usize,
    pub conversations: usize,
    pub requests: usize,
    pub measured_requests: usize,
    pub priced_requests: usize,
    pub failed_requests: usize,
    pub verified_invocations: usize,
    pub assessed_conversations: usize,
    pub successful_conversations: usize,
    pub input_tokens: i128,
    pub output_tokens: i128,
    pub cache_read_tokens: i128,
    pub cache_creation_tokens: i128,
    pub related_cost_microdollars: Option<i128>,
    pub average_tokens_per_measured_request: Option<f64>,
    pub average_latency_ms: Option<f64>,
    pub average_quality_score: Option<f64>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub fn aggregate<'a>(facts: impl IntoIterator<Item = &'a ResourceFact> + 'a) -> ResourceMetrics {
    let mut invocations = BTreeSet::new();
    let mut verified = BTreeSet::new();
    let mut users = BTreeSet::new();
    let mut sessions = BTreeSet::new();
    let mut requests = BTreeMap::new();
    let mut assessments = BTreeMap::new();
    let mut last_used = None;
    for fact in facts {
        invocations.insert((&fact.user_id, &fact.invocation_id));
        if fact.revision_verified {
            verified.insert((&fact.user_id, &fact.invocation_id));
        }
        users.insert(&fact.user_id);
        sessions.insert((&fact.user_id, &fact.session_id));
        last_used = Some(last_used.map_or(fact.invoked_at, |last: DateTime<Utc>| {
            last.max(fact.invoked_at)
        }));
        if fact.quality_score.is_some() || fact.successful.is_some() {
            assessments.insert(
                (&fact.user_id, &fact.session_id),
                (fact.quality_score, fact.successful),
            );
        }
        if let Some(request) = &fact.request_id {
            requests.entry((&fact.user_id, request)).or_insert(fact);
        }
    }
    let mut result = ResourceMetrics {
        invocations: invocations.len(),
        users: users.len(),
        conversations: sessions.len(),
        requests: requests.len(),
        verified_invocations: verified.len(),
        assessed_conversations: assessments.len(),
        last_used_at: last_used,
        ..ResourceMetrics::default()
    };
    aggregate_requests(&mut result, requests.into_values());
    result.average_quality_score = mean(
        &assessments
            .values()
            .filter_map(|(score, _)| *score)
            .collect::<Vec<_>>(),
    );
    result.successful_conversations = assessments
        .values()
        .filter(|(_, success)| *success == Some(true))
        .count();
    result
}

fn aggregate_requests<'a>(
    result: &mut ResourceMetrics,
    facts: impl IntoIterator<Item = &'a ResourceFact> + 'a,
) {
    let mut token_sum = 0i128;
    let mut latencies = Vec::new();
    let mut cost = 0i128;
    for fact in facts {
        result.failed_requests += usize::from(fact.failed);
        if let Some(amount) = fact.cost_microdollars.filter(|value| *value >= 0) {
            result.priced_requests += 1;
            cost += i128::from(amount);
        }
        if let (Some(input), Some(output)) = (
            fact.input_tokens.filter(|value| *value >= 0),
            fact.output_tokens.filter(|value| *value >= 0),
        ) {
            result.measured_requests += 1;
            let cache_read = i128::from(fact.cache_read_tokens.unwrap_or(0).max(0));
            let cache_write = i128::from(fact.cache_creation_tokens.unwrap_or(0).max(0));
            result.input_tokens += i128::from(input);
            result.output_tokens += i128::from(output);
            result.cache_read_tokens += cache_read;
            result.cache_creation_tokens += cache_write;
            token_sum += i128::from(input) + i128::from(output) + cache_read + cache_write;
        }
        if let Some(latency) = fact.latency_ms {
            latencies.push(latency as f64);
        }
    }
    result.related_cost_microdollars = (result.priced_requests > 0).then_some(cost);
    result.average_tokens_per_measured_request =
        (result.measured_requests > 0).then(|| token_sum as f64 / result.measured_requests as f64);
    result.average_latency_ms = mean(&latencies);
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
