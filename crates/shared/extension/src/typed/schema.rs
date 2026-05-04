//! [`SchemaExtensionTyped`] — typed contract for schema-bearing
//! extensions, plus the [`SchemaDefinitionTyped`] value type.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::ExtensionMeta;

/// Schema definition contributed by a typed schema-bearing extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinitionTyped {
    /// SQL table name owned by this definition.
    pub table: String,
    /// Source of the SQL DDL (embedded string or path on disk).
    pub sql: SchemaSourceTyped,
    /// Columns the host should verify exist after the schema is applied.
    pub required_columns: Vec<String>,
}

impl SchemaDefinitionTyped {
    /// Constructs a definition with embedded SQL.
    #[must_use]
    pub fn embedded(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSourceTyped::Embedded(sql.into()),
            required_columns: Vec::new(),
        }
    }

    /// Constructs a definition that loads SQL from disk.
    #[must_use]
    pub fn file(table: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSourceTyped::File(path.into()),
            required_columns: Vec::new(),
        }
    }

    /// Returns a copy with the given required-column list set.
    #[must_use]
    pub fn with_required_columns(mut self, columns: Vec<String>) -> Self {
        self.required_columns = columns;
        self
    }
}

/// Source of SQL DDL referenced by a [`SchemaDefinitionTyped`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaSourceTyped {
    /// SQL embedded inline as a `String`.
    Embedded(String),
    /// SQL loaded from disk at the given path.
    File(PathBuf),
}

/// Typed contract for an extension that contributes schema definitions.
pub trait SchemaExtensionTyped: ExtensionMeta {
    /// Returns the schema definitions this extension contributes.
    fn schemas(&self) -> Vec<SchemaDefinitionTyped>;

    /// Returns the migration ordering weight (lower runs first).
    fn migration_weight(&self) -> u32 {
        100
    }
}
