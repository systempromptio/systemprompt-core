//! `db doctor` subcommand.
//!
//! Reconciles the live database schema against the tables and required columns
//! declared by registered extensions, reporting missing tables, missing
//! columns, live tables that no extension declares and migration ledgers of
//! extensions that no longer exist. Residue is a failure (exit 1): it is the
//! schema a deleted crate left behind and the signal that a drop migration is
//! due — the same audit boot runs after every install, as a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::services::schema_linter::created_table_names;
use systemprompt_database::{DbPool, SchemaMember, SchemaName, audit_schema_residue};
use systemprompt_extension::BuiltState;
use systemprompt_extension::cli::CommandError;
use systemprompt_identifiers::ExtensionId;

use systemprompt_extension::cli::CommandOutput;

mod report;

#[derive(Debug, Serialize)]
struct DoctorReport {
    undeclared_tables: Vec<String>,
    orphan_migration_ledgers: Vec<String>,
    absent_member_tables: Vec<String>,
    absent_member_ledgers: Vec<String>,
    missing_tables: Vec<String>,
    missing_columns: Vec<MissingColumn>,
    unparseable_schemas: Vec<UnparseableSchema>,
}

#[derive(Debug, Serialize)]
struct UnparseableSchema {
    extension: ExtensionId,
    error: String,
}

#[derive(Debug, Serialize)]
struct MissingColumn {
    extension: ExtensionId,
    table: String,
    column: String,
}

pub(super) async fn execute_doctor(db_pool: &DbPool, built: &BuiltState) -> Result<CommandOutput> {
    let schema_extensions = super::admin_migrate::install_set(built);
    let write_provider = db_pool.write();
    let (declared_columns, owner) = declared_columns(&schema_extensions);

    let live_tables = fetch_live_tables(write_provider).await?;
    let live_columns = fetch_live_columns(write_provider).await?;

    // Why: the residue audit parses every extension's CREATE TABLEs, so a
    // schema file registered without a table name still declares its
    // tables; the `schema.table` list above is only for required columns.
    let mut created_tables: Vec<String> = Vec::new();
    let mut unparseable: Vec<UnparseableSchema> = Vec::new();
    let mut ids: Vec<ExtensionId> = Vec::new();
    for ext in &schema_extensions {
        let ext_id = ExtensionId::new(ext.id());
        for schema in ext.schemas() {
            match created_table_names(&schema.sql) {
                Ok(names) => created_tables.extend(names),
                Err(error) => unparseable.push(UnparseableSchema {
                    extension: ext_id.clone(),
                    error: error.to_string(),
                }),
            }
        }
        ids.push(ext_id);
    }
    let residue =
        audit_schema_residue(write_provider, &created_tables, &ids, &SchemaName::public()).await?;
    let undeclared: Vec<String> = residue
        .undeclared_tables
        .iter()
        .map(|t| format!("{}.{} ({} rows)", t.schema, t.table, t.live_rows))
        .collect();

    let missing_tables: Vec<String> = declared_columns
        .keys()
        .filter(|t| !live_tables.contains(*t))
        .cloned()
        .collect();

    let missing_columns = missing_columns(&declared_columns, &live_columns, &owner);

    let unparseable_count = unparseable.len();
    let output = report::report_output(
        &undeclared,
        &residue,
        &missing_tables,
        &missing_columns,
        unparseable,
    );

    if unparseable_count > 0 {
        return Err(CommandError::new(format!(
            "{unparseable_count} declared schema(s) do not parse: the residue audit cannot \
             attribute their tables"
        ))
        .with_output(output)
        .into());
    }
    if !residue.is_empty() {
        return Err(CommandError::new(format!(
            "{} undeclared table(s) and {} orphan migration ledger(s): add a DROP TABLE migration",
            residue.undeclared_tables.len(),
            residue.orphan_migration_ledgers.len()
        ))
        .with_output(output)
        .into());
    }
    Ok(output)
}

type DeclaredColumns = BTreeMap<String, BTreeSet<String>>;

fn declared_columns(
    schema_extensions: &[Arc<dyn SchemaMember>],
) -> (DeclaredColumns, BTreeMap<String, ExtensionId>) {
    let mut declared: DeclaredColumns = BTreeMap::new();
    let mut owner: BTreeMap<String, ExtensionId> = BTreeMap::new();
    for ext in schema_extensions {
        let ext_id = ExtensionId::new(ext.id());
        for schema in ext.schemas() {
            let Some(table) = schema.table else {
                continue;
            };
            owner.entry(table.clone()).or_insert_with(|| ext_id.clone());
            let entry = declared.entry(table).or_default();
            for col in &schema.required_columns {
                entry.insert(col.clone());
            }
        }
    }
    (declared, owner)
}

fn missing_columns(
    declared: &DeclaredColumns,
    live_columns: &DeclaredColumns,
    owner: &BTreeMap<String, ExtensionId>,
) -> Vec<(ExtensionId, String, String)> {
    let mut missing = Vec::new();
    for (table, required) in declared {
        let (Some(live_cols), Some(owner_id)) = (live_columns.get(table), owner.get(table)) else {
            continue;
        };
        for col in required {
            if !live_cols.contains(col) {
                missing.push((owner_id.clone(), table.clone(), col.clone()));
            }
        }
    }
    missing
}

async fn fetch_live_tables(db: &dyn DatabaseProvider) -> Result<BTreeSet<String>> {
    let result = db
        .query_raw_with(
            &"SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND \
              table_type = 'BASE TABLE'",
            &[],
        )
        .await
        .context("Failed to list live tables")?;

    Ok(result
        .rows
        .iter()
        .filter_map(|r| r.get("table_name")?.as_str().map(str::to_owned))
        .collect())
}

async fn fetch_live_columns(
    db: &dyn DatabaseProvider,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let result = db
        .query_raw_with(
            &"SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = \
              'public'",
            &[],
        )
        .await
        .context("Failed to list live columns")?;

    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in &result.rows {
        let Some(table) = row.get("table_name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(column) = row.get("column_name").and_then(|v| v.as_str()) else {
            continue;
        };
        out.entry(table.to_owned())
            .or_default()
            .insert(column.to_owned());
    }
    Ok(out)
}
