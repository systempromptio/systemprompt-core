mod mutations;
mod parts;
mod queries;

pub use parts::{get_artifact_parts, persist_artifact_part};

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ArtifactRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }
}
