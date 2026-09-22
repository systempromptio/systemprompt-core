//! Baseline snapshot pages: one set-based statement per keyset page of a
//! source's reporting view, so the rebuild never holds a cursor across
//! transactions and never round-trips per row.
//!
//! `lock_sources` blocks every writer of every reporting source until the
//! caller's transaction ends; it is taken only for the cutoff fence, never
//! across a page. `write_snapshot_page` writes the next `limit` retained rows
//! after a key from the live view straight into the target. Reading live rows
//! under the projector lock is what keeps a privacy delivery made between
//! pages from being undone by a stale snapshot: the newest committed version
//! always wins, and anything newer still has a fact above the cutoff to
//! apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;

use super::{SOURCE_DEFINITIONS, SourceDefinition};
use crate::Result;

/// What one page wrote and where the next one starts.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SnapshotPage {
    pub last_key: Option<String>,
    pub fetched: i64,
    pub written: i64,
}

pub async fn lock_sources(connection: &mut PgConnection) -> Result<()> {
    let tables = SOURCE_DEFINITIONS
        .iter()
        .map(|definition| definition.table)
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "LOCK TABLE {tables} IN SHARE MODE"
    )))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn write_snapshot_page(
    connection: &mut PgConnection,
    definition: &SourceDefinition,
    after: Option<&str>,
    limit: i64,
) -> Result<SnapshotPage> {
    let assignments = definition
        .columns
        .iter()
        .filter(|column| **column != definition.key)
        .map(|column| format!("{column} = EXCLUDED.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "WITH page AS (
            SELECT entity_key, row FROM {view}
            WHERE $1::text IS NULL OR entity_key > $1
            ORDER BY entity_key LIMIT $2
        ), written AS (
            INSERT INTO {target}
            SELECT populated.* FROM page
            CROSS JOIN LATERAL jsonb_populate_record(NULL::{target}, page.row) AS populated
            WHERE reporting_row_retained($3, page.row)
            ON CONFLICT ({key}) DO UPDATE SET {assignments}
            RETURNING 1
        )
        SELECT MAX(entity_key) AS last_key, COUNT(*) AS fetched,
               (SELECT COUNT(*) FROM written) AS written
        FROM page",
        view = definition.view,
        target = definition.target,
        key = definition.key,
    );
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(after)
        .bind(limit)
        .bind(definition.table)
        .fetch_one(connection)
        .await?)
}
