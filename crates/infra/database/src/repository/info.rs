//! Read-only repository over the [`DatabaseInfo`] introspection surface.

use crate::error::DatabaseResult;
use crate::models::{DatabaseInfo, TableInfo};
use crate::services::Database;
use std::sync::Arc;

#[derive(Debug)]
pub struct DatabaseInfoRepository {
    db: Arc<Database>,
}

impl DatabaseInfoRepository {
    #[must_use]
    pub const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        self.db.get_info().await
    }

    pub async fn list_tables(&self) -> DatabaseResult<Vec<TableInfo>> {
        let info = self.db.get_info().await?;
        Ok(info.tables)
    }

    pub async fn get_table_info(&self, table_name: &str) -> DatabaseResult<Option<TableInfo>> {
        let info = self.db.get_info().await?;
        Ok(info.tables.into_iter().find(|t| t.name == table_name))
    }
}
