//! `infra db introspect` command dumping schema structure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_database::{DatabaseAdminService, SafeIdentifier};
use systemprompt_models::artifacts::NoticeLine;

use crate::shared::CommandOutput;

use super::text_format::format_bytes;
use super::types::{DbIndexesOutput, DbSizeOutput, TableIndexInfo, TableSizeInfo};

pub(super) async fn execute_indexes(
    admin: &DatabaseAdminService,
    table_filter: Option<String>,
) -> Result<CommandOutput> {
    let tables = admin.list_tables().await.context("Failed to list tables")?;

    let filtered_tables: Vec<_> = if let Some(ref filter) = table_filter {
        tables.into_iter().filter(|t| t.name == *filter).collect()
    } else {
        tables
    };

    let mut all_indexes = Vec::new();

    for table in &filtered_tables {
        let table_id = match SafeIdentifier::parse(&table.name) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(table = %table.name, error = %e, "skipping table with invalid identifier");
                continue;
            },
        };
        match admin.list_table_indexes(&table_id).await {
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
        total: all_indexes.len(),
        indexes: all_indexes,
    };

    Ok(
        CommandOutput::table_of(vec!["table", "name", "columns", "unique"], &output.indexes)
            .with_title("Indexes")
            .with_note(NoticeLine::new(
                "info",
                format!("Total: {} index(es)", output.total),
            )),
    )
}}

pub(super) async fn execute_size(admin: &DatabaseAdminService) -> Result<CommandOutput> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let tables = admin.list_tables().await.context("Failed to list tables")?;

    let mut sorted_tables = tables.clone();
    sorted_tables.sort_by_key(|x| std::cmp::Reverse(x.size_bytes));

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
        largest_tables: largest,
    };

    Ok(CommandOutput::card_value("Database Size", &output))
}}
