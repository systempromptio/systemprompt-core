//! One drain round: claim up to a batch of events on one transaction,
//! decode them, preload the round cache (set claim of the result ledger,
//! alias resolution and dedupe rows of every entered fact), process each
//! event inside its own savepoint with its derived indexes deferred and
//! its reads served from the cache, then finish the ledger, reindex every
//! fact the round landed on and acknowledge, each as one set statement,
//! and commit once.
//!
//! A transient failure no longer ends the round: its savepoint is rolled
//! back, its attempt is recorded, its row stays unacknowledged and the
//! consumer holds it back for the poll interval. More than
//! [`MAX_ROUND_TRANSIENTS`] transient failures cut the round short, since
//! that many points at an outage rather than a bad event; the events not
//! reached are released at commit and the consumer waits before the next
//! round. The ledger claims of every event the round did not finish are
//! deleted before commit, so a retried event is claimed again by the round
//! that retries it. A failed set statement or commit rolls back the whole
//! round and every event in it is claimed again. Each event's result row
//! is claimed on the same transaction, so a lost round leaves no result
//! behind and an event is still processed at most once per pipeline version.
//!
//! The set statements after the loop ([`crate::index::reindex_facts`], the
//! ledger finish, the acknowledgement) replace what each event used to run
//! on its own. The cost is head-of-line coupling: if one of them fails,
//! the round rolls back and every event in it is claimed again, their
//! savepoints and attempt records with it, so a fact whose index rows
//! cannot be written holds its round until the cause is fixed; the
//! per-event attempt limit does not apply because no attempt is recorded.
//! The one known constraint on those rows, the alias length check, is
//! filtered before the statement so it cannot trip it. Within a round an
//! alias a fact contributes is not yet resolvable by a later event of the
//! same round; it is from the next round on. The acknowledgement is one
//! statement when every event finished and one per finished event
//! otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use sqlx::PgConnection;
use systemprompt_events::services::durable::DeliveryBatch;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::{BRAIN_CONSUMER, BrainEvent};

use super::decode::decode_all;
use super::handle::Handled;
use super::{BrainConsumer, Counters};
use crate::error::BrainResult;
use crate::index::{Reindex, ReindexBatch, ReindexReport, reindex_facts};
use crate::models::EventResult;
use crate::pipeline::{BatchContext, ProcessParams, RoundCache, metrics};
use crate::repository::EventResultRepository;

pub const MAX_ROUND_TRANSIENTS: usize = 3;

#[derive(Default)]
struct Round {
    done: Vec<EventOutboxId>,
    retried: Vec<EventOutboxId>,
    results: Vec<EventResult>,
    wrote: bool,
    cut_short: bool,
    reindex: ReindexBatch,
    reindexed: ReindexReport,
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
        let events = batch.len();
        let round = match self.process_batch(&mut batch).await {
            Ok(round) => round,
            Err(error) => {
                release(batch).await;
                return Err(error);
            },
        };
        if round.done.len() == events {
            batch.acknowledge_all().await?;
        } else {
            for id in &round.done {
                batch.acknowledge(id).await?;
            }
            batch.commit().await?;
        }
        self.round_state.hold(round.retried, self.poll());
        Counters::add(&self.counters.reindexed, round.reindexed.facts);
        metrics::batch_seconds(started.elapsed().as_secs_f64());
        metrics::round_events(events);
        metrics::round_reindexed(round.reindexed.facts, round.reindexed.statements);
        tracing::debug!(
            events,
            acknowledged = round.done.len(),
            facts_reindexed = round.reindexed.facts,
            index_statements = round.reindexed.statements,
            "Brain drain round committed"
        );
        if round.wrote
            && self.round_state.gauges_due()
            && let Err(error) = self.pipeline.refresh_store_gauges().await
        {
            tracing::warn!(error = %error, "Brain store gauges could not be refreshed");
        }
        Ok(!round.done.is_empty() && !round.cut_short)
    }

    async fn process_batch(&self, batch: &mut DeliveryBatch) -> BrainResult<Round> {
        let ctx = BatchContext::load(batch.connection()).await?;
        let decoded = decode_all(batch.entries());
        let decodable: Vec<(EventOutboxId, BrainEvent)> = decoded
            .iter()
            .filter_map(|d| {
                d.event
                    .as_ref()
                    .ok()
                    .map(|event| (d.id.clone(), event.clone()))
            })
            .collect();
        let cache = RoundCache::load(batch.connection(), &ctx, &decodable).await?;
        let params = ProcessParams {
            ctx: &ctx,
            reindex: Reindex::Deferred,
            round: Some(&cache),
        };
        let mut round = Round::default();
        for event in decoded {
            if round.retried.len() > MAX_ROUND_TRANSIENTS {
                round.cut_short = true;
                tracing::warn!(
                    transients = round.retried.len(),
                    "Brain drain round stopped early after repeated transient failures"
                );
                break;
            }
            let id = event.id.clone();
            match self.handle(batch.connection(), event, &params).await {
                Handled::Done {
                    wrote,
                    facts,
                    result,
                } => {
                    round.done.push(id);
                    round.wrote |= wrote;
                    round.reindex.extend(facts);
                    round.results.extend(result);
                },
                Handled::Retry(error) => {
                    self.record_retry(batch.connection(), &id, ctx.version(), &error)
                        .await;
                    round.retried.push(id);
                },
            }
        }
        settle(batch.connection(), &ctx, &decodable, &mut round).await?;
        Ok(round)
    }

    fn poll(&self) -> Duration {
        Duration::from_secs(u64::from(self.settings.poll_secs.max(1)))
    }
}

async fn settle(
    conn: &mut PgConnection,
    ctx: &BatchContext,
    decodable: &[(EventOutboxId, BrainEvent)],
    round: &mut Round,
) -> BrainResult<()> {
    if !round.results.is_empty() {
        EventResultRepository::finish_many(&mut *conn, ctx.version(), &round.results).await?;
    }
    let unfinished: Vec<EventOutboxId> = decodable
        .iter()
        .map(|(id, _)| id.clone())
        .filter(|id| !round.done.contains(id))
        .collect();
    if !unfinished.is_empty() {
        EventResultRepository::unclaim_many(&mut *conn, ctx.version(), &unfinished).await?;
    }
    if !round.reindex.is_empty() {
        round.reindexed = reindex_facts(conn, round.reindex.ids(), ctx.synonyms()).await?;
    }
    Ok(())
}

async fn release(batch: DeliveryBatch) {
    if let Err(error) = batch.rollback().await {
        tracing::error!(error = %error, "Brain drain round rollback failed");
    }
}
