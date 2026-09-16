//! Worker fencing, durable operation state and normalized projection records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AnalyticsChangeId, AnalyticsWorkerId, TaskId};
use systemprompt_models::feedback::analytics::{AnalyticsFactKey, NormalizedAnalyticsFact};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeReceipt {
    pub change_id: AnalyticsChangeId,
    pub state: FactChangeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactLease {
    pub change_id: AnalyticsChangeId,
    pub worker_id: AnalyticsWorkerId,
    pub epoch: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub change_id: AnalyticsChangeId,
    pub generation: i64,
    pub replaced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFact {
    pub key: AnalyticsFactKey,
    pub revision: i64,
    pub occurred_at: DateTime<Utc>,
    pub fact: Option<NormalizedAnalyticsFact>,
    pub generation: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FactsHealth {
    pub generation: i64,
    pub pending: i64,
    pub leased: i64,
    pub retries: i64,
    pub oldest_pending_at: Option<DateTime<Utc>>,
    pub last_applied_at: Option<DateTime<Utc>>,
    pub last_recorded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillProgress {
    pub job_id: TaskId,
    pub source: String,
    pub cursor: String,
    pub generation: i64,
    pub pages: i64,
    pub facts: i64,
    pub complete: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactsTotals {
    pub invocations: i64,
    pub verified_invocations: i64,
    pub requests: i64,
    pub failed_requests: i64,
    pub priced_requests: i64,
    pub latency_measured_requests: i64,
    pub token_measured_requests: i64,
    pub assessed_conversations: i64,
    pub assessment_conversations: i64,
    pub failed_assessments: i64,
    pub spend_by_currency: std::collections::BTreeMap<String, i128>,
    pub related_spend_non_additive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackfillPage {
    pub expected_generation: i64,
    pub next_cursor: String,
    pub complete: bool,
    pub changes: Vec<systemprompt_models::feedback::analytics::AnalyticsChange>,
}

/// How much delta work one consumer worker claims and for how long.
#[derive(Debug, Clone, Copy)]
pub struct DeltaClaim {
    pub limit: u32,
    pub lease_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaLease {
    pub consumer: String,
    pub worker_id: AnalyticsWorkerId,
    pub epoch: i64,
    pub after_generation: i64,
    pub through_generation: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDelta {
    pub generation: i64,
    pub key: AnalyticsFactKey,
    pub before: Option<NormalizedAnalyticsFact>,
    pub after: Option<NormalizedAnalyticsFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactChangeState {
    Pending,
    Leased,
    Applied,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDiagnostic {
    pub change_id: AnalyticsChangeId,
    pub state: FactChangeState,
    pub attempts: i32,
    pub lease_until: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}
