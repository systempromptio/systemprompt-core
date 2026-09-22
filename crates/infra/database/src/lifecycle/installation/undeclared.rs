//! What the live database holds that no registered extension declares.
//!
//! A crate that is deleted takes its schema files with it and leaves its
//! tables, its `extension_migrations` ledger and its scheduled job behind
//! on every installed database; a drop migration that was baseline-stamped
//! rather than executed leaves the same residue with a ledger that says
//! there is nothing left to run. The 2026-09-22 production audit found 39
//! such tables and a dead `evaluation` ledger, none reported anywhere. This
//! audit runs after every schema install and in `infra db doctor`: a live
//! base table in a namespace the extensions declare into that no
//! extension's `CREATE TABLE` names, and a ledger whose extension id no
//! registered extension carries. It reads; the fix is a migration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use serde::Serialize;
use systemprompt_extension::LoaderError;

use crate::services::DatabaseProvider;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UndeclaredTable {
    pub schema: String,
    pub table: String,
    pub live_rows: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrphanMigrationLedger {
    pub extension_id: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SchemaResidue {
    pub undeclared_tables: Vec<UndeclaredTable>,
    pub orphan_migration_ledgers: Vec<OrphanMigrationLedger>,
}

impl SchemaResidue {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.undeclared_tables.is_empty() && self.orphan_migration_ledgers.is_empty()
    }
}

/// `owned` is every `schema.table` (or bare `table`, meaning `public`) the
/// registered extensions declare; `extension_ids` every registered id.
pub async fn audit_schema_residue(
    db: &dyn DatabaseProvider,
    owned: &[String],
    extension_ids: &[String],
) -> Result<SchemaResidue, LoaderError> {
    let declared: BTreeSet<String> = owned.iter().map(|t| qualify(t)).collect();
    let namespaces: BTreeSet<String> = declared
        .iter()
        .filter_map(|t| t.split_once('.').map(|(schema, _)| schema.to_owned()))
        .collect();
    let mut residue = SchemaResidue::default();
    for (schema, table, live_rows) in live_tables(db).await? {
        let qualified = format!("{schema}.{table}");
        if namespaces.contains(&schema) && !declared.contains(&qualified) {
            residue.undeclared_tables.push(UndeclaredTable {
                schema,
                table,
                live_rows,
            });
        }
    }
    let registered: BTreeSet<&str> = extension_ids.iter().map(String::as_str).collect();
    for (extension_id, rows) in migration_ledgers(db).await? {
        if !registered.contains(extension_id.as_str()) {
            residue
                .orphan_migration_ledgers
                .push(OrphanMigrationLedger { extension_id, rows });
        }
    }
    Ok(residue)
}

// Why: a bigint reaches the JSON row as a number or, past 2^53, as a
// string; both spellings are a count.
fn as_count(value: &serde_json::Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

fn qualify(table: &str) -> String {
    if table.contains('.') {
        table.to_owned()
    } else {
        format!("public.{table}")
    }
}

// Why: `extension_migrations` itself is core's ledger and `_sqlx_migrations`
// is sqlx's; neither is declared by an extension's CREATE TABLE.
const LEDGER_TABLES: [&str; 2] = ["extension_migrations", "_sqlx_migrations"];

async fn live_tables(db: &dyn DatabaseProvider) -> Result<Vec<(String, String, i64)>, LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT n.nspname AS schema, c.relname AS table, \
                     COALESCE(s.n_live_tup, 0)::bigint AS live_rows \
              FROM pg_class c \
              JOIN pg_namespace n ON n.oid = c.relnamespace \
              LEFT JOIN pg_stat_user_tables s ON s.relid = c.oid \
              WHERE c.relkind = 'r' \
                AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
                AND n.nspname NOT LIKE 'pg_toast%' \
              ORDER BY 1, 2",
            &[],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: "schema-residue".to_owned(),
            message: format!("could not list live tables: {e}"),
        })?;
    Ok(result
        .rows
        .iter()
        .filter_map(|row| {
            let schema = row.get("schema")?.as_str()?.to_owned();
            let table = row.get("table")?.as_str()?.to_owned();
            let live_rows = row.get("live_rows").map_or(0, as_count);
            (!LEDGER_TABLES.contains(&table.as_str())).then_some((schema, table, live_rows))
        })
        .collect())
}

async fn migration_ledgers(db: &dyn DatabaseProvider) -> Result<Vec<(String, i64)>, LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT extension_id, COUNT(*)::bigint AS rows \
              FROM extension_migrations GROUP BY extension_id ORDER BY extension_id",
            &[],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: "schema-residue".to_owned(),
            message: format!("could not read extension_migrations: {e}"),
        })?;
    Ok(result
        .rows
        .iter()
        .filter_map(|row| {
            let id = row.get("extension_id")?.as_str()?.to_owned();
            Some((id, row.get("rows").map_or(0, as_count)))
        })
        .collect())
}
