//! One event of a drain round, on the round's transaction: decode it, run
//! the pipeline inside its own savepoint, and record what a failure leaves.
//!
//! A permanent failure is recorded and reported done, so the round
//! acknowledges it. A transient failure has already rolled back its
//! savepoint; it is reported for retry unless it is the event's
//! [`MAX_EVENT_ATTEMPTS`]th failure, which is recorded as abandoned and
//! reported done.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, PgConnection};
use systemprompt_events::services::durable::ReportingFact;
use systemprompt_identifiers::{EventOutboxId, PipelineVersion};
use systemprompt_models::brain::{BRAIN_EVENT_VERSION, BrainEvent};

use super::{BrainConsumer, Counters};
use crate::error::{BrainError, BrainResult};
use crate::models::{EventOutcome, EventResult, NewFailure};
use crate::pipeline::{BatchContext, ProcessOutcome, metrics};
use crate::repository::{EventResultRepository, FailureRepository};

pub const MAX_EVENT_ATTEMPTS: i64 = 5;
const DECODE_STAGE: &str = "decode";
const ABANDONED_STAGE: &str = "abandoned";

#[derive(Debug, thiserror::Error)]
enum Undecodable {
    #[error("brain event does not decode")]
    Json(#[from] serde_json::Error),
    #[error("unsupported event version {0}")]
    Version(u32),
}

// JSON: the fact envelope is decoded first, and its data only after its
// version is checked.
fn decode(raw: &serde_json::Value) -> Result<BrainEvent, Undecodable> {
    let fact: ReportingFact<serde_json::Value> = serde_json::from_value(raw.clone())?;
    if fact.version != BRAIN_EVENT_VERSION {
        return Err(Undecodable::Version(fact.version));
    }
    Ok(serde_json::from_value(fact.data)?)
}

pub(super) enum Handled {
    /// Finished for good; `wrote` is whether it changed facts or disputes.
    Done {
        wrote: bool,
    },
    Retry(BrainError),
}

pub(super) struct Failed<'a> {
    pub(super) id: &'a EventOutboxId,
    pub(super) version: &'a PipelineVersion,
    pub(super) stage: &'static str,
    pub(super) reason: String,
    pub(super) attempt: i64,
}

impl BrainConsumer {
    // JSON: the claimed row's fact is undecoded until here.
    pub(super) async fn handle(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        raw: &serde_json::Value,
        ctx: &BatchContext,
    ) -> Handled {
        let event = match decode(raw) {
            Ok(event) => event,
            Err(error) => {
                metrics::event("decode_failed");
                let failed = Failed {
                    id,
                    version: ctx.version(),
                    stage: DECODE_STAGE,
                    reason: error.to_string(),
                    attempt: 1,
                };
                fail(conn, &failed).await;
                Counters::bump(&self.counters.failed);
                return Handled::Done { wrote: false };
            },
        };
        let kind = event.kind.to_string();
        match self.process_in_savepoint(conn, id, event, ctx).await {
            Ok(outcome) => {
                Counters::bump(&self.counters.processed);
                Handled::Done {
                    wrote: wrote(&outcome),
                }
            },
            Err(error) if error.is_transient() => {
                self.retry_or_abandon(conn, id, &kind, ctx, error).await
            },
            Err(error) => {
                metrics::event("failed");
                tracing::warn!(outbox_id = %id, kind = %kind, error = %error, "Brain event failed");
                let failed = Failed {
                    id,
                    version: ctx.version(),
                    stage: error.stage(),
                    reason: error.to_string(),
                    attempt: 1,
                };
                fail(&mut *conn, &failed).await;
                close_result(conn, &failed, &kind, EventOutcome::Failed).await;
                Counters::bump(&self.counters.failed);
                Handled::Done { wrote: false }
            },
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the round's connection, identity and batch context travel with the error"
    )]
    async fn retry_or_abandon(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        kind: &str,
        ctx: &BatchContext,
        error: BrainError,
    ) -> Handled {
        let earlier = match FailureRepository::count_for_outbox(&mut *conn, id).await {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %id, error = %count_error, "Brain event attempts could not be read");
                return Handled::Retry(error);
            },
        };
        if earlier + 1 < MAX_EVENT_ATTEMPTS {
            return Handled::Retry(error);
        }
        metrics::event("abandoned");
        tracing::error!(outbox_id = %id, kind = %kind, attempts = earlier + 1, error = %error, "Brain event abandoned");
        let failed = Failed {
            id,
            version: ctx.version(),
            stage: ABANDONED_STAGE,
            reason: format!("event_abandoned: {error}"),
            attempt: earlier + 1,
        };
        fail(&mut *conn, &failed).await;
        close_result(conn, &failed, kind, EventOutcome::Abandoned).await;
        Counters::bump(&self.counters.abandoned);
        Handled::Done { wrote: false }
    }

    /// Records a transient attempt on the round's transaction; the event's
    /// own work was already rolled back with its savepoint, and its row stays
    /// unacknowledged.
    pub(super) async fn record_retry(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        version: &PipelineVersion,
        error: &BrainError,
    ) {
        metrics::event("retry");
        Counters::bump(&self.counters.retried);
        tracing::warn!(outbox_id = %id, error = %error, "Brain event failed; retrying");
        let earlier = match FailureRepository::count_for_outbox(&mut *conn, id).await {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %id, error = %count_error, "Brain event attempts could not be read");
                0
            },
        };
        let failed = Failed {
            id,
            version,
            stage: error.stage(),
            reason: error.to_string(),
            attempt: earlier + 1,
        };
        fail(conn, &failed).await;
    }

    async fn process_in_savepoint(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        event: BrainEvent,
        ctx: &BatchContext,
    ) -> BrainResult<ProcessOutcome> {
        let mut savepoint = conn.begin().await?;
        match self.pipeline.process(id, event, &mut savepoint, ctx).await {
            Ok(outcome) => {
                savepoint.commit().await?;
                if let ProcessOutcome::Finished(summary) = &outcome {
                    tracing::debug!(
                        outbox_id = %id,
                        outcome = summary.outcome.as_str(),
                        staled = summary.staled,
                        written = summary.tally.written,
                        disputed = summary.tally.disputed,
                        rejected = summary.tally.rejected,
                        "Brain event processed"
                    );
                }
                Ok(outcome)
            },
            Err(error) => {
                savepoint.rollback().await?;
                Err(error)
            },
        }
    }
}

const fn wrote(outcome: &ProcessOutcome) -> bool {
    match outcome {
        ProcessOutcome::Skipped => false,
        ProcessOutcome::Finished(summary) => {
            summary.staled > 0 || summary.tally.written > 0 || summary.tally.disputed > 0
        },
    }
}

async fn fail(executor: impl sqlx::PgExecutor<'_>, failed: &Failed<'_>) {
    let failure = NewFailure {
        outbox_id: failed.id.clone(),
        pipeline_version: Some(failed.version.clone()),
        stage: failed.stage,
        reason: failed.reason.clone(),
        attempt: u32::try_from(failed.attempt).unwrap_or(u32::MAX),
    };
    if let Err(error) = FailureRepository::record(executor, &failure).await {
        tracing::error!(outbox_id = %failed.id, stage = failed.stage, error = %error, "Brain failure could not be recorded");
    }
}

async fn close_result(
    conn: &mut PgConnection,
    failed: &Failed<'_>,
    kind: &str,
    outcome: EventOutcome,
) {
    let result = EventResult {
        outbox_id: failed.id.clone(),
        pipeline_version: failed.version.clone(),
        outcome,
        filter: None,
        facts_written: 0,
    };
    let closed =
        match EventResultRepository::claim(&mut *conn, failed.id, failed.version, kind).await {
            Ok(_) => EventResultRepository::finish(conn, &result).await,
            Err(error) => Err(error),
        };
    if let Err(error) = closed {
        tracing::error!(outbox_id = %failed.id, error = %error, "Brain event result could not be closed");
    }
}
