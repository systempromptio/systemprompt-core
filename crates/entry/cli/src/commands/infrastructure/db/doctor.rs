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
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::services::schema_linter::created_table_names;
use systemprompt_database::{DbPool, SchemaResidue, audit_schema_residue};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Serialize)]
struct DoctorReport {
    undeclared_tables: Vec<String>,
    orphan_migration_ledgers: Vec<String>,
    missing_tables: Vec<String>,
    missing_columns: Vec<MissingColumn>,
}

#[derive(Debug, Serialize)]
struct MissingColumn {
    extension: String,
    table: String,
    column: String,
}

pub(super) async fn execute_doctor(db_pool: &DbPool, config: &CliConfig) -> Result<()> {
    let registry = ExtensionRegistry::discover()?;
    let write_provider = db_pool.write();

    let mut declared_columns: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut owner: BTreeMap<String, String> = BTreeMap::new();

    for ext in registry.schema_extensions() {
        let ext_id = ext.id().to_owned();
        for schema in ext.schemas() {
            let Some(table) = schema.table else {
                continue;
            };
            owner.entry(table.clone()).or_insert_with(|| ext_id.clone());
            let entry = declared_columns.entry(table).or_default();
            for col in &schema.required_columns {
                entry.insert(col.clone());
            }
        }
    }

    let live_tables = fetch_live_tables(write_provider).await?;
    let live_columns = fetch_live_columns(write_provider).await?;

    // Why: the residue audit parses every extension's CREATE TABLEs, so a
    // schema file registered without a table name still declares its
    // tables; the `schema.table` list above is only for required columns.
    let mut owned: Vec<String> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    for ext in registry.schema_extensions() {
        ids.push(ext.id().to_owned());
        for schema in ext.schemas() {
            owned.extend(created_table_names(&schema.sql).unwrap_or_default());
        }
    }
    let residue = audit_schema_residue(write_provider, &owned, &ids).await?;
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

    let mut missing_columns: Vec<(String, String, String)> = Vec::new();
    for (table, required) in &declared_columns {
        let Some(live_cols) = live_columns.get(table) else {
            continue;
        };
        let owner_id = owner.get(table).cloned().unwrap_or_else(String::new);
        for col in required {
            if !live_cols.contains(col) {
                missing_columns.push((owner_id.clone(), table.clone(), col.clone()));
            }
        }
    }

    render(
        config,
        &undeclared,
        &residue,
        &missing_tables,
        &missing_columns,
    );

    if !residue.is_empty() {
        anyhow::bail!(
            "{} undeclared table(s) and {} orphan migration ledger(s): add a DROP TABLE migration",
            residue.undeclared_tables.len(),
            residue.orphan_migration_ledgers.len()
        );
    }
    Ok(())
}

fn render(
    config: &CliConfig,
    undeclared: &[String],
    residue: &SchemaResidue,
    missing_tables: &[String],
    missing_columns: &[(String, String, String)],
) {
    let orphan_ledgers: Vec<String> = residue
        .orphan_migration_ledgers
        .iter()
        .map(|l| format!("{} ({} rows)", l.extension_id, l.rows))
        .collect();
    if config.is_json_output() {
        let report = DoctorReport {
            undeclared_tables: undeclared.to_vec(),
            orphan_migration_ledgers: orphan_ledgers,
            missing_tables: missing_tables.to_vec(),
            missing_columns: missing_columns
                .iter()
                .map(|(ext, table, col)| MissingColumn {
                    extension: ext.clone(),
                    table: table.clone(),
                    column: col.clone(),
                })
                .collect(),
        };
        let result = CommandOutput::card_value("Database Doctor", &report);
        render_result(&result, config);
        return;
    }

    if undeclared.is_empty()
        && orphan_ledgers.is_empty()
        && missing_tables.is_empty()
        && missing_columns.is_empty()
    {
        CliService::success("Schema in sync with extension declarations");
        return;
    }

    if !missing_tables.is_empty() {
        CliService::warning(&format!(
            "{} declared table(s) absent from the live database:",
            missing_tables.len()
        ));
        for t in missing_tables {
            CliService::info(&format!("  - {t}"));
        }
    }

    if !missing_columns.is_empty() {
        CliService::warning(&format!(
            "{} required column(s) absent from live tables:",
            missing_columns.len()
        ));
        for (ext, table, col) in missing_columns {
            CliService::info(&format!("  - [{ext}] {table}.{col}"));
        }
    }

    if !undeclared.is_empty() {
        CliService::warning(&format!(
            "{} live table(s) declared by no registered extension — a deleted crate's residue; \
             add a DROP TABLE … CASCADE migration:",
            undeclared.len()
        ));
        for t in undeclared {
            CliService::info(&format!("  - {t}"));
        }
    }

    if !orphan_ledgers.is_empty() {
        CliService::warning(&format!(
            "{} extension_migrations ledger(s) for extensions that no longer exist — delete \
             the rows in the same migration:",
            orphan_ledgers.len()
        ));
        for l in &orphan_ledgers {
            CliService::info(&format!("  - {l}"));
        }
    }
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
