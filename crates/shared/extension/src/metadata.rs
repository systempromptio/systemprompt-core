//! Static metadata, schema-source, and role-definition value types.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Static metadata block that every extension publishes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    /// Stable extension identifier (kebab-case).
    pub id: &'static str,
    /// Human-readable extension name.
    pub name: &'static str,
    /// Semver-style version string.
    pub version: &'static str,
}

/// Single schema definition contributed by an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    /// SQL table name owned by this definition.
    pub table: String,
    /// Source of the SQL DDL (inline string or path on disk).
    pub sql: SchemaSource,
    /// Columns the host should verify exist after the schema is applied.
    pub required_columns: Vec<String>,
}

impl SchemaDefinition {
    /// Constructs a schema definition with inline SQL.
    #[must_use]
    pub fn inline(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::Inline(sql.into()),
            required_columns: Vec::new(),
        }
    }

    /// Constructs a schema definition that loads SQL from disk.
    #[must_use]
    pub fn file(table: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::File(path.into()),
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

/// Source of SQL DDL referenced by a [`SchemaDefinition`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaSource {
    /// SQL embedded inline as a `String`.
    Inline(String),
    /// SQL loaded from disk at the given path.
    File(PathBuf),
}

/// Source of SQL seed data referenced by an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeedSource {
    /// Seed SQL embedded inline as a `String`.
    Inline(String),
    /// Seed SQL loaded from disk at the given path.
    File(PathBuf),
}

/// Role definition contributed by an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRole {
    /// Stable role name.
    pub name: String,
    /// Human-readable label shown to operators.
    pub display_name: String,
    /// Role description.
    pub description: String,
    /// Permissions this role grants.
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl ExtensionRole {
    /// Constructs a new role with no permissions.
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

    /// Returns a copy with the permission list set.
    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }
}
