//! Admission reservation and its settlement.
//!
//! Admission adds one request and the request's [`QuotaEstimate`] to every
//! window's bucket and checks the ceilings against the result, so spend that
//! is still in flight already counts. Overshoot is bounded by one estimate per
//! concurrent request. Completion settles each reserved bucket by the
//! difference between the audited usage and the estimate, in the bucket the
//! reservation landed in (not re-aligned, so a completion that crosses a window
//! boundary trues up the window it was admitted to); failure or abandonment
//! releases the tokens and cost. The request count is never returned: an
//! admitted request is one request whatever its outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, TimeZone, Utc};
use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_security::authz::SubjectProviderSet;
use systemprompt_traits::RepositoryError;

use super::AccountingOutcome;
use super::decision::{QuotaDecision, ceiling_decision, fault_decision};
use super::estimate::QuotaEstimate;
use super::subject::{SubjectResolution, resolve_subject};
use crate::policies::QuotaWindow;

/// Who a request's quota windows can be keyed by.
#[derive(Debug, Clone, Copy)]
pub struct QuotaSubjects<'a> {
    pub user_id: &'a UserId,
    pub api_key_id: Option<&'a ApiKeyId>,
    pub attribution: &'a RequestAttribution,
}

/// One bucket row a reservation incremented, and by how much.
#[derive(Debug, Clone)]
pub struct ReservedWindow {
    pub subject_kind: String,
    pub subject_id: String,
    pub window_seconds: i32,
    pub window_start: DateTime<Utc>,
    pub delta: QuotaBucketDelta,
}

/// Every bucket a request's admission reserved in.
#[derive(Debug, Clone, Default)]
pub struct QuotaReservation {
    pub windows: Vec<ReservedWindow>,
}

impl QuotaReservation {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

/// The admission verdict. A denial still carries what was reserved: the
/// caller releases it (enforce) or keeps it for settlement (warn).
#[derive(Debug, Clone)]
pub enum ReserveOutcome {
    Admitted(QuotaReservation),
    Denied {
        decision: QuotaDecision,
        reservation: QuotaReservation,
    },
}

/// The audited usage a reservation settles to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_microdollars: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct ReserveParams<'a> {
    pub providers: &'a SubjectProviderSet,
    pub subjects: QuotaSubjects<'a>,
    pub windows: &'a [QuotaWindow],
    pub fault_mode: QuotaFaultMode,
    pub estimate: QuotaEstimate,
}

pub async fn precheck_and_reserve(
    repo: &AiQuotaBucketRepository,
    params: ReserveParams<'_>,
) -> Result<ReserveOutcome, RepositoryError> {
    let mut reservation = QuotaReservation::default();
    let now = Utc::now();
    let delta = QuotaBucketDelta {
        requests: 1,
        input_tokens: i64::from(params.estimate.input_tokens),
        output_tokens: i64::from(params.estimate.output_tokens),
        cost_microdollars: params.estimate.cost_microdollars,
    };
    for window in params.windows {
        let window_start = align_window(now, window.window_seconds);
        let subject = match resolve_subject(window, &params.subjects, params.providers).await {
            SubjectResolution::Resolved(subject) => subject,
            SubjectResolution::Fault(fault) => {
                if params.fault_mode.is_closed() {
                    let decision = fault_decision(window, fault, window_start, now);
                    return Ok(ReserveOutcome::Denied {
                        decision,
                        reservation,
                    });
                }
                tracing::warn!(
                    subject = %window.subject,
                    window_seconds = window.window_seconds,
                    fault,
                    fault_mode = params.fault_mode.as_str(),
                    "Quota window not evaluated; allowing the request"
                );
                continue;
            },
        };
        let state = match repo
            .increment(IncrementParams {
                subject_kind: subject.kind,
                subject_id: &subject.id,
                window_seconds: window.window_seconds,
                window_start,
                delta,
            })
            .await
        {
            Ok(state) => state,
            Err(error) => {
                release(repo, &reservation).await;
                return Err(error);
            },
        };
        reservation.windows.push(ReservedWindow {
            subject_kind: subject.kind.to_owned(),
            subject_id: subject.id,
            window_seconds: window.window_seconds,
            window_start,
            delta,
        });
        if let Some(decision) = ceiling_decision(window, state, window_start, now) {
            return Ok(ReserveOutcome::Denied {
                decision,
                reservation,
            });
        }
    }
    Ok(ReserveOutcome::Admitted(reservation))
}

pub async fn settle(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
    actual: QuotaUsage,
) -> AccountingOutcome {
    let mut fault: Option<String> = None;
    for window in &reservation.windows {
        let delta = QuotaBucketDelta {
            requests: 0,
            input_tokens: actual.input_tokens - window.delta.input_tokens,
            output_tokens: actual.output_tokens - window.delta.output_tokens,
            cost_microdollars: actual.cost_microdollars - window.delta.cost_microdollars,
        };
        if let Err(e) = repo
            .increment(IncrementParams {
                subject_kind: &window.subject_kind,
                subject_id: &window.subject_id,
                window_seconds: window.window_seconds,
                window_start: window.window_start,
                delta,
            })
            .await
        {
            tracing::warn!(
                error = %e,
                subject = %window.subject_kind,
                window_seconds = window.window_seconds,
                "quota settlement write failed"
            );
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

pub async fn release(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
) -> AccountingOutcome {
    settle(repo, reservation, QuotaUsage::default()).await
}

fn align_window(now: DateTime<Utc>, window_seconds: i32) -> DateTime<Utc> {
    let secs = now.timestamp();
    let w = i64::from(window_seconds.max(1));
    let aligned = (secs / w) * w;
    Utc.timestamp_opt(aligned, 0).single().unwrap_or(now)
}
