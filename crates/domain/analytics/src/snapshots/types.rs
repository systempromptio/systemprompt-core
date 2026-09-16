//! Snapshot values explicitly distinguish unavailable identity and suppressed
//! coverage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{AnalyticsSnapshotJobId, AnalyticsWorkerId, ManagedResourceId};

#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(default)]
/// Additive counts and measured denominators for the selected contribution
/// range.
pub struct SnapshotMetrics {
    pub invocations: i64,
    pub verified_invocations: i64,
    pub requests: i64,
    pub failed_requests: i64,
    pub priced_requests: i64,
    pub latency_measured_requests: i64,
    pub token_measured_requests: i64,
    pub input_tokens: i128,
    pub output_tokens: i128,
    pub assessed_conversations: i64,
    pub assessment_conversations: i64,
    pub failed_assessments: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
/// A retained aggregate result with explicit identity and suppression
/// availability.
pub struct FeedbackSnapshot {
    pub resource_id: Option<ManagedResourceId>,
    pub generation: i64,
    pub fact_generation: i64,
    pub from_day: NaiveDate,
    pub to_day: NaiveDate,
    pub generated_at: DateTime<Utc>,
    pub metrics: SnapshotMetrics,
    pub spend_by_currency: BTreeMap<String, i128>,
    pub distinct_users: Option<i64>,
    pub distinct_sessions: Option<i64>,
    pub histogram: super::LatencyHistogram,
    pub related_spend_non_additive: bool,
    pub suppressed_days: i64,
    pub historical_identity_available: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
/// Durable generation watermarks and outstanding producer, fact, and range
/// work.
pub struct SnapshotHealth {
    pub generation: i64,
    pub fact_generation: i64,
    pub generated_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub pending_changes: i64,
    pub pending_producer_changes: i64,
    pub facts_generation: i64,
    pub pending_jobs: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
/// An idempotent bounded request for a UTC-day aggregate range.
pub struct SnapshotRangeRequest {
    pub operation_id: AnalyticsSnapshotJobId,
    pub resource_id: Option<ManagedResourceId>,
    pub from_day: NaiveDate,
    pub to_day: NaiveDate,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
/// A durable custom-range operation and its current result or diagnostic.
pub struct SnapshotRangeJob {
    pub operation_id: AnalyticsSnapshotJobId,
    pub state: SnapshotJobState,
    pub result: Option<FeedbackSnapshot>,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone)]
/// Worker identity and fencing epoch for a leased custom-range operation.
pub struct SnapshotJobLease {
    pub operation_id: AnalyticsSnapshotJobId,
    pub worker_id: AnalyticsWorkerId,
    pub epoch: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Lifecycle of a custom-range operation; `failed` is terminal and carries the
/// assembly diagnostic.
pub enum SnapshotJobState {
    Pending,
    Leased,
    Ready,
    Failed,
}
impl SnapshotJobState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Leased => "leased",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
    pub fn parse(state: &str) -> crate::Result<Self> {
        Ok(match state {
            "pending" => Self::Pending,
            "leased" => Self::Leased,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => return Err(super::invalid("Unknown range job state")),
        })
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
/// One organizational scope compacted behind drained evidence barriers.
pub struct RetentionOutcome {
    pub compacted_before: NaiveDate,
    pub removed_facts: u64,
    pub removed_daily: u64,
}

/// Aggregate results after every initialized organizational scope is compacted
/// atomically.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RetentionSummary {
    pub organizations: u64,
    pub removed_facts: u64,
    pub removed_daily: u64,
}
