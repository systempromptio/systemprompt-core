//! One drain round: claim, process and acknowledge events one transaction
//! at a time. An event that failed transiently ends the round
//! unacknowledged, so the next round retries it after the poll interval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use sqlx::{Acquire, PgConnection};
use systemprompt_events::services::durable::Delivery;
use systemprompt_identifiers::{EventOutboxId, PipelineVersion};
use systemprompt_models::brain::{BRAIN_CONSUMER, BRAIN_EVENT_VERSION, BrainEvent};

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
fn decode(delivery: &Delivery) -> Result<BrainEvent, Undecodable> {
    let fact = delivery.fact::<serde_json::Value>()?;
    if fact.version != BRAIN_EVENT_VERSION {
        return Err(Undecodable::Version(fact.version));
    }
    Ok(serde_json::from_value(fact.data)?)
}

enum Handled {
    Done,
    Retry(BrainError),
}

struct Failed<'a> {
    id: &'a EventOutboxId,
    version: &'a PipelineVersion,
    stage: &'static str,
    reason: String,
    attempt: i64,
}

impl BrainConsumer {
    pub(super) async fn drain_round(&self) -> BrainResult<bool> {
        let started = Instant::now();
        let mut ctx: Option<BatchContext> = None;
        let mut claimed = 0;
        let mut retry_later = false;
        while claimed < self.batch() {
            let Some(mut delivery) = self.outbox.claim(BRAIN_CONSUMER).await? else {
                break;
            };
            claimed += 1;
            if ctx.is_none() {
                ctx = Some(BatchContext::load(delivery.connection()).await?);
            }
            let Some(context) = ctx.as_ref() else {
                break;
            };
            let id = delivery.id();
            match self.handle(&mut delivery, &id, context).await {
                Handled::Done => {
                    if let Err(error) = delivery.acknowledge().await {
                        tracing::error!(outbox_id = %id, error = %error, "Brain event acknowledgement failed");
                        break;
                    }
                },
                Handled::Retry(error) => {
                    self.release_for_retry(delivery, &id, context.version(), &error)
                        .await;
                    retry_later = true;
                    break;
                },
            }
        }
        if claimed > 0 {
            metrics::batch_seconds(started.elapsed().as_secs_f64());
        }
        Ok(claimed > 0 && !retry_later)
    }

    async fn handle(
        &self,
        delivery: &mut Delivery,
        id: &EventOutboxId,
        ctx: &BatchContext,
    ) -> Handled {
        let decoded = decode(delivery);
        let event = match decoded {
            Ok(event) => event,
            Err(error) => {
                metrics::event("decode_failed");
                let reason = error.to_string();
                let failed = Failed {
                    id,
                    version: ctx.version(),
                    stage: DECODE_STAGE,
                    reason,
                    attempt: 1,
                };
                fail(delivery.connection(), &failed).await;
                Counters::bump(&self.counters.failed);
                return Handled::Done;
            },
        };
        let kind = event.kind.to_string();
        match self
            .process_in_savepoint(delivery.connection(), id, event, ctx)
            .await
        {
            Ok(_) => {
                Counters::bump(&self.counters.processed);
                Handled::Done
            },
            Err(error) if error.is_transient() => {
                self.retry_or_abandon(delivery.connection(), id, &kind, ctx, error)
                    .await
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
                fail(delivery.connection(), &failed).await;
                close_result(delivery.connection(), &failed, &kind, EventOutcome::Failed).await;
                Counters::bump(&self.counters.failed);
                Handled::Done
            },
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the delivery's connection, identity and batch context travel with the error"
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
        Handled::Done
    }

    async fn release_for_retry(
        &self,
        delivery: Delivery,
        id: &EventOutboxId,
        version: &PipelineVersion,
        error: &BrainError,
    ) {
        metrics::event("retry");
        Counters::bump(&self.counters.retried);
        tracing::warn!(outbox_id = %id, error = %error, "Brain event failed; retrying");
        if let Err(rollback) = delivery.rollback().await {
            tracing::error!(outbox_id = %id, error = %rollback, "Brain event rollback failed");
        }
        let earlier = match FailureRepository::count_for_outbox(&self.pool, id).await {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %id, error = %count_error, "Brain event attempts could not be read");
                0
            },
        };
        let attempt = earlier + 1;
        let failed = Failed {
            id,
            version,
            stage: error.stage(),
            reason: error.to_string(),
            attempt,
        };
        fail(&self.pool, &failed).await;
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
