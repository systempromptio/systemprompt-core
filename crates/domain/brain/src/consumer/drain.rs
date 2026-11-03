//! One drain round: claim up to a batch of events on one transaction,
//! process each inside its own savepoint, acknowledge each finished event in
//! that transaction, and commit once.
//!
//! A transient failure no longer ends the round: its savepoint is rolled
//! back, its attempt is recorded, its row stays unacknowledged and the
//! consumer holds it back for the poll interval. More than
//! [`MAX_ROUND_TRANSIENTS`] transient failures cut the round short, since
//! that many points at an outage rather than a bad event; the events not
//! reached are released at commit and the consumer waits before the next
//! round. A failed acknowledgement or commit rolls back the whole round and
//! every event in it is claimed again. Each event's result row is claimed
//! on the same transaction, so a lost round leaves no result behind and an
//! event is still processed at most once per pipeline version.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use systemprompt_events::services::durable::DeliveryBatch;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::BRAIN_CONSUMER;

use super::BrainConsumer;
use super::handle::Handled;
use crate::error::BrainResult;
use crate::pipeline::{BatchContext, metrics};

pub const MAX_ROUND_TRANSIENTS: usize = 3;

#[derive(Default)]
struct Round {
    acknowledged: usize,
    retried: Vec<EventOutboxId>,
    wrote: bool,
    cut_short: bool,
}

impl BrainConsumer {
    pub(super) async fn drain_round(&self) -> BrainResult<bool> {
        let started = Instant::now();
        let held = self.round_state.held();
        let limit = i64::try_from(self.batch()).unwrap_or(i64::MAX);
        let claimed = self
            .outbox
            .claim_batch(BRAIN_CONSUMER, limit, &held)
            .await?;
        self.round_state.ran();
        let Some(mut batch) = claimed else {
            return Ok(false);
        };
        let ctx = match BatchContext::load(batch.connection()).await {
            Ok(ctx) => ctx,
            Err(error) => {
                release(batch).await;
                return Err(error);
            },
        };
        let events = batch.len();
        let round = match self.process_batch(&mut batch, &ctx).await {
            Ok(round) => round,
            Err(error) => {
                release(batch).await;
                return Err(error);
            },
        };
        batch.commit().await?;
        self.round_state.hold(round.retried, self.poll());
        metrics::batch_seconds(started.elapsed().as_secs_f64());
        metrics::round_events(events);
        if round.wrote
            && self.round_state.gauges_due()
            && let Err(error) = self.pipeline.refresh_store_gauges().await
        {
            tracing::warn!(error = %error, "Brain store gauges could not be refreshed");
        }
        Ok(round.acknowledged > 0 && !round.cut_short)
    }

    async fn process_batch(
        &self,
        batch: &mut DeliveryBatch,
        ctx: &BatchContext,
    ) -> BrainResult<Round> {
        let mut round = Round::default();
        for (id, raw) in batch.entries() {
            if round.retried.len() > MAX_ROUND_TRANSIENTS {
                round.cut_short = true;
                tracing::warn!(
                    transients = round.retried.len(),
                    "Brain drain round stopped early after repeated transient failures"
                );
                break;
            }
            match self.handle(batch.connection(), &id, &raw, ctx).await {
                Handled::Done { wrote } => {
                    batch.acknowledge(&id).await?;
                    round.acknowledged += 1;
                    round.wrote |= wrote;
                },
                Handled::Retry(error) => {
                    self.record_retry(batch.connection(), &id, ctx.version(), &error)
                        .await;
                    round.retried.push(id);
                },
            }
        }
        Ok(round)
    }

    fn poll(&self) -> Duration {
        Duration::from_secs(u64::from(self.settings.poll_secs.max(1)))
    }
}

async fn release(batch: DeliveryBatch) {
    if let Err(error) = batch.rollback().await {
        tracing::error!(error = %error, "Brain drain round rollback failed");
    }
}
