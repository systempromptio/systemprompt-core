//! Static metadata, schema, and role-definition value types.

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
    pub sql: String,
    pub required_columns: Vec<String>,
    #[serde(default)]
    pub schema: Option<String>,
}

impl SchemaDefinition {
    #[must_use]
    pub fn new(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: sql.into(),
            required_columns: Vec::new(),
            schema: None,
        }
    }

    #[must_use]
    pub fn with_required_columns(mut self, columns: Vec<String>) -> Self {
        self.required_columns = columns;
        self
    }

    #[must_use]
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Resolved Postgres schema name, defaulting to `public`.
    #[must_use]
    pub fn schema_name(&self) -> &str {
        self.schema.as_deref().unwrap_or("public")
    }
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
