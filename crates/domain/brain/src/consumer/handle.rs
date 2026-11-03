//! One event of a drain round, on the round's transaction: decoded up
//! front with the rest of the round, run through the pipeline inside its
//! own savepoint with the derived indexes deferred and the round cache
//! serving its reads, and recorded on failure. A finished event reports
//! the facts it landed on and its result row for the round's set-based
//! reindex and ledger finish, and its committed dedupe writes are applied
//! to the round cache before the next event runs.
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
use systemprompt_identifiers::{BrainFactId, EventOutboxId, PipelineVersion};
use systemprompt_models::brain::BrainEvent;

use super::decode::Decoded;
use super::{BrainConsumer, Counters};
use crate::error::{BrainError, BrainResult};
use crate::models::{EventOutcome, EventResult, NewFailure};
use crate::pipeline::{ProcessOutcome, ProcessParams, metrics};
use crate::repository::{EventResultRepository, FailureRepository};

pub const MAX_EVENT_ATTEMPTS: i64 = 5;
const DECODE_STAGE: &str = "decode";
const ABANDONED_STAGE: &str = "abandoned";

pub(super) enum Handled {
    Done {
        wrote: bool,
        facts: Vec<BrainFactId>,
        result: Option<EventResult>,
    },
    Retry(BrainError),
}

impl Handled {
    const fn failed() -> Self {
        Self::Done {
            wrote: false,
            facts: Vec::new(),
            result: None,
        }
    }
}

pub(super) struct Failed<'a> {
    pub(super) id: &'a EventOutboxId,
    pub(super) version: &'a PipelineVersion,
    pub(super) stage: &'static str,
    pub(super) reason: String,
    pub(super) attempt: i64,
}

struct Attempt<'a> {
    id: &'a EventOutboxId,
    kind: &'a str,
    version: &'a PipelineVersion,
}

impl BrainConsumer {
    pub(super) async fn handle(
        &self,
        conn: &mut PgConnection,
        decoded: Decoded,
        params: &ProcessParams<'_>,
    ) -> Handled {
        let id = &decoded.id;
        let event = match decoded.event {
            Ok(event) => event,
            Err(error) => {
                metrics::event("decode_failed");
                let failed = Failed {
                    id,
                    version: params.ctx.version(),
                    stage: DECODE_STAGE,
                    reason: error.to_string(),
                    attempt: 1,
                };
                fail(conn, &failed).await;
                Counters::bump(&self.counters.failed);
                return Handled::failed();
            },
        };
        let kind = event.kind.to_string();
        match self.process_in_savepoint(conn, id, event, params).await {
            Ok(outcome) => {
                Counters::bump(&self.counters.processed);
                let wrote = wrote(&outcome);
                let (facts, result) = landed(outcome);
                Handled::Done {
                    wrote,
                    facts,
                    result,
                }
            },
            Err(error) if error.is_transient() => {
                let attempt = Attempt {
                    id,
                    kind: &kind,
                    version: params.ctx.version(),
                };
                self.retry_or_abandon(conn, &attempt, error).await
            },
            Err(error) => {
                metrics::event("failed");
                tracing::warn!(outbox_id = %id, kind = %kind, error = %error, "Brain event failed");
                let failed = Failed {
                    id,
                    version: params.ctx.version(),
                    stage: error.stage(),
                    reason: error.to_string(),
                    attempt: 1,
                };
                fail(&mut *conn, &failed).await;
                close_result(conn, &failed, &kind, EventOutcome::Failed).await;
                Counters::bump(&self.counters.failed);
                Handled::failed()
            },
        }
    }

    async fn retry_or_abandon(
        &self,
        conn: &mut PgConnection,
        attempt: &Attempt<'_>,
        error: BrainError,
    ) -> Handled {
        let earlier = match FailureRepository::count_for_outbox(&mut *conn, attempt.id).await {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %attempt.id, error = %count_error, "Brain event attempts could not be read");
                return Handled::Retry(error);
            },
        };
        if earlier + 1 < MAX_EVENT_ATTEMPTS {
            return Handled::Retry(error);
        }
        metrics::event("abandoned");
        tracing::error!(outbox_id = %attempt.id, kind = %attempt.kind, attempts = earlier + 1, error = %error, "Brain event abandoned");
        let failed = Failed {
            id: attempt.id,
            version: attempt.version,
            stage: ABANDONED_STAGE,
            reason: format!("event_abandoned: {error}"),
            attempt: earlier + 1,
        };
        fail(&mut *conn, &failed).await;
        close_result(conn, &failed, attempt.kind, EventOutcome::Abandoned).await;
        Counters::bump(&self.counters.abandoned);
        Handled::failed()
    }

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
        params: &ProcessParams<'_>,
    ) -> BrainResult<ProcessOutcome> {
        let mut savepoint = conn.begin().await?;
        match self
            .pipeline
            .process_with(id, event, &mut savepoint, params)
            .await
        {
            Ok(mut outcome) => {
                savepoint.commit().await?;
                if let ProcessOutcome::Finished(summary) = &mut outcome {
                    if let Some(round) = params.round {
                        round.apply(std::mem::take(&mut summary.writes), params.ctx.loaded_at());
                    }
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

fn landed(outcome: ProcessOutcome) -> (Vec<BrainFactId>, Option<EventResult>) {
    match outcome {
        ProcessOutcome::Skipped => (Vec::new(), None),
        ProcessOutcome::Finished(summary) => (summary.facts, summary.result),
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

pub(super) async fn fail(executor: impl sqlx::PgExecutor<'_>, failed: &Failed<'_>) {
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

pub(super) async fn close_result(
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
