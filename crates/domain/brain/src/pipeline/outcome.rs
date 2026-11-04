//! What processing one event did, and the result row that records it in
//! the idempotency ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::BrainFactId;

use super::candidate::CandidateResult;
use super::extract::Unreadable;
use super::normalise::Normalised;
use super::persist::{Effect, Persisted};
use super::round::DedupeWrite;
use super::{ProcessParams, metrics};
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
            CandidateResult::Stored(Persisted {
                effect: Effect::Tombstoned,
                ..
            }) => (&mut self.rejected, "tombstoned"),
            CandidateResult::Stored(Persisted {
                effect: Effect::Disputed,
                ..
            }) => (&mut self.disputed, "disputed"),
            CandidateResult::Stored(Persisted {
                effect: Effect::Unchanged,
                ..
            }) => (&mut self.unchanged, "unchanged"),
            CandidateResult::Stored(Persisted {
                effect: Effect::Created,
                ..
            }) => (&mut self.written, "created"),
            CandidateResult::Stored(Persisted {
                effect: Effect::Revised,
                ..
            }) => (&mut self.written, "revised"),
            CandidateResult::Stored(Persisted {
                effect: Effect::Merged,
                ..
            }) => (&mut self.written, "merged"),
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

/// The finished record of one event, with the facts its stored candidates
/// landed on, the dedupe writes it committed for the round cache, and its
/// result row when the round writes the ledger as a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSummary {
    pub outcome: EventOutcome,
    pub filter: Option<String>,
    pub staled: usize,
    pub tally: Tally,
    pub facts: Vec<BrainFactId>,
    pub writes: Vec<DedupeWrite>,
    pub result: Option<EventResult>,
}

impl Default for EventSummary {
    fn default() -> Self {
        Self {
            outcome: EventOutcome::Processed,
            filter: None,
            staled: 0,
            tally: Tally::default(),
            facts: Vec::new(),
            writes: Vec::new(),
            result: None,
        }
    }
}

impl EventSummary {
    #[must_use]
    pub const fn with(mut self, outcome: EventOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    pub fn record(&mut self, result: &CandidateResult) {
        self.tally.record(result);
        if let CandidateResult::Stored(Persisted {
            fact: Some(fact), ..
        }) = result
            && !self.facts.contains(fact)
        {
            self.facts.push(fact.clone());
        }
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
    params: &ProcessParams<'_>,
    event: &Normalised,
    mut summary: EventSummary,
) -> BrainResult<ProcessOutcome> {
    let result = EventResult {
        outbox_id: event.envelope.outbox_id.clone(),
        pipeline_version: params.ctx.version().clone(),
        outcome: summary.outcome,
        filter: summary.filter.clone(),
        facts_written: summary.tally.written,
    };
    if params.round.is_some() {
        summary.result = Some(result);
    } else {
        EventResultRepository::finish(conn, &result).await?;
    }
    metrics::event(summary.outcome.as_str());
    Ok(ProcessOutcome::Finished(summary))
}

pub(super) async fn unreadable(
    conn: &mut PgConnection,
    params: &ProcessParams<'_>,
    event: &Normalised,
    summary: EventSummary,
    reason: Unreadable,
) -> BrainResult<ProcessOutcome> {
    tracing::warn!(outbox_id = %event.envelope.outbox_id, reason = %reason, "Brain source unreadable");
    let failure = NewFailure {
        outbox_id: event.envelope.outbox_id.clone(),
        pipeline_version: Some(params.ctx.version().clone()),
        stage: SOURCE_STAGE,
        reason: reason.to_string(),
        attempt: 1,
    };
    FailureRepository::record(&mut *conn, &failure).await?;
    finish(conn, params, event, summary.with(EventOutcome::Rejected)).await
}
