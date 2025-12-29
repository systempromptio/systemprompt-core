//! Database schema extension trait.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::ExtensionMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinitionTyped {
    pub table: String,
    pub sql: SchemaSourceTyped,
    pub required_columns: Vec<String>,
}

impl SchemaDefinitionTyped {
    #[must_use]
    pub fn embedded(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSourceTyped::Embedded(sql.into()),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn file(table: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSourceTyped::File(path.into()),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_required_columns(mut self, columns: Vec<String>) -> Self {
        self.required_columns = columns;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaSourceTyped {
    Embedded(String),
    File(PathBuf),
}

pub trait SchemaExtensionTyped: ExtensionMeta {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped>;

    fn migration_weight(&self) -> u32 {
        100
    }
}
