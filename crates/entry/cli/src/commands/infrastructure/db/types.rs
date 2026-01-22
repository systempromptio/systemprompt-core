use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStatusOutput {
    pub status: String,
    pub version: String,
    pub tables: usize,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbInfoOutput {
    pub version: String,
    pub database: String,
    pub size: String,
    pub table_count: usize,
    pub tables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTablesOutput {
    pub tables: Vec<TableInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub row_count: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbDescribeOutput {
    pub table: String,
    pub row_count: i64,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub row_count: usize,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbExecuteOutput {
    pub rows_affected: u64,
    pub execution_time_ms: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMigrateOutput {
    pub modules_installed: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAssignAdminOutput {
    pub user_id: UserId,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub already_admin: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbValidateOutput {
    pub valid: bool,
    pub expected_tables: usize,
    pub actual_tables: usize,
    pub missing_tables: Vec<String>,
    pub extra_tables: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCountOutput {
    pub table: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbIndexesOutput {
    pub indexes: Vec<TableIndexInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIndexInfo {
    pub table: String,
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSizeOutput {
    pub database_size: String,
    pub database_size_bytes: i64,
    pub table_count: usize,
    pub largest_tables: Vec<TableSizeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSizeInfo {
    pub name: String,
    pub size: String,
    pub size_bytes: i64,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatusOutput {
    pub extensions: Vec<ExtensionMigrationStatus>,
    pub total_pending: usize,
    pub total_applied: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMigrationStatus {
    pub extension_id: String,
    pub is_required: bool,
    pub total_defined: usize,
    pub total_applied: usize,
    pub pending_count: usize,
    pub pending_versions: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationHistoryOutput {
    pub extension_id: String,
    pub migrations: Vec<AppliedMigrationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigrationInfo {
    pub version: u32,
    pub name: String,
    pub checksum: String,
    pub applied_at: Option<String>,
}
