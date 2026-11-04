//! One drain round: claim up to a batch of events on one transaction,
//! decode them, preload the round cache (set claim of the result ledger,
//! alias resolution and dedupe rows of every entered fact), run the
//! events as one pass ([`super::pass`]) with their derived indexes
//! deferred and their reads served from the cache, acknowledge, and
//! commit once.
//!
//! A transient failure no longer ends the round: its savepoint is rolled
//! back, its attempt is recorded, its row stays unacknowledged and the
//! consumer holds it back for the poll interval. More than
//! [`MAX_ROUND_TRANSIENTS`] transient failures cut the round short, since
//! that many points at an outage rather than a bad event; the events not
//! reached are released at commit and the consumer waits before the next
//! round. The ledger claims of every event the round did not finish are
//! deleted before commit, so a retried event is claimed again by the round
//! that retries it. A failed ledger statement, a failed attempt record or
//! a failed commit rolls back the whole round and every event in it is
//! claimed again. Each event's result row is claimed on the same
//! transaction, so a lost round leaves no result behind and an event is
//! still processed at most once per pipeline version.
//!
//! The set statements after the loop ([`crate::index::reindex_facts`], the
//! ledger finish, the acknowledgement) replace what each event used to run
//! on its own. The reindex is the one with head-of-line risk: a row one
//! index constraint refuses fails the statement for every fact in the
//! round, and a round that rolled back for it would record no attempt and
//! retry forever. The guard against that, for any constraint, is the
//! second pass: the first pass is rolled back whole (so no fact of the
//! refused event survives), the cache stops serving the rows that rollback
//! removed, and the same events run again with every index written per
//! event, where the refused one fails inside its own savepoint as a
//! permanent failure ([`crate::error::BrainError::is_transient`] is false
//! for a constraint violation) or, for a transient cause, is held and
//! retried like any other. Model calls are not repeated: the round
//! remembers each first-pass extraction and the second pass re-records its
//! spend instead. The alias length check is additionally filtered before
//! the set statement ([`crate::index::MAX_ALIAS_NORM_LEN`]), so an
//! overlong alias never costs a second pass. Within a round an alias a
//! fact contributes is not yet resolvable by a later event of the same
//! round; it is from the next round on. The acknowledgement is one
//! statement when every event finished and one per finished event
//! otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use systemprompt_events::services::durable::DeliveryBatch;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::{BRAIN_CONSUMER, BrainEvent};

use super::decode::decode_all;
use super::pass::{Pass, PassInput, Round};
use super::{BrainConsumer, Counters};
use crate::error::{BrainError, BrainResult};
use crate::index::Reindex;
use crate::pipeline::{BatchContext, RoundCache, metrics};

pub const MAX_ROUND_TRANSIENTS: usize = 3;

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
        Counters::add(&self.counters.processed, round.tally.processed);
        Counters::add(&self.counters.retried, round.tally.retried);
        Counters::add(&self.counters.failed, round.tally.failed);
        Counters::add(&self.counters.abandoned, round.tally.abandoned);
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
        let entries = batch.entries();
        let decodable: Vec<(EventOutboxId, BrainEvent)> = decode_all(entries.clone())
            .into_iter()
            .filter_map(|d| d.event.ok().map(|event| (d.id, event)))
            .collect();
        let cache = RoundCache::load(batch.connection(), &ctx, &decodable).await?;
        let deferred = PassInput {
            ctx: &ctx,
            cache: &cache,
            events: decode_all(entries.clone()),
            reindex: Reindex::Deferred,
        };
        let error = match self.run_pass(batch.connection(), deferred).await? {
            Pass::Settled(round) => return Ok(round),
            Pass::Unindexable(error) => error,
        };
        tracing::warn!(
            events = entries.len(),
            error = %error,
            cause = ?std::error::Error::source(&error),
            "Brain round reindex failed as a set; running the round again per event"
        );
        metrics::round_second_pass();
        cache.forget_reads();
        let per_event = PassInput {
            ctx: &ctx,
            cache: &cache,
            events: decode_all(entries),
            reindex: Reindex::Now,
        };
        match self.run_pass(batch.connection(), per_event).await? {
            Pass::Settled(round) => Ok(round),
            Pass::Unindexable(error) => Err(BrainError::Repository(error)),
        }
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
