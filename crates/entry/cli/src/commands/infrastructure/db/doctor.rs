use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_database::DbPool;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Serialize)]
struct DoctorReport {
    undeclared_tables: Vec<String>,
    missing_tables: Vec<String>,
    missing_columns: Vec<MissingColumn>,
}

#[derive(Debug, Serialize)]
struct MissingColumn {
    extension: String,
    table: String,
    column: String,
}

pub(crate) async fn execute_doctor(db_pool: &DbPool, config: &CliConfig) -> Result<()> {
    let registry = ExtensionRegistry::discover()?;
    let write_provider = db_pool.write_provider();

    let mut declared_columns: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut owner: BTreeMap<String, String> = BTreeMap::new();

    for ext in registry.schema_extensions() {
        let ext_id = ext.id().to_owned();
        for schema in ext.schemas() {
            if schema.table.is_empty() {
                continue;
            }
            owner
                .entry(schema.table.clone())
                .or_insert_with(|| ext_id.clone());
            let entry = declared_columns.entry(schema.table.clone()).or_default();
            for col in &schema.required_columns {
                entry.insert(col.clone());
            }
        }
    }

    let live_tables = fetch_live_tables(write_provider).await?;
    let live_columns = fetch_live_columns(write_provider).await?;

    let undeclared: Vec<String> = live_tables
        .iter()
        .filter(|t| !declared_columns.contains_key(*t))
        .cloned()
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

    render(config, &undeclared, &missing_tables, &missing_columns);

    Ok(())
}

fn render(
    config: &CliConfig,
    undeclared: &[String],
    missing_tables: &[String],
    missing_columns: &[(String, String, String)],
) {
    if config.is_json_output() {
        let report = DoctorReport {
            undeclared_tables: undeclared.to_vec(),
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
        let result = CommandResult::text(report).with_title("Database Doctor");
        render_result(&result);
        return;
    }

    if undeclared.is_empty() && missing_tables.is_empty() && missing_columns.is_empty() {
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
        CliService::info(&format!(
            "{} live table(s) not declared by any registered extension (informational):",
            undeclared.len()
        ));
        for t in undeclared {
            CliService::info(&format!("  - {t}"));
        }
    }
}

async fn fetch_live_tables(db: &dyn DatabaseProvider) -> Result<BTreeSet<String>> {
    let result = db
        .query_raw_with(
            &"SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'",
            &[],
        )
        .await
        .context("Failed to list live tables")?;

    Ok(result
        .rows
        .iter()
        .filter_map(|r| r.get("table_name")?.as_str().map(str::to_string))
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
