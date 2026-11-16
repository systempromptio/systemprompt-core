//! Schema-inspection handlers for the `db` command group.
//!
//! Implements the `tables`, `describe`, `info`, and `count` subcommands over a
//! [`DatabaseAdminService`], rendering JSON or formatted text. Reconciling the
//! live schema against extension declarations is the `doctor` subcommand's job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use systemprompt_database::{DatabaseAdminService, SafeIdentifier};
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_traits::RepositoryError;

use crate::shared::CommandOutput;

use super::text_format::format_bytes;
use super::types::{
    ColumnInfo, DbCountOutput, DbDescribeOutput, DbInfoOutput, DbTablesOutput, IndexInfo, TableInfo,
};

pub(super) async fn execute_tables(
    admin: &DatabaseAdminService,
    filter: Option<String>,
    exact: bool,
) -> Result<CommandOutput> {
    let tables = if exact {
        admin.list_tables_counted().await
    } else {
        admin.list_tables().await
    }
    .context("Failed to list tables")?;

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

    let mut result =
        CommandOutput::table_of(vec!["name", "row_count", "size_bytes"], &output.tables)
            .with_title("Tables")
            .with_note(NoticeLine::new(
                "info",
                format!("Total: {} table(s)", output.total),
            ));
    if !exact {
        result = result.with_note(NoticeLine::new(
            "info",
            "Row counts are planner estimates (pg_stat_user_tables); use --exact for COUNT(*)",
        ));
    }
    Ok(result)
}

pub(super) async fn execute_describe(
    admin: &DatabaseAdminService,
    table_name: &str,
) -> Result<CommandOutput> {
    let table_id = SafeIdentifier::parse(table_name)
        .map_err(|e| anyhow!("Invalid table name '{}': {}", table_name, e))?;

    let (columns, row_count) = admin
        .describe_table(&table_id)
        .await
        .map_err(|e| table_error(e, table_name, "Failed to describe table"))?;

    let indexes = admin
        .list_table_indexes(&table_id)
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

    Ok(CommandOutput::table_of(
        vec!["name", "type", "nullable", "default", "primary_key"],
        &output.columns,
    )
    .with_title(format!("Table: {table_name} ({row_count} rows)")))
}

pub(super) async fn execute_info(admin: &DatabaseAdminService) -> Result<CommandOutput> {
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

    Ok(CommandOutput::card_value("Database Info", &output))
}

pub(super) async fn execute_count(
    admin: &DatabaseAdminService,
    table_name: &str,
) -> Result<CommandOutput> {
    let table_id = SafeIdentifier::parse(table_name)
        .map_err(|e| anyhow!("Invalid table name '{}': {}", table_name, e))?;

    let count = admin
        .count_rows(&table_id)
        .await
        .map_err(|e| table_error(e, table_name, "Failed to count rows"))?;

    let output = DbCountOutput {
        table: table_name.to_owned(),
        count,
    };

    Ok(CommandOutput::card_value("Row Count", &output))
}

// Why: Postgres reports a missing relation as SQLSTATE 42P01 (undefined_table).
const UNDEFINED_TABLE: &str = "42P01";

fn table_error(err: RepositoryError, table_name: &str, context: &'static str) -> anyhow::Error {
    if err.is_not_found() || err.sqlstate() == Some(UNDEFINED_TABLE) {
        anyhow!("Table '{table_name}' not found")
    } else {
        anyhow::Error::new(err).context(context)
    }
}
