//! What processing one event did, and the result row that records it in
//! the idempotency ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;

use super::candidate::CandidateResult;
use super::context::BatchContext;
use super::extract::Unreadable;
use super::metrics;
use super::normalise::Normalised;
use super::persist::Effect;
use crate::error::BrainResult;
use crate::models::{EventOutcome, EventResult, NewFailure};
use crate::repository::{EventResultRepository, FailureRepository};

const SOURCE_STAGE: &str = "source";

/// How the candidates of one event fared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub written: u32,
    pub unchanged: u32,
    pub disputed: u32,
    pub rejected: u32,
}

impl Tally {
    pub fn record(&mut self, result: &CandidateResult) {
        let (slot, label) = match result {
            CandidateResult::Rejected { by, reason } => {
                tracing::debug!(validator = %by, reason = %reason, "Brain candidate rejected");
                (&mut self.rejected, "rejected")
            },
            CandidateResult::Stored(Effect::Tombstoned) => (&mut self.rejected, "tombstoned"),
            CandidateResult::Stored(Effect::Disputed) => (&mut self.disputed, "disputed"),
            CandidateResult::Stored(Effect::Unchanged) => (&mut self.unchanged, "unchanged"),
            CandidateResult::Stored(Effect::Created) => (&mut self.written, "created"),
            CandidateResult::Stored(Effect::Revised) => (&mut self.written, "revised"),
            CandidateResult::Stored(Effect::Merged) => (&mut self.written, "merged"),
        };
        *slot = slot.saturating_add(1);
        metrics::candidate(label);
    }

    #[must_use]
    pub const fn outcome(&self) -> EventOutcome {
        if self.rejected > 0 && self.written + self.unchanged + self.disputed == 0 {
            EventOutcome::Rejected
        } else {
            EventOutcome::Processed
        }
    }
}

/// The finished record of one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSummary {
    pub outcome: EventOutcome,
    pub filter: Option<String>,
    pub staled: usize,
    pub tally: Tally,
}

impl Default for EventSummary {
    fn default() -> Self {
        Self {
            outcome: EventOutcome::Processed,
            filter: None,
            staled: 0,
            tally: Tally::default(),
        }
    }
}

impl EventSummary {
    #[must_use]
    pub const fn with(mut self, outcome: EventOutcome) -> Self {
        self.outcome = outcome;
        self
    }
}

/// Whether the event was processed now or already under this version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessOutcome {
    Skipped,
    Finished(EventSummary),
}

pub(super) async fn finish(
    conn: &mut PgConnection,
    ctx: &BatchContext,
    event: &Normalised,
    summary: EventSummary,
) -> BrainResult<ProcessOutcome> {
    let result = EventResult {
        outbox_id: event.envelope.outbox_id.clone(),
        pipeline_version: ctx.version().clone(),
        outcome: summary.outcome,
        filter: summary.filter.clone(),
        facts_written: summary.tally.written,
    };
    EventResultRepository::finish(conn, &result).await?;
    metrics::event(summary.outcome.as_str());
    Ok(ProcessOutcome::Finished(summary))
}

pub(super) async fn unreadable(
    conn: &mut PgConnection,
    ctx: &BatchContext,
    event: &Normalised,
    summary: EventSummary,
    reason: Unreadable,
) -> BrainResult<ProcessOutcome> {
    tracing::warn!(outbox_id = %event.envelope.outbox_id, reason = %reason, "Brain source unreadable");
    let failure = NewFailure {
        outbox_id: event.envelope.outbox_id.clone(),
        pipeline_version: Some(ctx.version().clone()),
        stage: SOURCE_STAGE,
        reason: reason.to_string(),
        attempt: 1,
    };
    FailureRepository::record(&mut *conn, &failure).await?;
    finish(conn, ctx, event, summary.with(EventOutcome::Rejected)).await
}
