//! One pass of a drain round over its claimed events, under one savepoint
//! taken before the first event runs.
//!
//! Each event runs in its own nested savepoint through the pipeline; the
//! ledger is then finished for the events that finished and unclaimed for
//! the ones that did not, and with the indexes deferred every fact the
//! pass landed on is reindexed as a set. When that set statement fails
//! the pass savepoint is rolled back whole: every per-event write of the
//! pass (facts, revisions, disputes, notifications, attempt rows, the
//! ledger finish) goes with it, the transaction is live again, and the
//! caller runs the same events a second time with the indexes written per
//! event, so the one whose rows a constraint refuses fails inside its own
//! savepoint, is recorded the way any per-event failure is, and leaves no
//! fact behind. The claims the round made before the savepoint survive
//! both passes; so does what the round cache remembered of the first
//! pass's model calls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, PgConnection};
use systemprompt_identifiers::EventOutboxId;
use systemprompt_traits::RepositoryError;

use super::decode::Decoded;
use super::handle::{Handled, PassTally, Retry};
use super::{BrainConsumer, MAX_ROUND_TRANSIENTS};
use crate::error::BrainResult;
use crate::index::{Reindex, ReindexBatch, ReindexReport, reindex_facts};
use crate::models::EventResult;
use crate::pipeline::{BatchContext, ProcessParams, RoundCache};
use crate::repository::EventResultRepository;

#[derive(Default)]
pub(super) struct Round {
    pub(super) done: Vec<EventOutboxId>,
    pub(super) retried: Vec<EventOutboxId>,
    pub(super) results: Vec<EventResult>,
    pub(super) wrote: bool,
    pub(super) cut_short: bool,
    pub(super) reindex: ReindexBatch,
    pub(super) reindexed: ReindexReport,
    pub(super) tally: PassTally,
}

pub(super) enum Pass {
    Settled(Round),
    Unindexable(RepositoryError),
}

pub(super) struct PassInput<'a> {
    pub(super) ctx: &'a BatchContext,
    pub(super) cache: &'a RoundCache,
    pub(super) events: Vec<Decoded>,
    pub(super) reindex: Reindex,
}

impl BrainConsumer {
    pub(super) async fn run_pass(
        &self,
        conn: &mut PgConnection,
        input: PassInput<'_>,
    ) -> BrainResult<Pass> {
        let PassInput {
            ctx,
            cache,
            events,
            reindex,
        } = input;
        let ids: Vec<EventOutboxId> = events.iter().map(|event| event.id.clone()).collect();
        let params = ProcessParams {
            ctx,
            reindex,
            round: Some(cache),
        };
        // Why: Postgres — a failed statement aborts the transaction until a
        // ROLLBACK TO SAVEPOINT; this savepoint is what lets the round
        // discard a pass and run again on the same transaction.
        let mut savepoint = conn.begin().await?;
        let mut round = Round::default();
        for event in events {
            if round.retried.len() > MAX_ROUND_TRANSIENTS {
                round.cut_short = true;
                tracing::warn!(
                    transients = round.retried.len(),
                    "Brain drain round stopped early after repeated transient failures"
                );
                break;
            }
            let id = event.id.clone();
            match self
                .handle(&mut savepoint, event, &params, &mut round.tally)
                .await?
            {
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
                    Self::record_retry(
                        &mut savepoint,
                        Retry {
                            id: &id,
                            version: ctx.version(),
                            error: &error,
                        },
                        &mut round.tally,
                    )
                    .await?;
                    round.retried.push(id);
                },
            }
        }
        finish_ledger(&mut savepoint, ctx, &ids, &round).await?;
        if reindex == Reindex::Deferred && !round.reindex.is_empty() {
            match reindex_facts(&mut savepoint, round.reindex.ids(), ctx.synonyms()).await {
                Ok(report) => round.reindexed = report,
                Err(error) => {
                    savepoint.rollback().await?;
                    return Ok(Pass::Unindexable(error));
                },
            }
        }
        savepoint.commit().await?;
        Ok(Pass::Settled(round))
    }
}

async fn finish_ledger(
    conn: &mut PgConnection,
    ctx: &BatchContext,
    ids: &[EventOutboxId],
    round: &Round,
) -> BrainResult<()> {
    if !round.results.is_empty() {
        EventResultRepository::finish_many(&mut *conn, ctx.version(), &round.results).await?;
    }
    let unfinished: Vec<EventOutboxId> = ids
        .iter()
        .filter(|id| !round.done.contains(id))
        .cloned()
        .collect();
    if !unfinished.is_empty() {
        EventResultRepository::unclaim_many(conn, ctx.version(), &unfinished).await?;
    }
    Ok(())
}
