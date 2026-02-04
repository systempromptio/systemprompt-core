use anyhow::{Context, Result};
use systemprompt_database::DatabaseAdminService;
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::shared::{render_result, CommandResult};

use super::helpers::format_bytes;
use super::types::{DbIndexesOutput, DbSizeOutput, TableIndexInfo, TableSizeInfo};

pub async fn execute_indexes(
    admin: &DatabaseAdminService,
    table_filter: Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let tables = admin.list_tables().await.context("Failed to list tables")?;

    let filtered_tables: Vec<_> = if let Some(ref filter) = table_filter {
        tables.into_iter().filter(|t| t.name == *filter).collect()
    } else {
        tables
    };

    let mut all_indexes = Vec::new();

    for table in &filtered_tables {
        match admin.get_table_indexes(&table.name).await {
            Ok(indexes) => {
                for idx in indexes {
                    all_indexes.push(TableIndexInfo {
                        table: table.name.clone(),
                        name: idx.name,
                        columns: idx.columns,
                        unique: idx.unique,
                    });
                }
            },
            Err(e) => {
                tracing::warn!(table = %table.name, error = %e, "Failed to get table indexes");
            },
        }
    }

    let output = DbIndexesOutput {
        indexes: all_indexes.clone(),
        total: all_indexes.len(),
    };

    if config.is_json_output() {
        let result = CommandResult::table(output)
            .with_title("Database Schema")
            .with_columns(vec![
                "table".into(),
                "name".into(),
                "columns".into(),
                "unique".into(),
            ]);
        render_result(&result);
    } else {
        CliService::section("Indexes");

        if all_indexes.is_empty() {
            CliService::info("No indexes found");
        } else {
            for idx in &all_indexes {
                let unique_marker = if idx.unique { " (unique)" } else { "" };
                CliService::info(&format!(
                    "{}.{} [{}]{}",
                    idx.table,
                    idx.name,
                    idx.columns.join(", "),
                    unique_marker
                ));
            }
            CliService::info(&format!("\nTotal: {} index(es)", output.total));
        }
    }

    Ok(())
}

pub async fn execute_size(admin: &DatabaseAdminService, config: &CliConfig) -> Result<()> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let tables = admin.list_tables().await.context("Failed to list tables")?;

    let mut sorted_tables = tables.clone();
    sorted_tables.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let largest: Vec<TableSizeInfo> = sorted_tables
        .iter()
        .take(10)
        .map(|t| TableSizeInfo {
            name: t.name.clone(),
            size: format_bytes(t.size_bytes),
            size_bytes: t.size_bytes,
            rows: t.row_count,
        })
        .collect();

    let output = DbSizeOutput {
        database_size: format_bytes(info.size as i64),
        database_size_bytes: info.size as i64,
        table_count: tables.len(),
        largest_tables: largest.clone(),
    };

    if config.is_json_output() {
        let result = CommandResult::dashboard(output).with_title("Database Size");
        render_result(&result);
    } else {
        CliService::section("Database Size");
        CliService::key_value("Total Size", &output.database_size);
        CliService::key_value("Table Count", &output.table_count.to_string());

        CliService::subsection("Largest Tables");
        for table in &largest {
            CliService::info(&format!(
                "  {} - {} ({} rows)",
                table.name, table.size, table.rows
            ));
        }
    }

    Ok(())
}
