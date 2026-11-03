//! The set statements that close a drain round, and the fallback that
//! keeps one bad event from holding the round.
//!
//! The ledger finish, the unclaim of the events the round did not reach,
//! and the set-based reindex of every fact the round landed on each run
//! once over the whole round. The reindex runs under its own savepoint:
//! when any of its statements fails (a row some index constraint
//! refuses, whatever the constraint), the savepoint is rolled back, which
//! undoes only the index writes, never the per-event writes already
//! released into the round's transaction, and the facts are reindexed
//! again event by event, each under a savepoint of its own. The event
//! whose facts cannot be indexed is recorded as a permanent failure
//! (its attempt row and its result row `failed`, exactly as a per-event
//! permanent failure is recorded) and stays acknowledged; every other
//! event keeps its indexes. The round therefore commits, the attempt is
//! counted, and the extraction spend the round recorded is kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, PgConnection};
use systemprompt_identifiers::{BrainFactId, EventOutboxId};
use systemprompt_models::brain::{BrainEvent, SynonymsDoc};

use super::drain::Round;
use super::handle::{Failed, close_result, fail};
use super::{BrainConsumer, Counters};
use crate::error::{BrainError, BrainResult};
use crate::index::{ReindexReport, reindex_facts};
use crate::models::EventOutcome;
use crate::pipeline::{BatchContext, metrics};
use crate::repository::{EventResultRepository, FailureRepository};

const REINDEX_STAGE: &str = "reindex";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Landed {
    pub(super) id: EventOutboxId,
    pub(super) facts: Vec<BrainFactId>,
}

struct Poisoned<'a> {
    event: &'a Landed,
    kind: &'a str,
    error: &'a BrainError,
}

impl BrainConsumer {
    pub(super) async fn settle(
        &self,
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
        if round.reindex.is_empty() {
            return Ok(());
        }
        let as_set = reindex_in_savepoint(conn, round.reindex.ids(), ctx.synonyms()).await;
        round.reindexed = match as_set {
            Ok(report) => report,
            Err(error) => {
                tracing::warn!(
                    facts = round.reindex.len(),
                    error = %error,
                    cause = ?std::error::Error::source(&error),
                    "Brain round reindex failed as a set; reindexing event by event"
                );
                self.reindex_each(conn, ctx, decodable, &round.landed)
                    .await?
            },
        };
        Ok(())
    }

    async fn reindex_each(
        &self,
        conn: &mut PgConnection,
        ctx: &BatchContext,
        decodable: &[(EventOutboxId, BrainEvent)],
        landed: &[Landed],
    ) -> BrainResult<ReindexReport> {
        let mut total = ReindexReport::default();
        for event in landed.iter().filter(|event| !event.facts.is_empty()) {
            match reindex_in_savepoint(conn, &event.facts, ctx.synonyms()).await {
                Ok(report) => {
                    total.facts += report.facts;
                    total.aliases += report.aliases;
                    total.statements += report.statements;
                },
                Err(error) => {
                    let kind = kind_of(decodable, &event.id);
                    self.poison(
                        conn,
                        ctx,
                        Poisoned {
                            event,
                            kind,
                            error: &error,
                        },
                    )
                    .await;
                },
            }
        }
        Ok(total)
    }

    async fn poison(&self, conn: &mut PgConnection, ctx: &BatchContext, poisoned: Poisoned<'_>) {
        let Poisoned { event, kind, error } = poisoned;
        metrics::event("failed");
        tracing::error!(
            outbox_id = %event.id,
            kind,
            facts = ?event.facts,
            error = %error,
            cause = ?std::error::Error::source(error),
            "Brain event's facts cannot be indexed; recorded as failed"
        );
        let earlier = match FailureRepository::count_for_outbox(&mut *conn, &event.id).await {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %event.id, error = %count_error, "Brain event attempts could not be read");
                0
            },
        };
        let failed = Failed {
            id: &event.id,
            version: ctx.version(),
            stage: REINDEX_STAGE,
            reason: reason_of(error),
            attempt: earlier + 1,
        };
        fail(&mut *conn, &failed).await;
        close_result(conn, &failed, kind, EventOutcome::Failed).await;
        Counters::bump(&self.counters.failed);
    }
}

async fn reindex_in_savepoint(
    conn: &mut PgConnection,
    ids: &[BrainFactId],
    synonyms: &SynonymsDoc,
) -> BrainResult<ReindexReport> {
    // Why: Postgres — a failed statement aborts the transaction until a
    // ROLLBACK TO SAVEPOINT; the savepoint taken here is what lets the
    // round go on after the set statement fails.
    let mut savepoint = conn.begin().await?;
    match reindex_facts(&mut savepoint, ids, synonyms).await {
        Ok(report) => {
            savepoint.commit().await?;
            Ok(report)
        },
        Err(error) => {
            savepoint.rollback().await?;
            Err(BrainError::Repository(error))
        },
    }
}

fn kind_of<'a>(decodable: &'a [(EventOutboxId, BrainEvent)], id: &EventOutboxId) -> &'a str {
    decodable
        .iter()
        .find(|(known, _)| known == id)
        .map_or("", |(_, event)| event.kind.as_str())
}

fn reason_of(error: &BrainError) -> String {
    let mut parts = vec![error.to_string()];
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        parts.push(next.to_string());
        cause = next.source();
    }
    parts.join(": ")
}
