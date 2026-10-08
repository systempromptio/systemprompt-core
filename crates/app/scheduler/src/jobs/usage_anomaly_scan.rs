//! Hourly usage-anomaly scan.
//!
//! At two minutes past each hour the job evaluates the previous, closed hour
//! once: each user active in it is compared with their trailing seven-day
//! hourly average. A user is flagged for `spend` when the hour's cost reaches
//! three times the baseline and at least [`SPEND_FLOOR_MICRODOLLARS`], and for
//! `request_rate` when the hour's requests reach three times the baseline and
//! at least [`REQUEST_FLOOR`]; a user with no history is held to the floors
//! alone. Evaluating a closed window once per run needs no state of its own.
//! Every flag increments `systemprompt_usage_anomaly_total{kind, subject_kind}`
//! and emits a `usage_anomaly` warning event, which a SIEM or log alert
//! consumes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Duration, DurationRound, Utc};
use systemprompt_ai::repository::{AiUsageAnomalyRepository, HourlyUsageProfile};
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

pub const SPEND_RATIO: f64 = 3.0;
pub const REQUEST_RATIO: f64 = 3.0;
pub const SPEND_FLOOR_MICRODOLLARS: i64 = 1_000_000;
pub const REQUEST_FLOOR: i64 = 50;

/// Which usage signal an anomaly was raised on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyKind {
    Spend,
    RequestRate,
}

impl AnomalyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spend => "spend",
            Self::RequestRate => "request_rate",
        }
    }
}

/// One flagged user in one scanned window.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageAnomaly {
    pub kind: AnomalyKind,
    pub profile: HourlyUsageProfile,
}

#[must_use]
pub fn evaluate(profile: &HourlyUsageProfile) -> Vec<AnomalyKind> {
    let mut kinds = Vec::new();
    let spend_threshold =
        (SPEND_RATIO * profile.baseline_cost_per_hour).max(SPEND_FLOOR_MICRODOLLARS as f64);
    if profile.observed_cost_microdollars as f64 >= spend_threshold {
        kinds.push(AnomalyKind::Spend);
    }
    let request_threshold =
        (REQUEST_RATIO * profile.baseline_requests_per_hour).max(REQUEST_FLOOR as f64);
    if profile.observed_requests as f64 >= request_threshold {
        kinds.push(AnomalyKind::RequestRate);
    }
    kinds
}

pub async fn scan_window(
    repository: &AiUsageAnomalyRepository,
    window_start: DateTime<Utc>,
) -> Result<Vec<UsageAnomaly>, systemprompt_traits::RepositoryError> {
    let window_end = window_start + Duration::hours(1);
    let profiles = repository.hourly_profile(window_start, window_end).await?;
    let mut anomalies = Vec::new();
    for profile in profiles {
        for kind in evaluate(&profile) {
            report(kind, &profile, window_start);
            anomalies.push(UsageAnomaly {
                kind,
                profile: profile.clone(),
            });
        }
    }
    Ok(anomalies)
}

fn report(kind: AnomalyKind, profile: &HourlyUsageProfile, window_start: DateTime<Utc>) {
    metrics::counter!(
        "systemprompt_usage_anomaly_total",
        "kind" => kind.as_str(),
        "subject_kind" => "user",
    )
    .increment(1);
    let (observed, baseline) = match kind {
        AnomalyKind::Spend => (
            profile.observed_cost_microdollars as f64,
            profile.baseline_cost_per_hour,
        ),
        AnomalyKind::RequestRate => (
            profile.observed_requests as f64,
            profile.baseline_requests_per_hour,
        ),
    };
    tracing::warn!(
        target: "usage_anomaly",
        user_id = %profile.user_id,
        kind = kind.as_str(),
        observed,
        baseline,
        ratio = if baseline > 0.0 { observed / baseline } else { f64::INFINITY },
        window_start = %window_start,
        "usage_anomaly"
    );
}

#[must_use]
pub fn previous_hour(now: DateTime<Utc>) -> DateTime<Utc> {
    now.duration_trunc(Duration::hours(1)).unwrap_or(now) - Duration::hours(1)
}

#[derive(Debug, Clone, Copy)]
pub struct UsageAnomalyScanJob;

#[async_trait]
impl Job for UsageAnomalyScanJob {
    fn name(&self) -> &'static str {
        "usage_anomaly_scan"
    }

    fn description(&self) -> &'static str {
        "Flags users whose previous-hour spend or request rate far exceeds their 7-day baseline"
    }

    fn schedule(&self) -> &'static str {
        "0 2 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let db_pool = std::sync::Arc::clone(ctx.get::<DbPool>()?);
        let repository = AiUsageAnomalyRepository::new(&db_pool);
        let anomalies = scan_window(&repository, previous_hour(Utc::now()))
            .await
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(Box::new(e)))?;
        Ok(JobResult::success()
            .with_stats(anomalies.len() as u64, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_provider_contracts::submit_job!(&UsageAnomalyScanJob);
