mod activity;
mod breakdowns;
mod leaderboards;
mod overview;

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct CoreStatsRepository {
    pool: Arc<PgPool>,
}

impl CoreStatsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
