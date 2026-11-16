//! What the live database holds that no registered extension declares.
//!
//! A crate that is deleted takes its schema files with it and leaves its
//! tables, its `extension_migrations` ledger and its scheduled job behind
//! on every installed database; a drop migration that was baseline-stamped
//! rather than executed leaves the same residue with a ledger that says
//! there is nothing left to run. This audit runs after every schema install and
//! in `infra db doctor`: a live base table in a namespace the extensions
//! declare into that no extension's `CREATE TABLE` names, and a ledger whose
//! extension id no registered extension carries. It reads; the fix is a
//! migration. A table or ledger that the ownership records
//! (`schema_table_owners`) give to a member outside the extension
//! ids passed in is a member not in this composition (disabled by the
//! profile): it is reported apart and is never residue to drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use serde::Serialize;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use super::extension::ownership::recorded_table_owners;
use super::prepare::{PreparedSchema, prepare_extension_schema};
use crate::lifecycle::schema_member::SchemaMember;
use crate::services::DatabaseProvider;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UndeclaredTable {
    pub schema: String,
    pub table: String,
    pub live_rows: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrphanMigrationLedger {
    pub extension_id: ExtensionId,
    pub rows: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AbsentMemberTable {
    pub owner: ExtensionId,
    pub schema: String,
    pub table: String,
    pub live_rows: i64,
}

/// What the live schema holds beyond the passed members' declarations.
///
/// `undeclared_tables` and `orphan_migration_ledgers` are residue a drop
/// migration removes; `absent_member_tables` and `absent_member_ledgers`
/// belong to a recorded member outside this composition and are kept.
/// [`Self::is_empty`] judges the residue only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SchemaResidue {
    pub undeclared_tables: Vec<UndeclaredTable>,
    pub orphan_migration_ledgers: Vec<OrphanMigrationLedger>,
    pub absent_member_tables: Vec<AbsentMemberTable>,
    pub absent_member_ledgers: Vec<OrphanMigrationLedger>,
}

impl SchemaResidue {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.undeclared_tables.is_empty() && self.orphan_migration_ledgers.is_empty()
    }
}

pub async fn audit_schema_residue(
    db: &dyn DatabaseProvider,
    owned: &[String],
    extension_ids: &[ExtensionId],
) -> Result<SchemaResidue, LoaderError> {
    let declared: BTreeSet<String> = owned.iter().map(|t| qualify(t)).collect();
    let namespaces: BTreeSet<String> = declared
        .iter()
        .filter_map(|t| t.split_once('.').map(|(schema, _)| schema.to_owned()))
        .collect();
    let registered: BTreeSet<&ExtensionId> = extension_ids.iter().collect();
    let owners = recorded_table_owners(db).await?;
    let mut residue = SchemaResidue::default();
    for (schema, table, live_rows) in live_tables(db).await? {
        let qualified = format!("{schema}.{table}");
        if !namespaces.contains(&schema) || declared.contains(&qualified) {
            continue;
        }
        match owners.owner_of(&qualified) {
            Some(owner) if !registered.contains(owner) => {
                residue.absent_member_tables.push(AbsentMemberTable {
                    owner: owner.clone(),
                    schema,
                    table,
                    live_rows,
                });
            },
            _ => residue.undeclared_tables.push(UndeclaredTable {
                schema,
                table,
                live_rows,
            }),
        }
    }
    for (extension_id, rows) in migration_ledgers(db).await? {
        if registered.contains(&extension_id) {
            continue;
        }
        let ledger = OrphanMigrationLedger { extension_id, rows };
        if owners.is_owner(&ledger.extension_id) {
            residue.absent_member_ledgers.push(ledger);
        } else {
            residue.orphan_migration_ledgers.push(ledger);
        }
    }
    Ok(residue)
}

// Why: a bigint reaches the JSON row as a number or, past 2^53, as a
// string; both spellings are a count.
// JSON: `DatabaseProvider` runtime row cell — catalog queries return JSON rows.
fn as_count(value: &serde_json::Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

pub(super) fn qualify(table: &str) -> String {
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
        .map_err(|e| LoaderError::SchemaInstallationStepFailed {
            extension: ExtensionId::new("schema-residue"),
            context: "could not list live tables".to_owned(),
            source: Box::new(e),
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

// Why: an ahead extension is held back from the install but still owns its
// tables and ledger; leaving it out of the audit would report them as residue.
pub(super) async fn residue_including(
    db: &dyn DatabaseProvider,
    prepared: &[PreparedSchema],
    held_back: &[std::sync::Arc<dyn SchemaMember>],
) -> Result<SchemaResidue, LoaderError> {
    let mut held: Vec<PreparedSchema> = Vec::with_capacity(held_back.len());
    for ext in held_back {
        held.push(prepare_extension_schema(ext.as_ref())?);
    }
    let all = || prepared.iter().chain(&held);
    let owned: Vec<String> = all().flat_map(|p| p.owned_tables.clone()).collect();
    let ids: Vec<ExtensionId> = all().map(|p| p.extension_id.clone()).collect();
    audit_schema_residue(db, &owned, &ids).await
}

async fn migration_ledgers(
    db: &dyn DatabaseProvider,
) -> Result<Vec<(ExtensionId, i64)>, LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT extension_id, COUNT(*)::bigint AS rows \
              FROM extension_migrations GROUP BY extension_id ORDER BY extension_id",
            &[],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationStepFailed {
            extension: ExtensionId::new("schema-residue"),
            context: "could not read extension_migrations".to_owned(),
            source: Box::new(e),
        })?;
    Ok(result
        .rows
        .iter()
        .filter_map(|row| {
            let id = ExtensionId::new(row.get("extension_id")?.as_str()?);
            Some((id, row.get("rows").map_or(0, as_count)))
        })
        .collect())
}
