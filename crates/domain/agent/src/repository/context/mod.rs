pub mod message;
mod mutations;
mod queries;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ContextRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    db_pool: DbPool,
}

impl ContextRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self {
            pool,
            write_pool,
            db_pool: db.clone(),
        })
    }
}

