use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub table: String,
    pub sql: SchemaSource,
    pub required_columns: Vec<String>,
}

impl SchemaDefinition {
    #[must_use]
    pub fn inline(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::Inline(sql.into()),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn file(table: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::File(path.into()),
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
pub enum SchemaSource {
    Inline(String),
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeedSource {
    Inline(String),
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRole {
    pub name: String,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl ExtensionRole {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            description: description.into(),
            permissions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }
}
