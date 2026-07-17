//! [`SchemaExtensionTyped`] — typed contract for schema-bearing
//! extensions, plus the [`SchemaDefinitionTyped`] value type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::types::ExtensionMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinitionTyped {
    pub table: String,
    pub sql: String,
    pub required_columns: Vec<String>,
}

impl SchemaDefinitionTyped {
    #[must_use]
    pub fn new(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: sql.into(),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_required_columns(mut self, columns: Vec<String>) -> Self {
        self.required_columns = columns;
        self
    }
}

pub trait SchemaExtensionTyped: ExtensionMeta {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped>;
}
