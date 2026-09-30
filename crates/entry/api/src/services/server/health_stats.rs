//! Serialized shapes of the detailed health surface: process memory, disk
//! usage, database and table sizes, and audit-log statistics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ProcessMemory {
    #[serde(rename = "rss_mb")]
    pub rss: Option<u64>,
    #[serde(rename = "virtual_mb")]
    pub virtual_size: Option<u64>,
    #[serde(rename = "peak_mb")]
    pub peak: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskUsage {
    pub total: String,
    pub used: String,
    pub available: String,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemStats {
    pub database: Option<DatabaseStats>,
    pub disk: Option<DiskUsage>,
    pub logs: Option<AuditLogStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseStats {
    pub name: String,
    pub total_size: String,
    pub total_size_bytes: i64,
    pub table_count: i64,
    pub top_tables: Vec<TableStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableStats {
    pub table_name: String,
    pub total_size: String,
    pub total_size_bytes: i64,
    pub row_estimate: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogStats {
    pub audit_rows: i64,
    pub audit_size: String,
    pub audit_size_bytes: i64,
    // JSON: runtime `JsonRow` cell — `MIN(created_at)` as the database driver rendered it.
    pub oldest: Option<serde_json::Value>,
    // JSON: runtime `JsonRow` cell — `MAX(created_at)` as the database driver rendered it.
    pub newest: Option<serde_json::Value>,
}
