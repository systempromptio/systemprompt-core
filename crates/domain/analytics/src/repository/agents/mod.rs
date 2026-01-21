mod detail_queries;
mod list_queries;
mod stats_queries;

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct AgentAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl AgentAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
