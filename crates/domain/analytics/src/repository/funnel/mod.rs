mod finders;
mod mutations;
mod stats;
mod types;

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Clone, Debug)]
pub struct FunnelRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl FunnelRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
