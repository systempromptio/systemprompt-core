//! Lightweight database-only runtime context.
//!
//! [`DatabaseContext`] is used by CLI / tooling paths that need a
//! `DbPool` without spinning up the full [`crate::AppContext`].

use crate::error::RuntimeResult;
use std::sync::Arc;
use systemprompt_database::{Database, DbPool};

/// A bare runtime context that exposes only a database pool.
#[derive(Debug, Clone)]
pub struct DatabaseContext {
    database: DbPool,
}

impl DatabaseContext {
    /// Build a context from a single Postgres URL (read = write).
    pub async fn from_url(database_url: &str) -> RuntimeResult<Self> {
        let db = Database::new_postgres(database_url).await?;
        Ok(Self {
            database: Arc::new(db),
        })
    }

    /// Build a context from a read URL and an optional write URL.
    pub async fn from_urls(read_url: &str, write_url: Option<&str>) -> RuntimeResult<Self> {
        let db = Database::from_config_with_write("postgres", read_url, write_url).await?;
        Ok(Self {
            database: Arc::new(db),
        })
    }

    /// Borrow the underlying [`DbPool`].
    pub const fn db_pool(&self) -> &DbPool {
        &self.database
    }

    /// Clone the underlying [`DbPool`] handle.
    pub fn db_pool_arc(&self) -> DbPool {
        Arc::clone(&self.database)
    }
}
