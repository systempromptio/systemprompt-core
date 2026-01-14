use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use systemprompt_core_database::{DatabaseAdminService, DatabaseCliDisplay};
use systemprompt_core_logging::CliService;
use tabled::{Table, Tabled};

use crate::cli_settings::CliConfig;

use super::helpers::format_bytes;
use super::types::{
    ColumnInfo, DbCountOutput, DbDescribeOutput, DbInfoOutput, DbTablesOutput, DbValidateOutput,
    IndexInfo, TableInfo,
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

pub async fn execute_tables(
    admin: &DatabaseAdminService,
    filter: Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let tables = admin
        .list_tables()
        .await
        .context("Failed to list tables")?;

    let filtered_tables: Vec<_> = if let Some(pattern) = &filter {
        let pattern = pattern.replace('%', "").replace('*', "");
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
                schema: "public".to_string(),
                row_count: t.row_count,
                size_bytes: t.size_bytes,
            })
            .collect(),
    };

    if config.is_json_output() {
        CliService::json(&output);
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

pub async fn execute_describe(
    admin: &DatabaseAdminService,
    table_name: &str,
    config: &CliConfig,
) -> Result<()> {
    let (columns, row_count) = admin.describe_table(table_name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") || msg.contains("does not exist") {
            anyhow!("Table '{}' not found", table_name)
        } else {
            anyhow!("Failed to describe table: {}", msg)
        }
    })?;

    let indexes = admin
        .get_table_indexes(table_name)
        .await
        .unwrap_or_default();

    let output = DbDescribeOutput {
        table: table_name.to_string(),
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
        CliService::json(&output);
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

pub async fn execute_info(admin: &DatabaseAdminService, config: &CliConfig) -> Result<()> {
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
        CliService::json(&output);
    } else {
        CliService::section("Database Info");
        CliService::key_value("Database", &output.database);
        CliService::key_value("Version", &output.version);
        CliService::key_value("Size", &output.size);
        CliService::key_value("Tables", &output.table_count.to_string());
    }

    Ok(())
}

pub async fn execute_validate(admin: &DatabaseAdminService, config: &CliConfig) -> Result<()> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let expected_tables: Vec<&str> = DatabaseAdminService::get_expected_tables();
    let table_names: Vec<String> = info.tables.iter().map(|t| t.name.clone()).collect();
    let actual_tables: HashSet<&str> = table_names.iter().map(|s| s.as_str()).collect();

    let missing: Vec<String> = expected_tables
        .iter()
        .filter(|t| !actual_tables.contains(*t))
        .map(|t| t.to_string())
        .collect();

    let extra: Vec<String> = table_names
        .iter()
        .filter(|t| {
            !expected_tables.contains(&t.as_str())
                && !t.starts_with("_sqlx")
                && !t.starts_with("v_")
        })
        .cloned()
        .collect();

    let valid = missing.is_empty();

    let output = DbValidateOutput {
        valid,
        expected_tables: expected_tables.len(),
        actual_tables: table_names.len(),
        missing_tables: missing.clone(),
        extra_tables: extra.clone(),
        message: if valid {
            "Database schema is valid".to_string()
        } else {
            format!("Database schema has {} missing table(s)", missing.len())
        },
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section("Schema Validation");

        if valid {
            CliService::success(&output.message);
        } else {
            CliService::error(&output.message);
            CliService::info("Missing tables:");
            for table in &missing {
                CliService::info(&format!("  - {}", table));
            }
        }

        if !extra.is_empty() && config.should_show_verbose() {
            CliService::info("Extra tables (not in expected list):");
            for table in &extra {
                CliService::info(&format!("  - {}", table));
            }
        }

        CliService::info(&format!(
            "Expected: {}, Actual: {}",
            output.expected_tables, output.actual_tables
        ));
    }

    Ok(())
}

pub async fn execute_count(
    admin: &DatabaseAdminService,
    table_name: &str,
    config: &CliConfig,
) -> Result<()> {
    let count = admin.count_rows(table_name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") || msg.contains("does not exist") {
            anyhow!("Table '{}' not found", table_name)
        } else {
            anyhow!("Failed to count rows: {}", msg)
        }
    })?;

    let output = DbCountOutput {
        table: table_name.to_string(),
        count,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::info(&format!("{}: {} rows", table_name, count));
    }

    Ok(())
}
