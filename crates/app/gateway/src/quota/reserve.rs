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

struct ResolvedWindow<'w> {
    window: &'w QuotaWindow,
    subject_kind: &'w str,
    subject_id: String,
    window_start: DateTime<Utc>,
}

struct ResolvedWindows<'w> {
    resolved: Vec<ResolvedWindow<'w>>,
    closed_fault: Option<QuotaDecision>,
}

// Why: subjects are resolved before any bucket is touched so every window
// can be reserved in one statement; a closed fault stops resolution where
// the per-window loop used to stop reserving.
async fn resolve_windows<'w>(
    params: &ReserveParams<'w>,
    now: DateTime<Utc>,
) -> ResolvedWindows<'w> {
    let mut resolved = Vec::with_capacity(params.windows.len());
    for window in params.windows {
        let window_start = align_window(now, window.window_seconds);
        match resolve_subject(window, &params.subjects, params.providers).await {
            SubjectResolution::Resolved(subject) => resolved.push(ResolvedWindow {
                window,
                subject_kind: subject.kind,
                subject_id: subject.id,
                window_start,
            }),
            SubjectResolution::Fault(fault) => {
                if params.fault_mode.is_closed() {
                    return ResolvedWindows {
                        resolved,
                        closed_fault: Some(fault_decision(window, fault, window_start, now)),
                    };
                }
                tracing::warn!(
                    subject = %window.subject,
                    window_seconds = window.window_seconds,
                    fault,
                    fault_mode = params.fault_mode.as_str(),
                    "Quota window not evaluated; allowing the request"
                );
            },
        }
    }
    ResolvedWindows {
        resolved,
        closed_fault: None,
    }
}

pub async fn precheck_and_reserve(
    repo: &AiQuotaBucketRepository,
    params: ReserveParams<'_>,
) -> Result<ReserveOutcome, RepositoryError> {
    let now = Utc::now();
    let delta = QuotaBucketDelta {
        requests: 1,
        input_tokens: i64::from(params.estimate.input_tokens),
        output_tokens: i64::from(params.estimate.output_tokens),
        cost_microdollars: params.estimate.cost_microdollars,
    };
    let ResolvedWindows {
        resolved,
        closed_fault,
    } = resolve_windows(&params, now).await;
    let increments: Vec<IncrementParams<'_>> = resolved
        .iter()
        .map(|r| IncrementParams {
            subject_kind: r.subject_kind,
            subject_id: &r.subject_id,
            window_seconds: r.window.window_seconds,
            window_start: r.window_start,
            delta,
        })
        .collect();
    let states = repo.increment_many(&increments).await?;
    let mut reservation = QuotaReservation::default();
    for (index, (r, state)) in resolved.iter().zip(states).enumerate() {
        reservation.windows.push(ReservedWindow {
            subject_kind: r.subject_kind.to_owned(),
            subject_id: r.subject_id.clone(),
            window_seconds: r.window.window_seconds,
            window_start: r.window_start,
            delta,
        });
        if let Some(decision) = ceiling_decision(r.window, state, r.window_start, now) {
            unreserve(repo, &increments[index + 1..]).await;
            return Ok(ReserveOutcome::Denied {
                decision,
                reservation,
            });
        }
    }
    Ok(match closed_fault {
        Some(decision) => ReserveOutcome::Denied {
            decision,
            reservation,
        },
        None => ReserveOutcome::Admitted(reservation),
    })
}

// Why: the per-window loop stopped at the first exceeded ceiling, so windows
// after it were never charged; the single statement charged them, and this
// takes the whole charge (request included) back out.
async fn unreserve(repo: &AiQuotaBucketRepository, tail: &[IncrementParams<'_>]) {
    if tail.is_empty() {
        return;
    }
    let reversed: Vec<IncrementParams<'_>> = tail
        .iter()
        .map(|p| IncrementParams {
            delta: QuotaBucketDelta {
                requests: -p.delta.requests,
                input_tokens: -p.delta.input_tokens,
                output_tokens: -p.delta.output_tokens,
                cost_microdollars: -p.delta.cost_microdollars,
            },
            ..*p
        })
        .collect();
    if let Err(error) = repo.increment_many(&reversed).await {
        tracing::warn!(%error, windows = reversed.len(), "quota unreserve write failed");
    }
}

pub async fn settle(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
    actual: QuotaUsage,
) -> AccountingOutcome {
    let deltas: Vec<IncrementParams<'_>> = reservation
        .windows
        .iter()
        .map(|window| IncrementParams {
            subject_kind: &window.subject_kind,
            subject_id: &window.subject_id,
            window_seconds: window.window_seconds,
            window_start: window.window_start,
            delta: QuotaBucketDelta {
                requests: 0,
                input_tokens: actual.input_tokens - window.delta.input_tokens,
                output_tokens: actual.output_tokens - window.delta.output_tokens,
                cost_microdollars: actual.cost_microdollars - window.delta.cost_microdollars,
            },
        })
        .collect();
    match repo.increment_many(&deltas).await {
        Ok(_) => AccountingOutcome::Counted,
        Err(e) => {
            tracing::warn!(error = %e, windows = deltas.len(), "quota settlement write failed");
            AccountingOutcome::Faulted {
                message: format!(
                    "quota accounting write failed for {} window(s): {e}",
                    deltas.len()
                ),
            }
        },
    }
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
