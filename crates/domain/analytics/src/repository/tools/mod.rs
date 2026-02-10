mod detail_queries;
mod list_queries;

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct ToolAnalyticsRepository {
    pool: Arc<PgPool>,
    #[allow(dead_code)]
    write_pool: Arc<PgPool>,
}

impl ToolAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
