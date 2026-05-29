//! Schema validation and auto-migration for MCP service tables.
//!
//! [`SchemaValidator`] reconciles a service's declared [`SchemaDefinition`]s
//! against the live database: it verifies existing tables carry the required
//! columns and, in [`SchemaValidationMode::AutoMigrate`], creates missing
//! tables from the service's schema files. [`SchemaValidationMode`] selects
//! whether a mismatch is auto-healed, treated as fatal, or skipped, and
//! [`SchemaValidationReport`] aggregates the per-table outcome.

use crate::error::McpDomainResult;
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_database::DatabaseProvider;
use systemprompt_models::mcp::deployment::SchemaDefinition;

use super::loader::SchemaLoader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaValidationMode {
    AutoMigrate,
    Strict,
    Skip,
}

impl SchemaValidationMode {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "strict" => Self::Strict,
            "skip" => Self::Skip,
            _ => Self::AutoMigrate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationReport {
    pub service_name: String,
    pub validated: usize,
    pub created: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SchemaValidationReport {
    pub const fn new(service_name: String) -> Self {
        Self {
            service_name,
            validated: 0,
            created: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.validated += other.validated;
        self.created += other.created;
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

#[derive(Debug)]
pub struct SchemaValidator<'a> {
    db: &'a dyn DatabaseProvider,
    mode: SchemaValidationMode,
}

impl<'a> SchemaValidator<'a> {
    pub fn new(db: &'a dyn DatabaseProvider, mode: SchemaValidationMode) -> Self {
        Self { db, mode }
    }

    pub async fn validate_and_apply(
        &self,
        service_name: &str,
        service_path: &Path,
        schemas: &[SchemaDefinition],
    ) -> McpDomainResult<SchemaValidationReport> {
        let mut report = SchemaValidationReport::new(service_name.to_owned());

        if self.mode == SchemaValidationMode::Skip {
            report.warnings.push("Schema validation skipped".to_owned());
            return Ok(report);
        }

        for schema_def in schemas {
            match self.validate_schema(service_path, schema_def).await {
                Ok(created) => {
                    report.validated += 1;
                    if created {
                        report.created += 1;
                    }
                },
                Err(e) => {
                    let error_msg = format!(
                        "Schema validation failed for table '{}': {}",
                        schema_def.table, e
                    );

                    if self.mode == SchemaValidationMode::Strict {
                        report.errors.push(error_msg.clone());
                        return Err(crate::error::McpDomainError::Internal(error_msg));
                    }
                    report.warnings.push(error_msg);
                },
            }
        }

        Ok(report)
    }

    async fn validate_schema(
        &self,
        service_path: &Path,
        schema_def: &SchemaDefinition,
    ) -> McpDomainResult<bool> {
        let table_exists = self.table_exists(&schema_def.table).await?;

        if table_exists {
            self.validate_columns(&schema_def.table, &schema_def.required_columns)
                .await?;
            return Ok(false);
        }

        if self.mode == SchemaValidationMode::AutoMigrate {
            self.create_table(service_path, schema_def).await?;
            return Ok(true);
        }

        Err(crate::error::McpDomainError::SchemaValidation(format!(
            "Table '{}' does not exist and auto_migrate is disabled",
            schema_def.table
        )))
    }

    async fn table_exists(&self, table_name: &str) -> McpDomainResult<bool> {
        let query = "SELECT name FROM sqlite_master WHERE type='table' AND name = ?";
        let row = self.db.fetch_optional(&query, &[&table_name]).await?;
        Ok(row.is_some())
    }

    async fn validate_columns(
        &self,
        table_name: &str,
        required_columns: &[String],
    ) -> McpDomainResult<()> {
        validate_sql_identifier(table_name)?;
        let query = format!("PRAGMA table_info({table_name})");
        let rows = self.db.fetch_all(&query, &[]).await?;

        let existing_columns: Vec<String> = rows
            .iter()
            .filter_map(|row| row.get("name").and_then(|v| v.as_str()).map(str::to_owned))
            .collect();

        let missing_columns: Vec<&String> = required_columns
            .iter()
            .filter(|col| !existing_columns.contains(col))
            .collect();

        if !missing_columns.is_empty() {
            return Err(crate::error::McpDomainError::SchemaValidation(format!(
                "Table '{}' is missing required columns: {}",
                table_name,
                missing_columns
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        Ok(())
    }

    async fn create_table(
        &self,
        service_path: &Path,
        schema_def: &SchemaDefinition,
    ) -> McpDomainResult<()> {
        let sql = SchemaLoader::load_schema_file(service_path, &schema_def.file).map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "Failed to load schema file: {}: {e}",
                schema_def.file
            ))
        })?;

        SchemaLoader::validate_schema_syntax(&sql)?;

        self.db.execute(&sql, &[]).await.map_err(|e| {
            crate::error::McpDomainError::Internal(format!("{}: {e}", {
                format!(
                    "Failed to execute schema SQL for table '{}'",
                    schema_def.table
                )
            }))
        })?;

        let table_exists = self.table_exists(&schema_def.table).await?;
        if !table_exists {
            return Err(crate::error::McpDomainError::SchemaValidation(format!(
                "Schema executed but table '{}' was not created",
                schema_def.table
            )));
        }

        self.validate_columns(&schema_def.table, &schema_def.required_columns)
            .await
            .map_err(|e| {
                crate::error::McpDomainError::Internal(format!("{}: {e}", {
                    format!(
                        "Table '{}' created but missing required columns",
                        schema_def.table
                    )
                }))
            })?;

        Ok(())
    }
}

/// `SQLite` identifiers cannot be parameter-bound in PRAGMA / DDL contexts, so
/// any identifier that reaches `format!`-built SQL must be charset-checked
/// first. Accepts the `SQLite`-safe subset: leading letter / underscore, then
/// up to 63 further alphanumerics / underscores.
fn validate_sql_identifier(name: &str) -> McpDomainResult<()> {
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if first_ok && rest_ok && name.len() <= 64 {
        Ok(())
    } else {
        Err(crate::error::McpDomainError::SchemaValidation(format!(
            "invalid SQL identifier: {name:?}"
        )))
    }
}
