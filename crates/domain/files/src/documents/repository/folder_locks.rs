//! The folder tree lock every reshape of a tree takes: a transaction-scoped
//! advisory lock on the root of each tree a statement touches.
//!
//! The roots are read without locking, then all of them are locked in sorted
//! order in one pass, so two lockers never wait on each other in opposite
//! orders. A tree moved under another root between the read and the locks
//! leaves a root unlocked; the transaction is then rolled back and the whole
//! pass retried a bounded number of times rather than taking the new root out
//! of order.
//!
//! Every tree-locked transaction takes `ROW EXCLUSIVE` on
//! `document_path_changes` as its first statement, before any row lock or
//! write assigns it a transaction id, so the Drive feed horizon sees every
//! reshape that can still land a path change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use sqlx::{PgPool, Postgres, Transaction};
use systemprompt_traits::RepositoryError;

use super::rows::RepoResult;

const LOCK_ATTEMPTS: usize = 5;

async fn roots_of(
    tx: &mut Transaction<'static, Postgres>,
    folders: &[String],
) -> RepoResult<BTreeSet<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT COALESCE(ancestor_ids[1], id)::text AS "root!"
           FROM document_folders WHERE id = ANY($1::text[]::uuid[])"#,
        folders,
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect())
}

pub(super) async fn begin_tree_locked(
    pool: &PgPool,
    folders: &[&str],
) -> RepoResult<Transaction<'static, Postgres>> {
    let ids: Vec<String> = folders.iter().map(|id| (*id).to_owned()).collect();
    for _ in 0..LOCK_ATTEMPTS {
        let mut tx = pool.begin().await?;
        sqlx::query!("LOCK TABLE document_path_changes IN ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        if ids.is_empty() {
            return Ok(tx);
        }
        let roots = roots_of(&mut tx, &ids).await?;
        for root in &roots {
            sqlx::query_scalar!(
                r#"SELECT true AS "locked!" FROM pg_advisory_xact_lock(hashtextextended($1, 0))"#,
                root,
            )
            .fetch_one(&mut *tx)
            .await?;
        }
        if roots_of(&mut tx, &ids).await?.is_subset(&roots) {
            return Ok(tx);
        }
        tx.rollback().await?;
    }
    Err(RepositoryError::conflict(
        "document folder",
        folders.join(","),
        "the folder tree kept moving while its lock was taken",
    ))
}
