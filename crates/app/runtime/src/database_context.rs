use anyhow::Result;
use std::sync::Arc;
use systemprompt_database::{Database, DbPool};

#[derive(Debug, Clone)]
pub struct DatabaseContext {
    database: DbPool,
}

impl DatabaseContext {
    pub async fn from_url(database_url: &str) -> Result<Self> {
        let db = Database::new_postgres(database_url).await?;
        Ok(Self {
            database: Arc::new(db),
        })
    }

    pub async fn from_urls(read_url: &str, write_url: Option<&str>) -> Result<Self> {
        let db = Database::from_config_with_write("postgres", read_url, write_url).await?;
        Ok(Self {
            database: Arc::new(db),
        })
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.database
    }

    pub fn db_pool_arc(&self) -> DbPool {
        Arc::clone(&self.database)
    }
}
