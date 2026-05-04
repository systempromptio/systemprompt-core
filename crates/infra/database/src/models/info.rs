//! Descriptors for databases, tables, columns, and indexes.

use serde::{Deserialize, Serialize};

/// High-level snapshot of a connected `PostgreSQL` instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    /// Free-form path/URL or backend identifier (currently always
    /// `"PostgreSQL"`).
    pub path: String,
    /// On-disk size of the database in bytes (`pg_database_size`).
    pub size: u64,
    /// Server version string returned by `version()`.
    pub version: String,
    /// All public-schema tables visible to the configured connection.
    pub tables: Vec<TableInfo>,
}

/// Descriptor for a single public-schema table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    /// Unqualified table name.
    pub name: String,
    /// Live row estimate (`pg_stat_user_tables.n_live_tup`).
    pub row_count: i64,
    /// Total relation size including indexes and TOAST, in bytes.
    #[serde(default)]
    pub size_bytes: i64,
    /// Column descriptors in `ordinal_position` order. May be empty for
    /// listing endpoints that do not eagerly populate columns.
    pub columns: Vec<ColumnInfo>,
}

/// Descriptor for a single column within a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// Postgres `data_type` string (`information_schema.columns.data_type`).
    pub data_type: String,
    /// Whether the column accepts `NULL`.
    pub nullable: bool,
    /// Whether the column participates in the table's primary key.
    pub primary_key: bool,
    /// `column_default` expression text, if any.
    pub default: Option<String>,
}

/// Descriptor for a single index defined on a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    /// Index name.
    pub name: String,
    /// Indexed columns in key order.
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
}
