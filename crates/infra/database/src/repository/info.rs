//! Read-only repository over the [`DatabaseInfo`] introspection surface.

use crate::models::{DatabaseInfo, TableInfo};
use crate::services::Database;
use anyhow::Result;
use std::sync::Arc;

/// Repository wrapping [`Database::get_info`] for use from CLI/admin code that
/// wants strongly-typed [`TableInfo`] / [`DatabaseInfo`] structs.
#[derive(Debug)]
pub struct DatabaseInfoRepository {
    db: Arc<Database>,
}

impl DatabaseInfoRepository {
    /// Wrap an existing [`Database`] handle.
    #[must_use]
    pub const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Return the full database snapshot (version, tables, columns).
    pub async fn get_database_info(&self) -> Result<DatabaseInfo> {
        self.db.get_info().await
    }

    /// List every table in the public schema.
    pub async fn list_tables(&self) -> Result<Vec<TableInfo>> {
        let info = self.db.get_info().await?;
        Ok(info.tables)
    }

    /// Look up a single table by name.
    pub async fn get_table_info(&self, table_name: &str) -> Result<Option<TableInfo>> {
        let info = self.db.get_info().await?;
        Ok(info.tables.into_iter().find(|t| t.name == table_name))
    }
}
