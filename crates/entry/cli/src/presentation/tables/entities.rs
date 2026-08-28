//! Listing tables for stored artifacts, contexts, and database tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tabled::{Table, Tabled};

use crate::commands::core::artifacts::ArtifactSummary;
use crate::commands::core::contexts::ContextSummary;
use crate::commands::infrastructure::db::TableInfo;
use crate::shared::truncate_with_ellipsis;

use super::dash;

#[derive(Tabled)]
struct ArtifactListRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    artifact_type: String,
    #[tabled(rename = "Tool")]
    tool_name: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[must_use]
pub fn artifact_list_table(artifacts: &[ArtifactSummary]) -> String {
    let rows: Vec<ArtifactListRow> = artifacts
        .iter()
        .map(|a| ArtifactListRow {
            id: truncate_with_ellipsis(a.artifact_id.as_str(), 12),
            name: a.name.clone().unwrap_or_else(dash),
            artifact_type: a.artifact_type.clone(),
            tool_name: a.tool_name.clone().unwrap_or_else(dash),
            created_at: a.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct ContextListRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Tasks")]
    task_count: i64,
    #[tabled(rename = "Messages")]
    message_count: i64,
    #[tabled(rename = "Updated")]
    updated_at: String,
    #[tabled(rename = "Active")]
    active: String,
}

#[must_use]
pub fn context_list_table(contexts: &[ContextSummary]) -> String {
    let rows: Vec<ContextListRow> = contexts
        .iter()
        .map(|c| ContextListRow {
            id: c.id.as_str().chars().take(8).collect(),
            name: truncate_with_ellipsis(&c.name, 40),
            task_count: c.task_count,
            message_count: c.message_count,
            updated_at: c.updated_at.format("%Y-%m-%d %H:%M").to_string(),
            active: if c.is_active {
                "*".to_owned()
            } else {
                String::new()
            },
        })
        .collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct DbTableRow {
    #[tabled(rename = "Table")]
    name: String,
    #[tabled(rename = "Rows")]
    row_count: i64,
    #[tabled(rename = "Size")]
    size: String,
}

#[must_use]
pub fn db_tables_table(tables: &[TableInfo]) -> String {
    let rows: Vec<DbTableRow> = tables
        .iter()
        .map(|t| DbTableRow {
            name: t.name.clone(),
            row_count: t.row_count,
            size: crate::commands::infrastructure::db::format_bytes(t.size_bytes),
        })
        .collect();
    Table::new(rows).to_string()
}
