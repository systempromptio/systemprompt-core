//! The derived indexes a stored fact keeps current: its working-set rows,
//! its subject links, its aliases and its weighted search vector.
//!
//! [`reindex_facts`] is the one code path: a set of fact ids in, every
//! index written set-based, one statement per index over the whole set
//! (eight statements for a set of any size). It reads the facts, their
//! kind declarations and the subjects' display names in one query and the
//! synonyms document is passed in, loaded once by the caller, so a console
//! edit, a captured candidate and the projection drain index identically.
//! A fact that no longer exists is a no-op: its rows cascade with the
//! delete.
//!
//! [`reindex_fact`] is that path for one id, run on the write connection
//! right after the fact is inserted, revised, merged or has its fields
//! edited ([`Reindex::Now`]). The consumer round instead defers
//! ([`Reindex::Deferred`]): each event's writes skip the indexes, the
//! round collects the fact ids it landed on in a [`ReindexBatch`], and
//! one [`reindex_facts`] call after the per-event loop indexes them all on
//! the round's transaction.
//!
//! [`refresh_projection`] is the same path for a batch of queued keys: the
//! `brain_fact_index` projection an upgrade migration queues every stored
//! fact into and the consumer sweep drains. It also floors `observed_at`
//! to `created_at`, the one stamp a row that predates the column needs.
//! When the set statement fails for the batch, the batch is rolled back
//! to a savepoint and reindexed key by key, each under its own; a key
//! that fails alone is parked in the queue past its retry budget
//! ([`systemprompt_database::projection::park`]) and the rest of the
//! batch commits, so one refused row never strands the keys beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod aliases;
mod batch;
mod rows;

use sqlx::{Acquire, PgConnection};
use systemprompt_database::projection;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SynonymsDoc;
use systemprompt_traits::RepositoryError;

pub use aliases::{AliasInput, derive_aliases};
pub use batch::{ReindexReport, STATEMENTS_PER_SET, reindex_facts};
pub use rows::{AliasOwner, AliasRows, MAX_ALIAS_NORM_LEN, ReindexBatch};

use crate::repository::SynonymsRepository;

pub const BRAIN_FACT_INDEX: &str = "brain_fact_index";

/// When a write refreshes the derived indexes of the fact it lands on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reindex {
    #[default]
    Now,
    Deferred,
}

pub async fn reindex_fact(
    conn: &mut PgConnection,
    id: &BrainFactId,
) -> Result<(), RepositoryError> {
    let synonyms = SynonymsRepository::load(&mut *conn).await?;
    reindex_facts(conn, std::slice::from_ref(id), &synonyms).await?;
    Ok(())
}

pub async fn refresh_projection(
    conn: &mut PgConnection,
    keys: Vec<String>,
) -> Result<u64, RepositoryError> {
    sqlx::query!(
        "UPDATE brain_facts SET observed_at = created_at
         WHERE id = ANY($1) AND observed_at > created_at",
        &keys
    )
    .execute(&mut *conn)
    .await?;
    let ids: Vec<BrainFactId> = keys.into_iter().map(BrainFactId::new).collect();
    let synonyms = SynonymsRepository::load(&mut *conn).await?;
    match reindex_in_savepoint(&mut *conn, &ids, &synonyms).await {
        Ok(report) => return Ok(report.facts),
        Err(error) => tracing::warn!(
            keys = ids.len(),
            error = %error,
            "Brain projection batch failed as a set; reindexing key by key"
        ),
    }
    let mut refreshed = 0;
    for id in &ids {
        match reindex_in_savepoint(&mut *conn, std::slice::from_ref(id), &synonyms).await {
            Ok(report) => refreshed += report.facts,
            Err(error) => {
                tracing::error!(fact = %id, error = %error, "Brain fact cannot be indexed; parked");
                projection::park(&mut *conn, BRAIN_FACT_INDEX, id.as_str()).await?;
            },
        }
    }
    Ok(refreshed)
}

async fn reindex_in_savepoint(
    conn: &mut PgConnection,
    ids: &[BrainFactId],
    synonyms: &SynonymsDoc,
) -> Result<ReindexReport, RepositoryError> {
    // Why: Postgres — a failed statement aborts the transaction until a
    // ROLLBACK TO SAVEPOINT; without one the batch could only fail whole.
    let mut savepoint = conn.begin().await?;
    match reindex_facts(&mut savepoint, ids, synonyms).await {
        Ok(report) => {
            savepoint.commit().await?;
            Ok(report)
        },
        Err(error) => {
            savepoint.rollback().await?;
            Err(error)
        },
    }
}
