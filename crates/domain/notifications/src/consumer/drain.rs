//! One drain round: claim up to a batch of facts on one transaction,
//! process each inside its own savepoint, acknowledge each finished fact in
//! that transaction, commit once, then deliver the `pending` rows the
//! committed facts left.
//!
//! A transient failure no longer ends the round: its savepoint is rolled
//! back, its attempt is recorded, its row stays unacknowledged and the
//! consumer holds it back for the poll interval. More than
//! [`MAX_ROUND_TRANSIENTS`] transient failures cut the round short; the
//! facts not reached are released at commit and the consumer waits before
//! the next round. A failed acknowledgement or commit rolls back the whole
//! round, nothing from it is delivered, and every fact in it is claimed
//! again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use systemprompt_events::services::durable::DeliveryBatch;
use systemprompt_identifiers::{EventOutboxId, NotificationId};
use systemprompt_models::notifications::NOTIFICATIONS_CONSUMER;

use super::NotificationsConsumer;
use super::handle::Handled;
use crate::error::NotificationsResult;
use crate::pipeline::{BatchContext, metrics};

pub const MAX_ROUND_TRANSIENTS: usize = 3;

#[derive(Default)]
struct Round {
    acknowledged: usize,
    retried: Vec<EventOutboxId>,
    pending: Vec<NotificationId>,
    cut_short: bool,
}

impl NotificationsConsumer {
    pub(super) async fn drain_round(&self) -> NotificationsResult<bool> {
        let started = Instant::now();
        let held = self.hold.held();
        let limit = i64::try_from(self.batch()).unwrap_or(i64::MAX);
        let Some(mut batch) = self
            .outbox
            .claim_batch(NOTIFICATIONS_CONSUMER, limit, &held)
            .await?
        else {
            return Ok(false);
        };
        let mut ctx = match BatchContext::load(batch.connection()).await {
            Ok(ctx) => ctx,
            Err(error) => {
                release(batch).await;
                return Err(error);
            },
        };
        let round = match self.process_batch(&mut batch, &mut ctx).await {
            Ok(round) => round,
            Err(error) => {
                metrics::fact("ack_failed");
                release(batch).await;
                return Err(error);
            },
        };
        batch.commit().await?;
        self.hold.hold(round.retried, self.poll());
        if !round.pending.is_empty() {
            self.pipeline.deliver(&round.pending).await;
        }
        metrics::batch_seconds(started.elapsed().as_secs_f64());
        self.report_backlog().await;
        Ok(round.acknowledged > 0 && !round.cut_short)
    }

    async fn process_batch(
        &self,
        batch: &mut DeliveryBatch,
        ctx: &mut BatchContext,
    ) -> NotificationsResult<Round> {
        let mut round = Round::default();
        for (id, raw) in batch.entries() {
            if round.retried.len() > MAX_ROUND_TRANSIENTS {
                round.cut_short = true;
                tracing::warn!(
                    transients = round.retried.len(),
                    "Notification drain round stopped early after repeated transient failures"
                );
                break;
            }
            match self.handle(batch.connection(), &id, &raw, ctx).await {
                Handled::Done(pending) => {
                    batch.acknowledge(&id).await?;
                    round.acknowledged += 1;
                    round.pending.extend(pending);
                },
                Handled::Retry(error) => {
                    self.record_retry(batch.connection(), &id, &error).await;
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
        tracing::error!(error = %error, "Notification drain round rollback failed");
    }
}
