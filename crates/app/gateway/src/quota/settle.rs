//! Settling, releasing and revoking a [`QuotaReservation`].
//!
//! Every write is one `increment_many` statement over all of a reservation's
//! windows, so the windows never disagree about a request. `revoke` takes a
//! denied request's whole charge (request count included) back out;
//! `settle_unmetered` settles a response that reported no usage to the larger
//! of its reservation and what was observed, so it is neither free nor
//! refunded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};

use super::AccountingOutcome;
use super::reserve::{QuotaReservation, QuotaUsage};

pub async fn revoke(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
) -> AccountingOutcome {
    let reversed: Vec<IncrementParams<'_>> = reservation
        .windows
        .iter()
        .map(|window| IncrementParams {
            subject_kind: window.subject_kind.as_str(),
            subject_id: &window.subject_id,
            window_seconds: window.window_seconds,
            window_start: window.window_start,
            shard: window.shard,
            delta: QuotaBucketDelta {
                requests: -window.delta.requests,
                input_tokens: -window.delta.input_tokens,
                output_tokens: -window.delta.output_tokens,
                cost_microdollars: -window.delta.cost_microdollars,
            },
        })
        .collect();
    write_deltas(repo, &reversed, "revocation").await
}

pub async fn settle(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
    actual: QuotaUsage,
) -> AccountingOutcome {
    settle_each(repo, reservation, |_| actual).await
}

/// Whether an unmetered settlement floors the cost at the reservation
/// (platform-billed) or charges no cost at all (billed to the user's own key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostFloor {
    Reserved,
    Waived,
}

pub async fn settle_unmetered(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
    observed: QuotaUsage,
    cost_floor: CostFloor,
) -> AccountingOutcome {
    settle_each(repo, reservation, |reserved| QuotaUsage {
        input_tokens: observed.input_tokens.max(reserved.input_tokens),
        output_tokens: observed.output_tokens.max(reserved.output_tokens),
        cost_microdollars: match cost_floor {
            CostFloor::Reserved => observed.cost_microdollars.max(reserved.cost_microdollars),
            CostFloor::Waived => 0,
        },
    })
    .await
}

async fn settle_each(
    repo: &AiQuotaBucketRepository,
    reservation: &QuotaReservation,
    target: impl Fn(&QuotaBucketDelta) -> QuotaUsage,
) -> AccountingOutcome {
    let deltas: Vec<IncrementParams<'_>> = reservation
        .windows
        .iter()
        .map(|window| {
            let actual = target(&window.delta);
            IncrementParams {
                subject_kind: window.subject_kind.as_str(),
                subject_id: &window.subject_id,
                window_seconds: window.window_seconds,
                window_start: window.window_start,
                shard: window.shard,
                delta: QuotaBucketDelta {
                    requests: 0,
                    input_tokens: actual.input_tokens - window.delta.input_tokens,
                    output_tokens: actual.output_tokens - window.delta.output_tokens,
                    cost_microdollars: actual.cost_microdollars - window.delta.cost_microdollars,
                },
            }
        })
        .collect();
    write_deltas(repo, &deltas, "settlement").await
}

async fn write_deltas(
    repo: &AiQuotaBucketRepository,
    deltas: &[IncrementParams<'_>],
    write: &'static str,
) -> AccountingOutcome {
    if deltas.is_empty() {
        return AccountingOutcome::Counted;
    }
    match repo.increment_many(deltas).await {
        Ok(_) => AccountingOutcome::Counted,
        Err(e) => {
            tracing::warn!(error = %e, windows = deltas.len(), write, "quota write failed");
            AccountingOutcome::Faulted {
                message: format!(
                    "quota {write} write failed for {} window(s): {e}",
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
