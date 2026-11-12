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
//! admitted request is one request whatever its outcome. A denied request is
//! charged in every window and then revoked from every window in one
//! statement, so it counts in none of them whatever their order.
//!
//! A window keyed by a shared subject is spread over
//! `QuotaConfig::shared_subject_shards` rows; per-principal (`user`,
//! `api_key`) windows over `principal_subject_shards`. One random draw per
//! request picks the shard of every window, the shard is kept on the
//! [`ReservedWindow`] so settlement lands on the same row, and the ceiling is
//! checked against the window total across shards. Each shard beyond the
//! first widens the overshoot bound by up to one reservation per admission
//! running concurrently on another shard (see `AiQuotaBucketRepository`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_identifiers::{ApiKeyId, QuotaSubjectKind, UserId};
use systemprompt_manifest::profile::QuotaConfig;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_traits::RepositoryError;

use super::decision::{QuotaDecision, ceiling_decision, fault_decision};
use super::distinct::distinct_increments;
use super::estimate::QuotaEstimate;
use super::subject::{SubjectResolution, resolve_subject};
use crate::policies::{QuotaWindow, WindowSpan};

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
    pub subject_kind: QuotaSubjectKind,
    pub subject_id: String,
    pub window_seconds: i32,
    pub window_start: DateTime<Utc>,
    pub shard: i16,
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

/// The admission verdict.
///
/// A denial carries every window the request was charged in: the caller
/// revokes the whole charge (enforce), so the request counts in no window, or
/// keeps it for settlement (warn), so it counts in every window.
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
    pub sharding: QuotaConfig,
}

struct ResolvedWindow<'w> {
    window: &'w QuotaWindow,
    subject_kind: QuotaSubjectKind,
    subject_id: String,
    span: WindowSpan,
    shard: i16,
}

struct ResolvedWindows<'w> {
    resolved: Vec<ResolvedWindow<'w>>,
    closed_fault: Option<QuotaDecision>,
}

async fn resolve_windows<'w>(
    params: &ReserveParams<'w>,
    now: DateTime<Utc>,
) -> ResolvedWindows<'w> {
    let mut resolved = Vec::with_capacity(params.windows.len());
    let draw: u16 = rand::random();
    for window in params.windows {
        let span = window.span(now);
        match resolve_subject(window, &params.subjects, params.providers).await {
            SubjectResolution::Resolved(subject) => resolved.push(ResolvedWindow {
                window,
                shard: pick_shard(&params.sharding, &subject.kind, draw),
                subject_kind: subject.kind,
                subject_id: subject.id,
                span,
            }),
            SubjectResolution::Fault(fault) => {
                if params.fault_mode.is_closed() {
                    return ResolvedWindows {
                        resolved,
                        closed_fault: Some(fault_decision(window, fault, span, now)),
                    };
                }
                tracing::warn!(
                    subject = %window.subject,
                    window = %window.label(),
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
    let (increments, slots) = distinct_increments(
        resolved
            .iter()
            .map(|r| IncrementParams {
                subject_kind: r.subject_kind.as_str(),
                subject_id: &r.subject_id,
                window_seconds: r.span.bucket_seconds,
                window_start: r.span.start,
                shard: r.shard,
                delta,
            })
            .collect(),
    );
    let states = repo.increment_many(&increments).await?;
    let mut reservation = QuotaReservation::default();
    for (r, &slot) in resolved.iter().zip(&slots) {
        if slot == reservation.windows.len() {
            let charged = &increments[slot];
            reservation.windows.push(ReservedWindow {
                subject_kind: r.subject_kind.clone(),
                subject_id: charged.subject_id.to_owned(),
                window_seconds: charged.window_seconds,
                window_start: charged.window_start,
                shard: charged.shard,
                delta,
            });
        }
    }
    let denial = resolved
        .iter()
        .zip(&slots)
        .find_map(|(r, &slot)| ceiling_decision(r.window, states[slot], r.span, now))
        .or(closed_fault);
    Ok(match denial {
        Some(decision) => ReserveOutcome::Denied {
            decision,
            reservation,
        },
        None => ReserveOutcome::Admitted(reservation),
    })
}

fn pick_shard(sharding: &QuotaConfig, subject_kind: &QuotaSubjectKind, draw: u16) -> i16 {
    let shards = if subject_kind.is_principal() {
        sharding.principal_subject_shards
    } else {
        sharding.shared_subject_shards
    };
    i16::try_from(draw % shards.max(1)).unwrap_or(0)
}
