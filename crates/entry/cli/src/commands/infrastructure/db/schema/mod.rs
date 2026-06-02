//! Schema-inspection handlers for the `db` command group.
//!
//! Implements the `tables`, `describe`, `info`, and `count` subcommands over a
//! [`DatabaseAdminService`], rendering JSON or formatted text. Schema
//! validation lives in the `validate` submodule and is re-exported here.

mod validate;

pub(super) use validate::execute_validate;

use anyhow::{Context, Result, anyhow};
use systemprompt_database::{DatabaseAdminService, DatabaseCliDisplay, SafeIdentifier};
use systemprompt_logging::CliService;
use tabled::{Table, Tabled};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::helpers::format_bytes;
use super::types::{
    ColumnInfo, DbCountOutput, DbDescribeOutput, DbInfoOutput, DbTablesOutput, IndexInfo, TableInfo,
};

#[derive(Tabled)]
struct TableRow {
    #[tabled(rename = "Table")]
    name: String,
    #[tabled(rename = "Rows")]
    row_count: i64,
    #[tabled(rename = "Size")]
    size: String,
}

pub(super) async fn execute_tables(
    admin: &DatabaseAdminService,
    filter: Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let tables = admin.list_tables().await.context("Failed to list tables")?;

    let filtered_tables: Vec<_> = if let Some(pattern) = &filter {
        let pattern = pattern.replace(['%', '*'], "");
        tables
            .into_iter()
            .filter(|t| t.name.contains(&pattern))
            .collect()
    } else {
        tables
    };

    let output = DbTablesOutput {
        total: filtered_tables.len(),
        tables: filtered_tables
            .iter()
            .map(|t| TableInfo {
                name: t.name.clone(),
                schema: "public".to_owned(),
                row_count: t.row_count,
                size_bytes: t.size_bytes,
            })
            .collect(),
    };

    if config.is_json_output() {
        let result =
            CommandOutput::table_of(vec!["name", "row_count", "size_bytes"], &output.tables)
                .with_title("Schema");
        render_result(&result);
    } else {
        CliService::section("Tables");

        if filtered_tables.is_empty() {
            CliService::info("No tables found");
        } else {
            let rows: Vec<TableRow> = filtered_tables
                .iter()
                .map(|t| TableRow {
                    name: t.name.clone(),
                    row_count: t.row_count,
                    size: format_bytes(t.size_bytes),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);
            CliService::info(&format!("Total: {} table(s)", output.total));
        }
    }

    Ok(())
}

pub(super) async fn execute_describe(
    admin: &DatabaseAdminService,
    table_name: &str,
    config: &CliConfig,
) -> Result<()> {
    let table_id = SafeIdentifier::parse(table_name)
        .map_err(|e| anyhow!("Invalid table name '{}': {}", table_name, e))?;

    let (columns, row_count) = admin.describe_table(&table_id).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") || msg.contains("does not exist") {
            anyhow!("Table '{}' not found", table_name)
        } else {
            anyhow!("Failed to describe table: {}", msg)
        }
    })?;

    let indexes = admin
        .get_table_indexes(&table_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(table = %table_name, error = %e, "Failed to get table indexes");
            Vec::new()
        });

    let output = DbDescribeOutput {
        table: table_name.to_owned(),
        row_count,
        columns: columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                nullable: c.nullable,
                default: c.default.clone(),
                primary_key: c.primary_key,
            })
            .collect(),
        indexes: indexes
            .iter()
            .map(|i| IndexInfo {
                name: i.name.clone(),
                columns: i.columns.clone(),
                unique: i.unique,
            })
            .collect(),
    };

    if config.is_json_output() {
        let result = CommandOutput::table_of(
            vec!["name", "data_type", "nullable", "primary_key"],
            &output.columns,
        )
        .with_title("Schema");
        render_result(&result);
    } else {
        CliService::section(&format!("Table: {} ({} rows)", table_name, row_count));
        CliService::subsection("Columns");
        (columns, row_count).display_with_cli();

        if !indexes.is_empty() {
            CliService::subsection("Indexes");
            for idx in &indexes {
                let unique_marker = if idx.unique { " (unique)" } else { "" };
                CliService::info(&format!(
                    "  {} [{}]{}",
                    idx.name,
                    idx.columns.join(", "),
                    unique_marker
                ));
            }
        }
    }

    Ok(())
}

pub(super) async fn execute_info(admin: &DatabaseAdminService, config: &CliConfig) -> Result<()> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let table_names: Vec<String> = info.tables.iter().map(|t| t.name.clone()).collect();

    let output = DbInfoOutput {
        version: info.version.clone(),
        database: info.path.clone(),
        size: format_bytes(info.size as i64),
        table_count: info.tables.len(),
        tables: table_names,
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Database Schema", &output);
        render_result(&result);
    } else {
        CliService::section("Database Info");
        CliService::key_value("Database", &output.database);
        CliService::key_value("Version", &output.version);
        CliService::key_value("Size", &output.size);
        CliService::key_value("Tables", &output.table_count.to_string());
    }

    Ok(())
}

pub(super) async fn execute_count(
    admin: &DatabaseAdminService,
    table_name: &str,
    config: &CliConfig,
) -> Result<()> {
    let table_id = SafeIdentifier::parse(table_name)
        .map_err(|e| anyhow!("Invalid table name '{}': {}", table_name, e))?;

    let count = admin.count_rows(&table_id).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") || msg.contains("does not exist") {
            anyhow!("Table '{}' not found", table_name)
        } else {
            anyhow!("Failed to count rows: {}", msg)
        }
    })?;

    let output = DbCountOutput {
        table: table_name.to_owned(),
        count,
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Row Count", &output);
        render_result(&result);
    } else {
        CliService::info(&format!("{}: {} rows", table_name, count));
    }

    Ok(())
}
