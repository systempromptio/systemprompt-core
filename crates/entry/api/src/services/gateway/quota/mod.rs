//! Gateway quota-window alignment and accounting.
//!
//! Request counts are incremented at admission; tokens and cost are recorded
//! after completion. In-flight usage can exceed token and cost ceilings.
//! Subject-resolution faults follow the configured `QuotaFaultMode`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod subject;

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use systemprompt_ai::repository::{
    AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta, QuotaBucketState,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::QuotaFaultMode;

use self::subject::{SubjectResolution, resolve_subject};
use super::policy::QuotaWindow;

/// The outcome of the post-response accounting write.
///
/// `Faulted` means spend for this request was not counted against any ceiling.
#[derive(Debug)]
pub enum AccountingOutcome {
    Counted,
    Faulted { message: String },
}

#[derive(Debug, Clone)]
pub struct QuotaDecision {
    pub allow: bool,
    pub window_seconds: i32,
    pub message: String,
    pub state: QuotaBucketState,
}

fn fault_decision(window: &QuotaWindow, fault: &str) -> QuotaDecision {
    QuotaDecision {
        allow: false,
        window_seconds: window.window_seconds,
        message: format!(
            "quota window {}s for subject '{}' could not be evaluated: {fault}",
            window.window_seconds, window.subject
        ),
        state: QuotaBucketState {
            requests: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost_microdollars: 0,
        },
    }
}

fn ceiling_decision(
    window: &QuotaWindow,
    subject_kind: &str,
    state: QuotaBucketState,
) -> Option<QuotaDecision> {
    let breach = [
        (window.max_requests, state.requests, "request", "used", ""),
        (
            window.max_input_tokens,
            state.input_tokens,
            "input token",
            "consumed",
            " tokens",
        ),
        (
            window.max_output_tokens,
            state.output_tokens,
            "output token",
            "consumed",
            " tokens",
        ),
        (
            window.max_cost_microdollars,
            state.cost_microdollars,
            "cost",
            "spent",
            " microdollars",
        ),
    ]
    .into_iter()
    .find_map(|(max, used, label, verb, unit)| match max {
        Some(max) if used > max => Some((max, used, label, verb, unit)),
        _ => None,
    });
    let (max, used, label, verb, unit) = breach?;
    Some(QuotaDecision {
        allow: false,
        window_seconds: window.window_seconds,
        message: format!(
            "{label} ceiling exceeded for {subject_kind} window {}s ({verb} {used}/{max}{unit})",
            window.window_seconds
        ),
        state,
    })
}

pub async fn precheck_and_reserve(
    db: &DbPool,
    repo: &AiQuotaBucketRepository,
    user_id: &UserId,
    windows: &[QuotaWindow],
    fault_mode: QuotaFaultMode,
) -> Result<Option<QuotaDecision>> {
    if windows.is_empty() {
        return Ok(None);
    }
    let pool = db
        .pool_arc()
        .map_err(|e| anyhow::anyhow!("quota pool init: {e}"))?;

    let now = Utc::now();
    for window in windows {
        let subject = match resolve_subject(window, user_id, &pool).await {
            SubjectResolution::Resolved(subject) => subject,
            SubjectResolution::Fault(fault) => {
                if fault_mode.is_closed() {
                    return Ok(Some(fault_decision(window, fault)));
                }
                tracing::warn!(
                    subject = %window.subject,
                    window_seconds = window.window_seconds,
                    fault,
                    fault_mode = fault_mode.as_str(),
                    "Quota window not evaluated; allowing the request"
                );
                continue;
            },
        };
        let window_start = align_window(now, window.window_seconds);
        let state = repo
            .increment(IncrementParams {
                subject_kind: subject.kind,
                subject_id: &subject.id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 1,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_microdollars: 0,
                },
            })
            .await?;

        if let Some(decision) = ceiling_decision(window, subject.kind, state) {
            return Ok(Some(decision));
        }
    }
    Ok(None)
}

#[derive(Debug)]
pub struct PostUpdateParams<'a> {
    pub user_id: &'a UserId,
    pub windows: &'a [QuotaWindow],
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_microdollars: i64,
}

pub async fn post_update_tokens(
    db: &DbPool,
    repo: &AiQuotaBucketRepository,
    params: PostUpdateParams<'_>,
) -> AccountingOutcome {
    if params.windows.is_empty() {
        return AccountingOutcome::Counted;
    }
    let pool = match db.pool_arc() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "quota pool init failed in post_update");
            return AccountingOutcome::Faulted {
                message: format!("quota accounting pool init failed: {e}"),
            };
        },
    };
    let now = Utc::now();
    let mut fault: Option<String> = None;
    for window in params.windows {
        let subject = match resolve_subject(window, params.user_id, &pool).await {
            SubjectResolution::Resolved(subject) => subject,
            SubjectResolution::Fault(reason) => {
                tracing::warn!(
                    subject = %window.subject,
                    window_seconds = window.window_seconds,
                    fault = reason,
                    "Quota accounting skipped: window subject unresolved"
                );
                fault.get_or_insert_with(|| {
                    format!(
                        "quota accounting skipped for window {}s: {reason}",
                        window.window_seconds
                    )
                });
                continue;
            },
        };
        let window_start = align_window(now, window.window_seconds);
        if let Err(e) = repo
            .increment(IncrementParams {
                subject_kind: subject.kind,
                subject_id: &subject.id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 0,
                    input_tokens: i64::from(params.input_tokens),
                    output_tokens: i64::from(params.output_tokens),
                    cost_microdollars: params.cost_microdollars,
                },
            })
            .await
        {
            tracing::warn!(error = %e, window_seconds = window.window_seconds, "quota post_update failed");
            fault.get_or_insert_with(|| {
                format!(
                    "quota accounting write failed for window {}s: {e}",
                    window.window_seconds
                )
            });
        }
    }
    fault.map_or(AccountingOutcome::Counted, |message| {
        AccountingOutcome::Faulted { message }
    })
}

fn align_window(now: DateTime<Utc>, window_seconds: i32) -> DateTime<Utc> {
    let secs = now.timestamp();
    let w = i64::from(window_seconds.max(1));
    let aligned = (secs / w) * w;
    Utc.timestamp_opt(aligned, 0).single().unwrap_or(now)
}
